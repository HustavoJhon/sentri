//! GitHub REST search connector (repositories or issues).
//!
//! Unauthenticated search allows 10 requests/minute; a token (read from the
//! environment variable named in `token_env`) raises it to 30. Only public
//! metadata is stored.

use chrono::{DateTime, Utc};
use sentri_core::{CanonicalUrl, Category, Opportunity, SourceKind};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};

const API: &str = "https://api.github.com";

/// What to search for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Repositories,
    Issues,
}

/// A configured GitHub search.
#[derive(Debug, Clone)]
pub struct GitHub {
    pub(crate) spec: SourceSpec,
    target: Target,
    base: Url,
}

impl GitHub {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let target = match spec.target.as_deref() {
            None | Some("repositories") => Target::Repositories,
            Some("issues") => Target::Issues,
            Some(other) => {
                return Err(SourceError::Config(format!(
                    "{}: target must be \"repositories\" or \"issues\", not {other:?}",
                    spec.name
                )));
            }
        };
        if spec.query.as_deref().is_none_or(|q| q.trim().is_empty()) {
            return Err(SourceError::Config(format!(
                "{}: `query` is required",
                spec.name
            )));
        }
        // `url` may point to a GitHub Enterprise or test API base.
        let base = Url::parse(spec.url.as_deref().unwrap_or(API))
            .map_err(|e| SourceError::Config(e.to_string()))?;
        Ok(Self { spec, target, base })
    }

    fn search_url(&self) -> Url {
        let path = match self.target {
            Target::Repositories => "search/repositories",
            Target::Issues => "search/issues",
        };
        let mut url = self.base.clone();
        url.set_path(&format!(
            "{}/{path}",
            self.base.path().trim_end_matches('/')
        ));
        let per_page = self.spec.max_items.clamp(1, 100).to_string();
        url.query_pairs_mut()
            .append_pair("q", self.spec.query.as_deref().unwrap_or_default())
            .append_pair("sort", "updated")
            .append_pair("order", "desc")
            .append_pair("per_page", &per_page);
        url
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let mut headers = vec![
            ("accept", "application/vnd.github+json".to_owned()),
            ("x-github-api-version", "2022-11-28".to_owned()),
        ];
        if let Some(token) = &self.spec.token {
            headers.push(("authorization", format!("Bearer {token}")));
        }
        let page = http.get_with(&self.search_url(), &headers).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse(&json, self.target, &self.spec.name, now)
    }
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn date(v: &Value, key: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s(v, key)?)
        .ok()
        .map(|d| d.to_utc())
}

/// Converts a search response into opportunities.
pub fn parse(
    json: &Value,
    target: Target,
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let items = json
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("missing `items` array".into()))?;
    Ok(items
        .iter()
        .filter_map(|item| match target {
            Target::Repositories => repository(item, source_name, now),
            Target::Issues => issue(item, source_name, now),
        })
        .collect())
}

fn base(item: &Value, title: &str, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let link = s(item, "html_url")?;
    let canonical = CanonicalUrl::parse(link).ok()?;
    let mut o = Opportunity::new(
        SourceKind::GitHub,
        source_name,
        link,
        &canonical,
        title,
        now,
    );
    o.category = Category::OpenSource;
    o.external_id = item.get("id").map(Value::to_string);
    Some(o)
}

