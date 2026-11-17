//! End-to-end collection from the Peru sources against a local mock server.
//!
//! The mock publishes the same shapes as the real Peruanos.dev API and
//! hack0.dev event pages, so parsing, cross-source deduplication, incremental
//! collection and manual corrections are exercised without network access.

use std::path::Path;

use sentri_app::{Config, Sentri};
use sentri_core::{Category, FeedQuery, Field, SourceKind};
use sentri_storage::Store;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

const EVENTS: &str = r#"[
  {
    "title": "DevFest Cusco 2027",
    "description": "Conferencia de la comunidad",
    "date": "2027-01-20",
    "time": "09:00",
    "location": "Universidad Andina",
    "city": "Cusco",
    "type": "Presencial",
    "image_url": "https://example.com/devfest.png",
    "registration_url": "https://luma.com/devfest-cusco-2027",
    "tags": ["IA", "Android"],
    "organizer": "GDG Cusco"
  }
]"#;

const COMMUNITIES: &str = r#"[
  {
    "name": "GDG Cusco",
    "description": "Comunidad de desarrolladores",
    "city": "Cusco",
    "topics": ["Cloud", "Android"],
    "logo_url": "https://example.com/logo.png",
    "contact": {
      "email": "",
      "website": "https://gdg.community.dev/gdg-cusco/",
      "socialMedia": {"github": "https://github.com/gdg-cusco"}
    }
  }
]"#;

const CFS: &str = r#"[
  {
    "id": "cfs-devfest-cusco-2027",
    "title": "DevFest Cusco 2027 - Call for Speakers",
    "community": "GDG Cusco",
    "description": "Buscamos ponentes",
    "event_date": "2027-01-20",
    "deadline": "2026-12-15",
    "cfs_url": "https://sessionize.com/devfest-cusco-2027",
    "website_url": "https://gdg.community.dev/gdg-cusco/",
    "location": "Cusco",
    "city": "Cusco",
    "type": "Presencial",
    "topics": ["IA"],
    "status": "open"
  }
]"#;

const HACK0_EVENT: &str = r#"<html><head>
<script type="application/ld+json">
{"@type":"Event","name":"DevFest Cusco 2027",
 "startDate":"2027-01-20T14:00:00Z",
 "eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode",
 "location":{"@type":"Place","name":"Universidad Andina",
   "address":{"addressLocality":"Cusco","addressCountry":"PE"}},
 "offers":{"@type":"Offer","url":"https://luma.com/devfest-cusco-2027"},
 "organizer":{"@type":"Organization","name":"GDG Cusco"}}
</script></head><body></body></html>"#;

fn config(server: &MockServer) -> Config {
    let text = format!(
        r#"
[[sources]]
name = "peruanos-events"
kind = "peruanos"
url = "{0}"
target = "events"
category = "event"

[[sources]]
name = "peruanos-communities"
kind = "peruanos"
url = "{0}"
target = "communities"
category = "community"

[[sources]]
name = "peruanos-cfs"
kind = "peruanos"
url = "{0}"
target = "cfs"
category = "call_for_speakers"

[[sources]]
name = "hack0"
kind = "hack0"
url = "{0}/eventos"
category = "event"
"#,
        server.uri()
    );
    Config::parse(&text, Path::new("test")).unwrap()
}

async fn mount(server: &MockServer) {
    for (route, body, content_type) in [
        ("/api/events", EVENTS, "application/json"),
        ("/api/communities", COMMUNITIES, "application/json"),
        ("/api/cfs", CFS, "application/json"),
        (
            "/eventos",
            r#"<a href="/eventos/devfest-cusco-2027">x</a>"#,
            "text/html",
        ),
        ("/eventos/devfest-cusco-2027", HACK0_EVENT, "text/html"),
    ] {
        Mock::given(path(route))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), content_type),
            )
            .mount(server)
            .await;
    }
}

