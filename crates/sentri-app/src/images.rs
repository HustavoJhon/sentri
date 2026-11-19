//! Image download, disk cache and safe decoding.
//!
//! Images are untrusted input: they are downloaded through `SafeClient` with a
//! size cap and a strict MIME allowlist, decoded with dimension and memory
//! limits (against decompression bombs) on a blocking thread, and only RGB
//! pixels reach the UI.
//!
//! Quality: the best available variant of each image is downloaded (see
//! [`crate::image_source`]), decoded once into a *source* image of up to
//! [`SOURCE_EDGE`] pixels, and every size the interface draws is resampled
//! from that source with a Lanczos filter, never from an earlier resize.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits, RgbImage};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

use crate::Sentri;

/// Largest image file accepted. Original uploads behind `WordPress` crops are
/// often a few megabytes; anything larger falls back to the published URL.
pub const MAX_IMAGE_BYTES: u64 = 8_000_000;
/// Longest edge kept after decoding. Enough for a details pane drawn with a
/// graphics protocol on a high-DPI terminal (about 70 cells × 14 px).
pub const SOURCE_EDGE: u32 = 1024;
/// Transparent pixels are composited on Gruvbox's dark background, which is
/// what the terminal shows behind the image.
pub const BACKGROUND: [u8; 3] = [0x28, 0x28, 0x28];
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

/// Decoded RGB pixels, shared cheaply between the state and the renderer.
#[derive(Clone, PartialEq, Eq)]
pub struct Thumbnail {
    image: Arc<RgbImage>,
}

impl std::fmt::Debug for Thumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thumbnail({}×{})", self.width(), self.height())
    }
}

impl From<RgbImage> for Thumbnail {
    fn from(image: RgbImage) -> Self {
        Self {
            image: Arc::new(image),
        }
    }
}

impl Thumbnail {
    /// Builds an image from row-major pixels; `None` when the sizes disagree.
    pub fn from_pixels(width: u32, height: u32, pixels: &[[u8; 3]]) -> Option<Self> {
        let raw: Vec<u8> = pixels.iter().flatten().copied().collect();
        RgbImage::from_raw(width, height, raw).map(Self::from)
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn rgb(&self) -> &RgbImage {
        &self.image
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        (x < self.width() && y < self.height()).then(|| self.image.get_pixel(x, y).0)
    }

    /// Largest size with this image's aspect ratio inside `max_w`×`max_h`
    /// (never zero for a non-empty box).
    pub fn fit_size(&self, max_w: u32, max_h: u32) -> (u32, u32) {
        let (w, h) = (u64::from(self.width()), u64::from(self.height()));
        let (mw, mh) = (u64::from(max_w), u64::from(max_h));
        if w == 0 || h == 0 || mw == 0 || mh == 0 {
            return (0, 0);
        }
        let (fw, fh) = if mw * h <= mh * w {
            (mw, (mw * h / w).max(1))
        } else {
            ((mh * w / h).max(1), mh)
        };
        (
            u32::try_from(fw).unwrap_or(max_w),
            u32::try_from(fh).unwrap_or(max_h),
        )
    }

    /// Exactly `width`×`height`, resampled from this image. Shrinking uses
    /// Lanczos3 (sharp, no aliasing); enlarging uses Catmull-Rom, which
    /// stays smooth instead of turning pixels into visible squares.
    #[must_use]
    pub fn resized(&self, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.width(), self.height()) {
            return self.clone();
        }
        let filter = if width < self.width() || height < self.height() {
            FilterType::Lanczos3
        } else {
            FilterType::CatmullRom
        };
        image::imageops::resize(self.rgb(), width, height, filter).into()
    }

    /// Fills `width`×`height` completely, cropping the overflow around the
    /// center (used for avatars).
    #[must_use]
    pub fn cover(&self, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let (w, h) = (u64::from(self.width()), u64::from(self.height()));
        let (tw, th) = (u64::from(width), u64::from(height));
        let (cw, ch) = if w * th > h * tw {
            ((h * tw / th).max(1), h)
        } else {
            (w, (w * th / tw).max(1))
        };
        let x = u32::try_from((w - cw) / 2).unwrap_or(0);
        let y = u32::try_from((h - ch) / 2).unwrap_or(0);
        let cropped = image::imageops::crop_imm(
            self.rgb(),
            x,
            y,
            u32::try_from(cw).unwrap_or(1),
            u32::try_from(ch).unwrap_or(1),
        )
        .to_image();
        Self::from(cropped).resized(width, height)
    }

    /// The image as a `DynamicImage`, for graphics protocol encoders.
    pub fn to_dynamic(&self) -> DynamicImage {
        DynamicImage::ImageRgb8(self.rgb().clone())
    }
}

/// Decodes `bytes` into a source image whose longest edge is at most
/// `max_edge` pixels. Smaller images keep their size: they are never
/// upscaled here, only when drawn.
pub fn decode_image(bytes: &[u8], max_edge: u32) -> Result<Thumbnail, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8_192);
    limits.max_image_height = Some(8_192);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    if image.width() == 0 || image.height() == 0 {
        return Err(ImageError::Decode("empty image".into()));
    }
    let max_edge = max_edge.max(1);
    let image = if image.width() > max_edge || image.height() > max_edge {
        image.resize(max_edge, max_edge, FilterType::Lanczos3)
    } else {
        image
    };
    Ok(flatten(&image, BACKGROUND).into())
}

/// RGB pixels with transparency blended over `background` (dropping the
/// alpha channel would show hidden, often black or garbage, colors).
fn flatten(image: &DynamicImage, background: [u8; 3]) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |c: u8, bg: u8| {
            let mixed =
                (u16::from(c) * u16::from(a) + u16::from(bg) * (255 - u16::from(a)) + 127) / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        image::Rgb([
            blend(r, background[0]),
            blend(g, background[1]),
            blend(b, background[2]),
        ])
    })
}

