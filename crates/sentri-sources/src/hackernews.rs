//! Hacker News connector via the public Algolia HN Search API
//! (`hn.algolia.com/api/v1`), which needs no key. One request per run.
//!
//! - no query: current front page;
//! - `query`: recent stories matching it (e.g. `nvidia`, `rust`);
//! - `target = "jobs"`: "Who is hiring?" style job stories.

use chrono::{DateTime, Utc};
use sentri_core::{Author, CanonicalUrl, Opportunity, SourceKind};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::html_to_text;

const API: &str = "https://hn.algolia.com/api/v1";
const ITEM: &str = "https://news.ycombinator.com/item?id=";
const USER: &str = "https://news.ycombinator.com/user?id=";

#[derive(Debug, Clone)]
pub struct HackerNews {
    pub(crate) spec: SourceSpec,
    base: Url,
}

impl HackerNews {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        if let Some(target) = spec.target.as_deref()
            && !matches!(target, "stories" | "jobs")
        {
            return Err(SourceError::Config(format!(
                "{}: target must be \"stories\" or \"jobs\"",
                spec.name
            )));
        }
        let base = Url::parse(spec.url.as_deref().unwrap_or(API))
            .map_err(|e| SourceError::Config(e.to_string()))?;
        Ok(Self { spec, base })
    }

    fn search_url(&self) -> Url {
        let query = self
            .spec
            .query
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        let (endpoint, tags) = match (self.spec.target.as_deref(), query.is_empty()) {
            (Some("jobs"), _) => ("search_by_date", "job"),
            (_, true) => ("search", "front_page"),
            (_, false) => ("search_by_date", "story"),
        };
        let mut url = self.base.clone();
        url.set_path(&format!(
            "{}/{endpoint}",
            self.base.path().trim_end_matches('/')
        ));
        let hits = self.spec.max_items.clamp(1, 100).to_string();
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("tags", tags)
                .append_pair("hitsPerPage", &hits);
            if !query.is_empty() {
                pairs.append_pair("query", query);
            }
        }
        url
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.search_url()).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse(&json, &self.spec.name, now)
    }
}

/// Converts an Algolia response into items. Stories without an external
/// link point to their discussion page.
pub fn parse(
    json: &Value,
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let hits = json
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("missing `hits` array".into()))?;
    Ok(hits
        .iter()
        .filter_map(|hit| item(hit, source_name, now))
        .collect())
}

fn item(hit: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let str_at = |k: &str| hit.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let id = str_at("objectID")?;
    let title = str_at("title")?;
    let discussion = format!("{ITEM}{id}");
    let link = str_at("url").unwrap_or(&discussion).to_owned();
    let canonical = CanonicalUrl::parse(&link).ok()?;
    let mut o = Opportunity::new(
        SourceKind::HackerNews,
        source_name,
        &link,
        &canonical,
        title,
        now,
    );
    o.external_id = Some(id.to_owned());
    o.dates.published_at = str_at("created_at")
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.to_utc());
    let points = hit.get("points").and_then(Value::as_u64).unwrap_or(0);
    let comments = hit.get("num_comments").and_then(Value::as_u64).unwrap_or(0);
    let mut summary = format!("▲ {points} points · {comments} comments · discussion: {discussion}");
    if let Some(text) = str_at("story_text") {
        summary = format!("{} — {summary}", html_to_text(text));
    }
    o.summary = Some(summary);
    if let Some(user) = str_at("author") {
        o.author = Author {
            name: None,
            handle: Some(user.to_owned()),
            profile_url: Some(format!("{USER}{user}")),
            avatar_url: None,
        };
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(query: Option<&str>, target: Option<&str>) -> SourceSpec {
        SourceSpec {
            name: "hn".into(),
            kind: SourceKind::HackerNews,
            url: None,
            query: query.map(str::to_owned),
            target: target.map(str::to_owned),
            category: None,
            tags: vec![],
            max_items: 30,
            token: None,
            content: sentri_core::ContentKind::News,
            stance: None,
        }
    }

    #[test]
    fn builds_urls_for_each_mode() {
        let front = HackerNews::new(spec(None, None)).unwrap().search_url();
        assert_eq!(front.path(), "/api/v1/search");
        assert!(front.query().unwrap().contains("tags=front_page"));
        let q = HackerNews::new(spec(Some("nvidia"), None))
            .unwrap()
            .search_url();
        assert_eq!(q.path(), "/api/v1/search_by_date");
        assert!(q.query().unwrap().contains("query=nvidia"));
        let jobs = HackerNews::new(spec(None, Some("jobs")))
            .unwrap()
            .search_url();
        assert!(jobs.query().unwrap().contains("tags=job"));
        assert!(HackerNews::new(spec(None, Some("users"))).is_err());
    }

    #[test]
    fn parses_hits() {
        let body = json!({"hits": [
            {"objectID": "1", "title": "Linux 7.0", "url": "https://lwn.example/a",
             "author": "pg", "points": 500, "num_comments": 120, "created_at": "2026-11-12T13:52:29Z"},
            {"objectID": "2", "title": "Ask HN: Rust or Go?", "url": null, "author": "x",
             "story_text": "<p>Thoughts?</p>", "points": 10, "num_comments": 3},
            {"objectID": "3"}
        ]});
        let items = parse(&body, "hn", Utc::now()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].original_url, "https://lwn.example/a");
        assert_eq!(items[0].author.handle.as_deref(), Some("pg"));
        assert!(
            items[0]
                .summary
                .as_deref()
                .unwrap()
                .starts_with("▲ 500 points")
        );
        assert_eq!(
            items[1].original_url,
            "https://news.ycombinator.com/item?id=2"
        );
        assert!(
            items[1]
                .summary
                .as_deref()
                .unwrap()
                .starts_with("Thoughts?")
        );
    }
}