#[tokio::test]
async fn collects_peru_sources_and_merges_the_same_event_across_sources() {
    let server = MockServer::start().await;
    mount(&server).await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();

    let reports = app.collect(None).await;
    let counts: Vec<_> = reports
        .iter()
        .map(|r| (r.source.as_str(), r.result.clone()))
        .collect();
    assert_eq!(
        counts,
        vec![
            ("peruanos-events", Ok((1, 1))),
            ("peruanos-communities", Ok((1, 1))),
            ("peruanos-cfs", Ok((1, 1))),
            ("hack0", Ok((1, 0))),
        ]
    );

    let feed = app.feed(&FeedQuery::default()).await.unwrap();
    let titles: Vec<&str> = feed
        .iter()
        .map(|r| r.item.opportunity.title.as_str())
        .collect();
    assert!(titles.contains(&"DevFest Cusco 2027"), "{titles:?}");
    assert!(titles.contains(&"GDG Cusco"), "{titles:?}");
    assert!(
        titles.contains(&"DevFest Cusco 2027 - Call for Speakers"),
        "{titles:?}"
    );
    let event = feed
        .iter()
        .find(|r| r.item.opportunity.title == "DevFest Cusco 2027")
        .unwrap();
    assert_eq!(
        event.item.opportunity.source,
        SourceKind::Peruanos,
        "the first sighting keeps the logical item"
    );
    assert_eq!(
        event.item.opportunity.location.as_deref(),
        Some("Universidad Andina, Cusco")
    );

    let sightings = app.store().sightings(event.item.id).await.unwrap();
    let mut names: Vec<&str> = sightings.iter().map(|s| s.source_name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["hack0", "peruanos-events"]);

    let communities = app
        .feed(&FeedQuery::view(sentri_core::FeedView::Communities))
        .await
        .unwrap();
    assert_eq!(communities.len(), 1);
    assert_eq!(
        communities[0].item.opportunity.category,
        Category::Community
    );

    let calls = app
        .feed(&FeedQuery::view(sentri_core::FeedView::CallForSpeakers))
        .await
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].item.opportunity.category,
        Category::CallForSpeakers
    );
    assert!(calls[0].item.opportunity.dates.deadline.is_some());

    // A second run finds the same items and creates no duplicates.
    let second = app.collect(None).await;
    assert_eq!(second[0].result, Ok((1, 0)));
    assert_eq!(second[3].result, Ok((1, 0)));
    assert_eq!(app.store().count().await.unwrap(), 3);
}

#[tokio::test]
async fn manual_corrections_survive_repeated_collection() {
    let server = MockServer::start().await;
    mount(&server).await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    app.collect(None).await;

    let event = app
        .feed(&FeedQuery::default())
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.item.opportunity.title == "DevFest Cusco 2027")
        .unwrap();
    app.store()
        .set_overrides(
            event.item.id,
            &[(Field::Title, "Mi DevFest corregido".into())],
            chrono::Utc::now(),
        )
        .await
        .unwrap();

    app.collect(None).await;
    let feed = app.feed(&FeedQuery::default()).await.unwrap();
    let edited: Vec<&str> = feed
        .iter()
        .map(|r| r.item.opportunity.title.as_str())
        .filter(|t| *t == "Mi DevFest corregido")
        .collect();
    assert_eq!(
        edited.len(),
        1,
        "manual title is applied and not duplicated"
    );
    assert!(
        feed.iter()
            .all(|r| r.item.opportunity.title != "DevFest Cusco 2027"),
        "collection never overwrites the correction"
    );
}

#[tokio::test]
async fn a_broken_endpoint_does_not_stop_the_other_sources() {
    let server = MockServer::start().await;
    Mock::given(path("/api/events"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    Mock::given(path("/api/communities"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(COMMUNITIES.as_bytes().to_vec(), "application/json"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/api/cfs"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(b"not json".to_vec(), "application/json"),
        )
        .mount(&server)
        .await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    let reports = app.collect(None).await;

    assert!(reports[0].result.as_ref().unwrap_err().contains("429"));
    assert_eq!(reports[1].result, Ok((1, 1)));
    assert!(reports[2].result.as_ref().unwrap_err().contains("parse"));
    assert_eq!(app.store().count().await.unwrap(), 1);
}
