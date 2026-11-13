use std::time::Duration;

use sentri_sources::{FetchError, HttpLimits, SafeClient};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn local_client(max_body_bytes: u64) -> SafeClient {
    SafeClient::new(HttpLimits {
        timeout: Duration::from_secs(5),
        max_body_bytes,
        max_redirects: 2,
        allow_private_networks: true,
    })
    .unwrap()
}

fn html(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "text/html; charset=utf-8")
}

#[tokio::test]
async fn downloads_html() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/page"))
        .respond_with(html("<title>Hi</title>"))
        .mount(&server)
        .await;
    let url = Url::parse(&format!("{}/page", server.uri())).unwrap();
    let page = local_client(1_000).get(&url).await.unwrap();
    assert!(page.is_html());
    assert_eq!(page.text(), "<title>Hi</title>");
}

#[tokio::test]
async fn enforces_size_type_status_and_redirect_limits() {
    let server = MockServer::start().await;
    Mock::given(path("/big"))
        .respond_with(html(&"x".repeat(2_000)))
        .mount(&server)
        .await;
    Mock::given(path("/binary"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0u8; 10], "application/x-sh"))
        .mount(&server)
        .await;
    Mock::given(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(path("/loop"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/loop"))
        .mount(&server)
        .await;

    let client = local_client(1_000);
    let get = |p: &str| {
        let url = Url::parse(&format!("{}{p}", server.uri())).unwrap();
        let client = client.clone();
        async move { client.get(&url).await }
    };
    assert!(matches!(
        get("/big").await,
        Err(FetchError::TooLarge(1_000))
    ));
    assert!(
        matches!(get("/binary").await, Err(FetchError::ContentType(t)) if t == "application/x-sh")
    );
    assert!(matches!(
        get("/missing").await,
        Err(FetchError::Status(404))
    ));
    assert!(matches!(get("/loop").await, Err(FetchError::Request(_))));
}

#[tokio::test]
async fn production_client_refuses_local_servers() {
    let server = MockServer::start().await;
    Mock::given(path("/"))
        .respond_with(html("secret"))
        .mount(&server)
        .await;
    let client = SafeClient::new(HttpLimits::default()).unwrap();
    let url = Url::parse(&server.uri()).unwrap();
    assert!(matches!(
        client.get(&url).await,
        Err(FetchError::Forbidden(_))
    ));
}