fn repository(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let name = s(item, "full_name")?;
    let mut o = base(item, name, source_name, now)?;
    let stars = item
        .get("stargazers_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let issues = item
        .get("open_issues_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let language = s(item, "language");
    let license = item
        .get("license")
        .and_then(|l| s(l, "spdx_id"))
        .filter(|id| *id != "NOASSERTION");
    let updated = date(item, "pushed_at").or_else(|| date(item, "updated_at"));
    let mut facts = vec![format!("★ {stars}"), format!("{issues} open issues")];
    facts.extend(language.map(str::to_owned));
    facts.extend(license.map(|l| format!("license {l}")));
    if let Some(updated) = updated {
        facts.push(format!("updated {}", updated.format("%Y-%m-%d")));
    }
    let description = s(item, "description").unwrap_or_default();
    o.summary = Some(format!("{description} — {}", facts.join(" · ")));
    o.dates.published_at = updated;
    o.modality = sentri_core::Modality::Online;
    o.cost = sentri_core::Cost::Free;
    let mut tags: Vec<String> = item
        .get("topics")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    tags.extend(language.map(str::to_lowercase));
    o.tags = tags;
    Some(o)
}

fn issue(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    if item.get("pull_request").is_some() {
        return None;
    }
    let repo = s(item, "repository_url")
        .and_then(|u| u.split("/repos/").nth(1))
        .unwrap_or("github");
    let title = format!("[{repo}] {}", s(item, "title")?);
    let mut o = base(item, &title, source_name, now)?;
    o.summary = s(item, "body").map(str::to_owned);
    o.dates.published_at = date(item, "created_at");
    o.modality = sentri_core::Modality::Online;
    o.cost = sentri_core::Cost::Free;
    o.tags = item
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|l| s(l, "name"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(target: Option<&str>, query: Option<&str>) -> SourceSpec {
        SourceSpec {
            name: "gh".into(),
            kind: SourceKind::GitHub,
            url: None,
            query: query.map(str::to_owned),
            target: target.map(str::to_owned),
            category: None,
            tags: vec![],
            max_items: 20,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        }
    }

    #[test]
    fn builds_search_urls() {
        let gh = GitHub::new(spec(Some("issues"), Some("label:\"good first issue\""))).unwrap();
        let url = gh.search_url();
        assert_eq!(url.path(), "/search/issues");
        assert!(url.query().unwrap().contains("per_page=20"));
        assert!(GitHub::new(spec(Some("users"), Some("x"))).is_err());
        assert!(GitHub::new(spec(None, None)).is_err());
    }

    #[test]
    fn parses_repositories() {
        let body = json!({"items": [{
            "id": 7, "full_name": "rust-lang/rust", "html_url": "https://github.com/rust-lang/rust",
            "description": "Empowering everyone", "stargazers_count": 100_000, "open_issues_count": 9000,
            "language": "Rust", "license": {"spdx_id": "Apache-2.0"},
            "pushed_at": "2026-11-12T10:00:00Z", "topics": ["compiler"]
        }]});
        let items = parse(&body, Target::Repositories, "gh", Utc::now()).unwrap();
        let o = &items[0];
        assert_eq!(o.title, "rust-lang/rust");
        assert_eq!(o.category, Category::OpenSource);
        assert_eq!(o.external_id.as_deref(), Some("7"));
        assert!(o.summary.as_deref().unwrap().contains("★ 100000"));
        assert!(o.summary.as_deref().unwrap().contains("license Apache-2.0"));
        assert_eq!(o.tags, ["compiler", "rust"]);
    }

    #[test]
    fn parses_issues_and_skips_pull_requests() {
        let body = json!({"items": [
            {"id": 1, "title": "Fix typo", "html_url": "https://github.com/a/b/issues/1",
             "repository_url": "https://api.github.com/repos/a/b", "created_at": "2026-11-10T00:00:00Z",
             "labels": [{"name": "good first issue"}], "body": "Easy"},
            {"id": 2, "title": "PR", "html_url": "https://github.com/a/b/pull/2", "pull_request": {}}
        ]});
        let items = parse(&body, Target::Issues, "gh", Utc::now()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "[a/b] Fix typo");
        assert_eq!(items[0].tags, ["good first issue"]);
    }

    #[test]
    fn reports_malformed_responses() {
        assert!(
            parse(
                &json!({"message": "rate limited"}),
                Target::Issues,
                "gh",
                Utc::now()
            )
            .is_err()
        );
    }
}
