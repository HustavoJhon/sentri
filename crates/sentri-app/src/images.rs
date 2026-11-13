//! Image download, disk cache and safe decoding into small thumbnails.
//!
//! Images are untrusted input: they are downloaded through `SafeClient` with a
//! size cap and a strict MIME allowlist, decoded with dimension and memory
//! limits (against decompression bombs) on a blocking thread, and only the
//! resized RGB pixels reach the UI.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use image::{ImageReader, Limits};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

use crate::Sentri;

/// Largest image file accepted.
pub const MAX_IMAGE_BYTES: u64 = 3_000_000;
/// Cached files older than this are downloaded again.
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("invalid image URL: {0}")]
    Url(String),
    #[error(transparent)]
    Fetch(#[from] sentri_sources::FetchError),
    #[error("cannot decode image: {0}")]
    Decode(String),
    #[error("image cache error: {0}")]
    Io(#[from] std::io::Error),
}

/// Decoded, resized RGB pixels, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 3]>,
}

impl Thumbnail {
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(y * self.width + x).ok()?;
        self.pixels.get(index).copied()
    }
}

/// Decodes `bytes` and fits the result in `max_w`×`max_h` pixels.
pub fn decode_thumbnail(bytes: &[u8], max_w: u32, max_h: u32) -> Result<Thumbnail, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8_192);
    limits.max_image_height = Some(8_192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let rgb = image.thumbnail(max_w.max(1), max_h.max(1)).to_rgb8();
    Ok(Thumbnail {
        width: rgb.width(),
        height: rgb.height(),
        pixels: rgb.pixels().map(|p| p.0).collect(),
    })
}

fn cache_path(dir: &Path, url: &str) -> PathBuf {
    let digest = Sha256::digest(url.as_bytes());
    let name = digest.iter().take(16).fold(String::new(), |mut out, b| {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
        out
    });
    dir.join(format!("{name}.img"))
}

fn fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < CACHE_TTL)
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(path)?, bytes)
}

/// Default image cache directory (`~/.cache/sentri/images`).
pub fn default_cache_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "sentri").map_or_else(
        || PathBuf::from(".sentri-cache/images"),
        |d| d.cache_dir().join("images"),
    )
}

impl Sentri {
    /// Raw image bytes from the cache or the network.
    pub async fn image_bytes(&self, url: &str) -> Result<Vec<u8>, ImageError> {
        let parsed = Url::parse(url).map_err(|e| ImageError::Url(e.to_string()))?;
        let path = cache_path(&self.cache_dir(), url);
        if fresh(&path)
            && let Ok(bytes) = std::fs::read(&path)
        {
            return Ok(bytes);
        }
        let page = self.http().get_image(&parsed, MAX_IMAGE_BYTES).await?;
        if let Err(error) = write_private(&path, &page.body) {
            tracing::debug!(%error, "could not cache image");
        }
        Ok(page.body)
    }

    /// Downloads (or reuses) and decodes an image into a thumbnail, without
    /// blocking the async runtime.
    pub async fn image_thumbnail(
        &self,
        url: &str,
        max_w: u32,
        max_h: u32,
    ) -> Result<Thumbnail, ImageError> {
        let bytes = self.image_bytes(url).await?;
        tokio::task::spawn_blocking(move || decode_thumbnail(&bytes, max_w, max_h))
            .await
            .map_err(|e| ImageError::Decode(e.to_string()))?
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.config()
            .ui
            .cache_dir
            .clone()
            .unwrap_or_else(default_cache_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn decodes_and_fits_thumbnails() {
        let thumb = decode_thumbnail(&png(40, 20), 10, 10).unwrap();
        assert_eq!((thumb.width, thumb.height), (10, 5));
        assert_eq!(thumb.pixel(0, 0), Some([255, 0, 0]));
        assert_eq!(thumb.pixel(9, 4), Some([0, 0, 255]));
        assert_eq!(thumb.pixel(10, 0), None);
    }

    #[test]
    fn rejects_garbage_and_oversized_images() {
        assert!(decode_thumbnail(b"not an image", 10, 10).is_err());
        assert!(decode_thumbnail(&png(9_000, 1), 10, 10).is_err());
    }

    #[test]
    fn cache_paths_are_stable_and_opaque() {
        let a = cache_path(Path::new("/c"), "https://x.example/a.png");
        assert_eq!(a, cache_path(Path::new("/c"), "https://x.example/a.png"));
        assert!(!a.to_string_lossy().contains("x.example"));
    }
}
