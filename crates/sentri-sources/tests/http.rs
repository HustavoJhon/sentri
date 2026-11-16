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

#[tokio::test]
async fn authenticated_requests_do_not_follow_redirects() {
    let server = MockServer::start().await;
    Mock::given(path("/api"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/elsewhere"))
        .mount(&server)
        .await;
    Mock::given(path("/elsewhere"))
        .respond_with(html("leaked"))
        .mount(&server)
        .await;
    let client = local_client(1_000);
    let url = Url::parse(&format!("{}/api", server.uri())).unwrap();
    let auth = [("authorization", "Bearer secret".to_owned())];
    assert!(matches!(
        client.get_with(&url, &auth).await,
        Err(FetchError::Status(302))
    ));
    assert!(client.get(&url).await.is_ok());
}

#[tokio::test]
async fn images_have_their_own_type_and_size_limits() {
    let server = MockServer::start().await;
    Mock::given(path("/a.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0u8; 100], "image/png"))
        .mount(&server)
        .await;
    Mock::given(path("/a.svg"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"<svg/>".to_vec(), "image/svg+xml"))
        .mount(&server)
        .await;
    let client = local_client(1_000);
    let url = |p: &str| Url::parse(&format!("{}{p}", server.uri())).unwrap();
    assert_eq!(
        client
            .get_image(&url("/a.png"), 200)
            .await
            .unwrap()
            .body
            .len(),
        100
    );
    assert!(matches!(
        client.get_image(&url("/a.png"), 50).await,
        Err(FetchError::TooLarge(50))
    ));
    assert!(matches!(
        client.get_image(&url("/a.svg"), 200).await,
        Err(FetchError::ContentType(_))
    ));
    assert!(
        matches!(
            client.get(&url("/a.png")).await,
            Err(FetchError::ContentType(_))
        ),
        "documents and images are fetched separately"
    );
}

#[tokio::test]
async fn kaggle_sends_credentials_only_to_the_api() {
    use sentri_core::{ContentKind, SourceKind};
    use sentri_sources::{Connector, SourceSpec};
    use wiremock::matchers::header;

    let server = MockServer::start().await;
    Mock::given(path("/api/v1/competitions/list"))
        .and(header("authorization", "Basic dXNlcjprZXk="))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            br#"[{"id":1,"ref":"titanic","title":"Titanic"}]"#.to_vec(),
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;
    let spec = SourceSpec {
        name: "kaggle".into(),
        kind: SourceKind::Kaggle,
        url: Some(format!("{}/api/v1/competitions/list", server.uri())),
        query: None,
        target: None,
        category: None,
        tags: vec![],
        max_items: 10,
        token: Some("user:key".into()),
        content: ContentKind::Opportunity,
        stance: None,
    };
    let items = Connector::from_spec(spec)
        .unwrap()
        .fetch(&local_client(10_000), chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].category, sentri_core::Category::Competition);
}

#[tokio::test]
async fn conditional_requests_report_not_modified() {
    use sentri_sources::Validators;
    use wiremock::matchers::header;
    let server = MockServer::start().await;
    Mock::given(path("/feed"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    Mock::given(path("/feed"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"v1\"")
                .set_body_raw(b"<rss/>".to_vec(), "application/rss+xml"),
        )
        .mount(&server)
        .await;
    let client = local_client(1_000);
    let url = Url::parse(&format!("{}/feed", server.uri())).unwrap();
    let first = client
        .get_conditional(&url, &Validators::default())
        .await
        .unwrap();
    assert_eq!(first.validators.etag.as_deref(), Some("\"v1\""));
    assert!(matches!(
        client.get_conditional(&url, &first.validators).await,
        Err(FetchError::NotModified)
    ));
}
