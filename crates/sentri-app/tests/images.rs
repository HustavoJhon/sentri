//! Image downloads: best variant first, fallback, and the disk cache.

use std::io::Cursor;
use std::path::Path;

use sentri_app::{Config, Sentri};
use sentri_storage::Store;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb([200, 100, 50]));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

async fn app(cache: &Path) -> Sentri {
    let mut config = Config::parse("", Path::new("test")).unwrap();
    config.ui.cache_dir = Some(cache.to_path_buf());
    let store = Store::in_memory().await.unwrap();
    Sentri::with_store(config, store, true).unwrap()
}

fn image(bytes: Vec<u8>) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_bytes(bytes)
        .insert_header("content-type", "image/png")
}

#[tokio::test]
async fn prefers_the_original_upload_over_the_published_crop() {
    let server = MockServer::start().await;
    Mock::given(path("/wp-content/uploads/photo.png"))
        .respond_with(image(png(1600, 900)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/wp-content/uploads/photo-500x500.png"))
        .respond_with(image(png(500, 500)))
        .expect(0)
        .mount(&server)
        .await;
    let cache = tempfile::tempdir().unwrap();
    let app = app(cache.path()).await;
    let url = format!("{}/wp-content/uploads/photo-500x500.png", server.uri());

    let first = app.image(&url).await.unwrap();
    assert_eq!(
        (first.width(), first.height()),
        (1024, 576),
        "original, shrunk once to the source size"
    );
    // A second load comes from the disk cache: no request (expect(1) above).
    let again = app.image(&url).await.unwrap();
    assert_eq!(first, again);
}

#[tokio::test]
async fn falls_back_to_the_published_url_when_the_variant_is_missing() {
    let server = MockServer::start().await;
    Mock::given(path("/wp-content/uploads/photo.png"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(path("/wp-content/uploads/photo-500x500.png"))
        .respond_with(image(png(500, 500)))
        .expect(1)
        .mount(&server)
        .await;
    let cache = tempfile::tempdir().unwrap();
    let app = app(cache.path()).await;
    let url = format!("{}/wp-content/uploads/photo-500x500.png", server.uri());
    let image = app.image(&url).await.unwrap();
    assert_eq!((image.width(), image.height()), (500, 500));
    app.image(&url).await.unwrap();
}

#[tokio::test]
async fn invalid_and_missing_images_are_errors_not_panics() {
    let server = MockServer::start().await;
    Mock::given(path("/broken.png"))
        .respond_with(image(b"definitely not a png".to_vec()))
        .mount(&server)
        .await;
    Mock::given(path("/page.html"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<html></html>")
                .insert_header("content-type", "text/html"),
        )
        .mount(&server)
        .await;
    let cache = tempfile::tempdir().unwrap();
    let app = app(cache.path()).await;
    for url in [
        format!("{}/broken.png", server.uri()),
        format!("{}/missing.png", server.uri()),
        format!("{}/page.html", server.uri()),
        "javascript:alert(1)".to_owned(),
    ] {
        assert!(app.image(&url).await.is_err(), "{url}");
    }
    let cached: Vec<_> = std::fs::read_dir(cache.path())
        .map(Iterator::collect)
        .unwrap_or_default();
    assert!(cached.is_empty(), "failures are not cached");
}
