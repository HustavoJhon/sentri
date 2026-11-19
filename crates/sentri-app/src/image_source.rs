//! Which URL to download for an image.
//!
//! Feeds and pages often point at a small, cropped preview of a larger
//! picture (`WordPress` `-500x500` crops, Meetup `thumb_` files, GitHub
//! avatars at their default size). Drawn in a details pane those previews
//! are upscaled and look blocky, so SENTRI first asks for the original or a
//! larger variant and keeps the published URL as the fallback.
//!
//! Only well-known, documented URL schemes are rewritten; anything else is
//! downloaded exactly as published.

use url::Url;

/// Edge requested from services that scale images on demand (GitHub,
/// Gravatar, Google). Large enough for a details pane on a high-DPI
/// terminal, small enough to stay a quick download.
pub const REQUESTED_EDGE: u32 = 460;

/// URLs to try for an image, best first; the last one is always the URL as
/// published. Returns an empty list for unparsable or non-HTTP(S) URLs.
pub fn candidates(raw: &str) -> Vec<Url> {
    let Ok(url) = Url::parse(raw) else {
        return Vec::new();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2);
    if let Some(better) = upgrade(&url).filter(|u| *u != url) {
        out.push(better);
    }
    out.push(url);
    out
}

fn upgrade(url: &Url) -> Option<Url> {
    let host = url.host_str()?.to_ascii_lowercase();
    if host == "avatars.githubusercontent.com" {
        return Some(with_query(url, "s", &REQUESTED_EDGE.to_string()));
    }
    if host.ends_with("gravatar.com") && url.path().starts_with("/avatar/") {
        return Some(with_query(url, "s", &REQUESTED_EDGE.to_string()));
    }
    if host.ends_with("googleusercontent.com") {
        return google_sized(url);
    }
    if host.ends_with("meetupstatic.com") {
        return meetup_highres(url);
    }
    if url.path().contains("/wp-content/uploads/") {
        return wordpress_original(url);
    }
    None
}

/// `url` with `key` set to `value`, keeping every other parameter.
fn with_query(url: &Url, key: &str, value: &str) -> Url {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != key && !(key == "s" && k == "size"))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let mut out = url.clone();
    {
        let mut query = out.query_pairs_mut();
        query.clear();
        for (k, v) in &pairs {
            query.append_pair(k, v);
        }
        query.append_pair(key, value);
    }
    out
}

/// `…/photo=s96-c` → `…/photo=s460-c`. Google encodes the size after the
/// last `=` of the path.
fn google_sized(url: &Url) -> Option<Url> {
    let path = url.path();
    let at = path.rfind('=')?;
    let spec = &path[at + 1..];
    let is_size = spec.split('-').next().is_some_and(|s| {
        s.len() > 1
            && matches!(&s[..1], "s" | "w" | "h")
            && s[1..].chars().all(|c| c.is_ascii_digit())
    });
    if !is_size {
        return None;
    }
    let mut out = url.clone();
    out.set_path(&format!("{}=s{REQUESTED_EDGE}", &path[..at]));
    Some(out)
}

/// Meetup keeps `highres_`, `600_`, `thumb_`, `preview_` … variants of every
/// upload side by side.
fn meetup_highres(url: &Url) -> Option<Url> {
    let path = url.path();
    let slash = path.rfind('/')?;
    let file = &path[slash + 1..];
    let rest = ["thumb_", "preview_", "600_", "member_", "global_"]
        .iter()
        .find_map(|prefix| file.strip_prefix(prefix))?;
    let mut out = url.clone();
    out.set_path(&format!("{}highres_{rest}", &path[..=slash]));
    Some(out)
}

/// `photo-500x500.jpg` → `photo.jpg`: `WordPress` stores the uploaded original
/// next to every generated size.
fn wordpress_original(url: &Url) -> Option<Url> {
    let path = url.path();
    let dot = path.rfind('.')?;
    let (stem, extension) = path.split_at(dot);
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        ".jpg" | ".jpeg" | ".png" | ".webp" | ".gif"
    ) {
        return None;
    }
    let dash = stem.rfind('-')?;
    let (width, height) = stem[dash + 1..].split_once('x')?;
    let numeric = |s: &str| (2..=4).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit());
    if !numeric(width) || !numeric(height) {
        return None;
    }
    let mut out = url.clone();
    out.set_path(&format!("{}{extension}", &stem[..dash]));
    out.set_query(None);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(raw: &str) -> String {
        candidates(raw)[0].to_string()
    }

    #[test]
    fn github_avatars_are_requested_large() {
        let c = candidates("https://avatars.githubusercontent.com/u/583231?v=4");
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0].as_str(),
            "https://avatars.githubusercontent.com/u/583231?v=4&s=460"
        );
        assert_eq!(
            first("https://avatars.githubusercontent.com/u/1?v=4&s=96"),
            "https://avatars.githubusercontent.com/u/1?v=4&s=460"
        );
    }

    #[test]
    fn wordpress_crops_fall_back_to_the_original_upload() {
        let c = candidates(
            "https://cdn.arstechnica.net/wp-content/uploads/2026/10/binance-500x500.jpg",
        );
        assert_eq!(
            c[0].as_str(),
            "https://cdn.arstechnica.net/wp-content/uploads/2026/10/binance.jpg"
        );
        assert_eq!(
            c[1].as_str(),
            "https://cdn.arstechnica.net/wp-content/uploads/2026/10/binance-500x500.jpg",
            "published URL stays as fallback"
        );
        assert_eq!(
            first(
                "https://blogs.nvidia.com/wp-content/uploads/2026/11/slurm-1920x1080-5942487-842x450.png"
            ),
            "https://blogs.nvidia.com/wp-content/uploads/2026/11/slurm-1920x1080-5942487.png"
        );
    }

    #[test]
    fn meetup_and_google_variants() {
        assert_eq!(
            first("https://secure.meetupstatic.com/photos/member/8/6/a/f/preview_320914479.webp"),
            "https://secure.meetupstatic.com/photos/member/8/6/a/f/highres_320914479.webp"
        );
        assert_eq!(
            first("https://lh3.googleusercontent.com/a/ACg8oc=s96-c"),
            "https://lh3.googleusercontent.com/a/ACg8oc=s460"
        );
    }

    #[test]
    fn unknown_urls_are_kept_as_published() {
        for raw in [
            "https://images.lumacdn.com/uploads/ew/4fb1510b.png",
            "https://example.com/wp-content/uploads/logo-dark.png",
            "https://example.com/photo-500x500.jpg",
            "https://cdn.sessionize.com/image/579e-1140o400o3-35vfU8Rxc3aC2Ur6eKykn3.svg",
        ] {
            let c = candidates(raw);
            assert_eq!(c.len(), 1, "{raw}");
            assert_eq!(c[0].as_str(), raw);
        }
    }

    #[test]
    fn rejects_non_http_urls() {
        assert_eq!(candidates("javascript:alert(1)"), Vec::<Url>::new());
        assert_eq!(candidates("file:///etc/passwd"), Vec::<Url>::new());
        assert_eq!(candidates("not a url"), Vec::<Url>::new());
    }
}