fn cache_path(dir: &Path, key: &str) -> PathBuf {
    let digest = Sha256::digest(key.as_bytes());
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

async fn decode_blocking(bytes: Vec<u8>, max_edge: u32) -> Result<Thumbnail, ImageError> {
    tokio::task::spawn_blocking(move || decode_image(&bytes, max_edge))
        .await
        .map_err(|e| ImageError::Decode(e.to_string()))?
}

impl Sentri {
    /// The decoded source image for `url`, at up to [`SOURCE_EDGE`] pixels.
    pub async fn image(&self, url: &str) -> Result<Thumbnail, ImageError> {
        self.image_cached(url, url, SOURCE_EDGE).await
    }

    /// Downloads the best variant of `url` (see [`crate::image_source`]),
    /// falling back to the published URL when a larger variant is missing,
    /// too big or undecodable. The original bytes of whichever variant
    /// worked are cached under `cache_key`, so a cache hit costs no request
    /// at all; decoding never blocks the async runtime.
    pub async fn image_cached(
        &self,
        url: &str,
        cache_key: &str,
        max_edge: u32,
    ) -> Result<Thumbnail, ImageError> {
        let candidates = crate::image_source::candidates(url);
        if candidates.is_empty() {
            return Err(ImageError::Url(format!("unsupported image URL: {url}")));
        }
        let path = cache_path(&self.cache_dir(), cache_key);
        if fresh(&path)
            && let Ok(bytes) = std::fs::read(&path)
        {
            match decode_blocking(bytes, max_edge).await {
                Ok(image) => return Ok(image),
                Err(error) => tracing::debug!(%error, "cached image unusable; downloading again"),
            }
        }
        let mut last = None;
        for candidate in candidates {
            match self.download_and_decode(&candidate, max_edge).await {
                Ok((bytes, image)) => {
                    if let Err(error) = write_private(&path, &bytes) {
                        tracing::debug!(%error, "could not cache image");
                    }
                    return Ok(image);
                }
                Err(error) => {
                    tracing::debug!(%error, url = %candidate, "image variant unavailable");
                    last = Some(error);
                }
            }
        }
        Err(last.unwrap_or_else(|| ImageError::Url(url.to_owned())))
    }

    async fn download_and_decode(
        &self,
        url: &Url,
        max_edge: u32,
    ) -> Result<(Vec<u8>, Thumbnail), ImageError> {
        let page = self.http().get_image(url, MAX_IMAGE_BYTES).await?;
        let image = decode_blocking(page.body.clone(), max_edge).await?;
        Ok((page.body, image))
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
    fn keeps_small_images_at_their_size_and_shrinks_large_ones() {
        let small = decode_image(&png(40, 20), 1024).unwrap();
        assert_eq!((small.width(), small.height()), (40, 20), "never upscaled");
        assert_eq!(small.pixel(0, 0), Some([255, 0, 0]));
        assert_eq!(small.pixel(39, 19), Some([0, 0, 255]));
        assert_eq!(small.pixel(40, 0), None);
        let large = decode_image(&png(2000, 500), 1024).unwrap();
        assert_eq!((large.width(), large.height()), (1024, 256));
    }

    #[test]
    fn rejects_garbage_and_oversized_images() {
        assert!(decode_image(b"not an image", 10).is_err());
        assert!(decode_image(b"<svg xmlns='http://www.w3.org/2000/svg'/>", 10).is_err());
        let mut truncated = png(32, 32);
        truncated.truncate(truncated.len() / 2);
        assert!(decode_image(&truncated, 10).is_err());
        assert!(decode_image(&png(9_000, 1), 10).is_err());
    }

    #[test]
    fn transparency_blends_with_the_background() {
        let img = image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgba([255, 255, 255, 0])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        });
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let decoded = decode_image(&bytes, 10).unwrap();
        assert_eq!(decoded.pixel(0, 0), Some(BACKGROUND));
        assert_eq!(decoded.pixel(1, 0), Some([255, 255, 255]));
    }

    #[test]
    fn variants_come_from_the_source_with_the_right_size() {
        let source = decode_image(&png(400, 200), 1024).unwrap();
        assert_eq!(source.fit_size(100, 100), (100, 50));
        assert_eq!(source.fit_size(100, 20), (40, 20));
        assert_eq!(source.fit_size(0, 20), (0, 0));
        let small = source.resized(40, 20);
        assert_eq!((small.width(), small.height()), (40, 20));
        assert_eq!(small.pixel(2, 10), Some([255, 0, 0]));
        assert_eq!(small.pixel(37, 10), Some([0, 0, 255]));
        let square = source.cover(30, 30);
        assert_eq!((square.width(), square.height()), (30, 30));
        assert_eq!(source.resized(400, 200), source, "same size is free");
    }

    #[test]
    fn builds_from_pixels() {
        let t = Thumbnail::from_pixels(2, 1, &[[1, 2, 3], [4, 5, 6]]).unwrap();
        assert_eq!(t.pixel(1, 0), Some([4, 5, 6]));
        assert!(Thumbnail::from_pixels(3, 1, &[[0; 3]]).is_none());
        assert_eq!(format!("{t:?}"), "Thumbnail(2×1)");
    }

    #[test]
    fn cache_paths_are_stable_and_opaque() {
        let a = cache_path(Path::new("/c"), "https://x.example/a.png");
        assert_eq!(a, cache_path(Path::new("/c"), "https://x.example/a.png"));
        assert!(!a.to_string_lossy().contains("x.example"));
    }
}
