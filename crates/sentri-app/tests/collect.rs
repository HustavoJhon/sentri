use std::path::Path;

use sentri_app::{Config, Sentri};
use sentri_core::{Category, FeedQuery};
use sentri_storage::{RunStatus, Store};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

const FEED: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>t</title>
<item><title>Hackathon de IA</title><link>https://hack.example/1</link><guid>1</guid></item>
<item><title>Untitled news</title><link>https://hack.example/2</link><guid>2</guid></item>
</channel></rss>"#;

fn config(server: &MockServer) -> Config {
    let text = format!(
        r#"
[[sources]]
name = "good"
kind = "rss"
url = "{0}/feed.xml"
category = "event"
tags = ["peru"]

[[sources]]
name = "broken"
kind = "rss"
url = "{0}/missing.xml"

[[sources]]
name = "disabled"
kind = "rss"
enabled = false
url = "{0}/feed.xml"

[[sources]]
name = "linkedin"
kind = "linkedin"
"#,
        server.uri()
    );
    Config::parse(&text, Path::new("test")).unwrap()
}

#[tokio::test]
async fn collects_each_source_independently() {
    let server = MockServer::start().await;
    Mock::given(path("/feed.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(FEED.as_bytes().to_vec(), "application/rss+xml"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/missing.xml"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    let reports = app.collect(None).await;

    let names: Vec<&str> = reports.iter().map(|r| r.source.as_str()).collect();
    assert_eq!(
        names,
        ["good", "broken", "linkedin"],
        "disabled source skipped"
    );
    assert_eq!(reports[0].result, Ok((2, 2)));
    assert!(reports[1].result.as_ref().unwrap_err().contains("500"));
    assert!(
        reports[2]
            .result
            .as_ref()
            .unwrap_err()
            .contains("not supported")
    );

    let feed = app.feed(&FeedQuery::default()).await.unwrap();
    assert_eq!(feed.len(), 2);
    let hack = feed
        .iter()
        .find(|r| r.item.opportunity.title == "Hackathon de IA")
        .unwrap();
    assert_eq!(hack.item.opportunity.category, Category::Hackathon);
    let other = feed
        .iter()
        .find(|r| r.item.opportunity.title == "Untitled news")
        .unwrap();
    assert_eq!(
        other.item.opportunity.category,
        Category::Event,
        "source default"
    );
    assert!(other.item.opportunity.tags.contains(&"peru".to_owned()));

    // Second run: everything is a duplicate.
    assert_eq!(app.collect(Some("good")).await[0].result, Ok((2, 0)));

    let runs = app.store().latest_runs().await.unwrap();
    let broken = runs.iter().find(|r| r.source_name == "broken").unwrap();
    assert_eq!(broken.status, RunStatus::Failed);
}

#[tokio::test]
async fn failing_sources_back_off_and_unchanged_feeds_are_skipped() {
    use wiremock::matchers::header;
    let server = MockServer::start().await;
    Mock::given(path("/feed.xml"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    Mock::given(path("/feed.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"v1\"")
                .set_body_raw(FEED.as_bytes().to_vec(), "application/rss+xml"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/missing.xml"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();

    let first = app.collect(None).await;
    assert_eq!(first[0].result, Ok((2, 2)));
    assert!(first[1].result.is_err());

    let second = app.collect(None).await;
    assert_eq!(second[0].note.as_deref(), Some("not modified"));
    assert!(
        second[1]
            .note
            .as_deref()
            .unwrap()
            .starts_with("skipped after 1 failures")
    );
    assert!(second[1].result.is_ok(), "skipping is not a new failure");

    // Asking for the source explicitly ignores the back-off.
    let forced = app.collect(Some("broken")).await;
    assert!(forced[0].result.is_err());
    let state = app.store().source_state("broken").await.unwrap();
    assert_eq!(state.consecutive_failures, 2);
    assert!(state.last_error.unwrap().contains("500"));
}

#[tokio::test]
async fn tests_a_source_without_storing_anything() {
    let server = MockServer::start().await;
    Mock::given(path("/feed.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(FEED.as_bytes().to_vec(), "application/rss+xml"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/page.html"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(b"<html></html>".to_vec(), "text/html"),
        )
        .mount(&server)
        .await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    let draft = |url: String| sentri_app::settings::SourceDraft {
        name: "blog".into(),
        kind: "rss".into(),
        url,
        content: "news".into(),
        category: String::new(),
        enabled: true,
    };
    let ok = sentri_app::draft_config(&draft(format!("{}/feed.xml", server.uri()))).unwrap();
    assert_eq!(app.test_source(&ok).await, Ok(2));
    let html = sentri_app::draft_config(&draft(format!("{}/page.html", server.uri()))).unwrap();
    assert!(app.test_source(&html).await.unwrap_err().contains("parse"));
    let bad = sentri_app::draft_config(&draft("file:///etc/passwd".into())).unwrap();
    assert!(app.test_source(&bad).await.is_err());
    assert_eq!(app.store().count().await.unwrap(), 0, "nothing stored");
}

#[tokio::test]
async fn collects_in_parallel_and_reports_each_source() {
    let server = MockServer::start().await;
    Mock::given(path("/feed.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(FEED.as_bytes().to_vec(), "application/rss+xml")
                .set_delay(std::time::Duration::from_millis(50)),
        )
        .mount(&server)
        .await;
    Mock::given(path("/missing.xml"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    let mut reports = Vec::new();
    let total = app.collect_each(4, |r| reports.push(r)).await;

    assert_eq!(total, 3, "enabled sources only");
    let mut names: Vec<&str> = reports.iter().map(|r| r.source.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["broken", "good", "linkedin"]);
    let good = reports.iter().find(|r| r.source == "good").unwrap();
    assert_eq!(good.result, Ok((2, 2)));
    assert_eq!(app.feed(&FeedQuery::default()).await.unwrap().len(), 2);
}

#[tokio::test]
async fn counts_items_per_view() {
    let server = MockServer::start().await;
    Mock::given(path("/feed.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(FEED.as_bytes().to_vec(), "application/rss+xml"),
        )
        .mount(&server)
        .await;
    let store = Store::in_memory().await.unwrap();
    let app = Sentri::with_store(config(&server), store, true).unwrap();
    app.collect(Some("good")).await;
    let counts = app
        .view_counts(&[
            sentri_core::FeedView::Discover,
            sentri_core::FeedView::Hackathons,
            sentri_core::FeedView::Jobs,
        ])
        .await
        .unwrap();
    assert_eq!(
        counts,
        [
            (sentri_core::FeedView::Discover, 2),
            (sentri_core::FeedView::Hackathons, 1),
            (sentri_core::FeedView::Jobs, 0),
        ]
    );
}
