use std::path::Path;

use sentri_app::{AddOutcome, Config, LinkOverrides, Sentri};
use sentri_core::{Category, Cost, Modality};
use sentri_storage::Store;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn app() -> Sentri {
    let config = Config::parse("", Path::new("test")).unwrap();
    let store = Store::in_memory().await.unwrap();
    Sentri::with_store(config, store, true).unwrap()
}

const EVENT_PAGE: &str = r#"<html><head>
<meta property="og:title" content="DevFest Cusco 2026">
<meta property="og:description" content="Free GDG conference with certificate">
<script type="application/ld+json">{"@type":"Event","startDate":"2026-12-05T09:00:00-05:00",
 "eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode",
 "location":{"name":"Cusco","address":{"addressCountry":"PE"}},"offers":{"price":0}}</script>
</head></html>"#;

#[tokio::test]
async fn adds_link_with_extracted_metadata_and_detects_duplicates() {
    let server = MockServer::start().await;
    Mock::given(path("/devfest"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(EVENT_PAGE.as_bytes().to_vec(), "text/html"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let app = app().await;
    let url = format!("{}/devfest?utm_source=whatsapp", server.uri());

    let AddOutcome::Added { item, warnings } =
        app.add_link(&url, &LinkOverrides::default()).await.unwrap()
    else {
        panic!("expected a new item");
    };
    let o = &item.opportunity;
    assert_eq!(o.title, "DevFest Cusco 2026");
    assert_eq!(o.category, Category::Conference);
    assert_eq!(o.modality, Modality::InPerson);
    assert_eq!(o.location.as_deref(), Some("Cusco"));
    assert_eq!(o.cost, Cost::Free);
    assert_eq!(o.perks.certificate, Some(true));
    assert!(o.dates.starts_at.is_some());
    assert_eq!(o.original_url, url);
    assert!(warnings.is_empty(), "{warnings:?}");

    // Same page without tracking parameters: duplicate, and no second fetch.
    let again = app
        .add_link(
            &format!("{}/devfest", server.uri()),
            &LinkOverrides::default(),
        )
        .await
        .unwrap();
    assert!(matches!(again, AddOutcome::Duplicate(i) if i.id == item.id));
}

#[tokio::test]
async fn unreachable_pages_are_stored_with_overrides_and_warnings() {
    let server = MockServer::start().await;
    Mock::given(path("/gone"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let app = app().await;
    let overrides = LinkOverrides {
        title: Some("Beca Chevening".into()),
        tags: vec!["uk".into()],
        ..LinkOverrides::default()
    };
    let AddOutcome::Added { item, warnings } = app
        .add_link(&format!("{}/gone", server.uri()), &overrides)
        .await
        .unwrap()
    else {
        panic!("expected a new item");
    };
    assert_eq!(item.opportunity.title, "Beca Chevening");
    assert_eq!(item.opportunity.category, Category::Scholarship);
    assert_eq!(item.opportunity.tags, ["uk"]);
    assert!(warnings.iter().any(|w| w.contains("404")));
}

#[tokio::test]
async fn rejects_invalid_urls() {
    let app = app().await;
    assert!(
        app.add_link("file:///etc/passwd", &LinkOverrides::default())
            .await
            .is_err()
    );
    assert!(
        app.add_link("not a url", &LinkOverrides::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn corrections_are_applied_after_adding() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let app = app().await;
    let AddOutcome::Added { item, .. } = app
        .add_link(&format!("{}/x", server.uri()), &LinkOverrides::default())
        .await
        .unwrap()
    else {
        panic!("expected a new item");
    };
    let fixed = app
        .correct(
            item.id,
            &LinkOverrides {
                title: Some("GDG Cusco Meetup".into()),
                category: Some(Category::Meetup),
                modality: Some(Modality::InPerson),
                ..LinkOverrides::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(fixed.opportunity.title, "GDG Cusco Meetup");
    assert_eq!(fixed.opportunity.category, Category::Meetup);
    assert_eq!(fixed.opportunity.modality, Modality::InPerson);
}
