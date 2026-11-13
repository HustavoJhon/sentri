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
