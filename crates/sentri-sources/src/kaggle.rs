//! Kaggle competitions through the official API
//! (`GET /api/v1/competitions/list`), which requires credentials.
//!
//! The token variable (`token_env`) holds either `username:key` (from
//! `kaggle.json`, sent as HTTP Basic) or an API token (sent as Bearer).
//! Credentials go only to the configured API origin (redirects are not
//! followed for authenticated requests).

use chrono::{DateTime, Utc};
use sentri_core::{Author, CanonicalUrl, Category, Cost, Modality, Opportunity, SourceKind};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};

const API: &str = "https://www.kaggle.com/api/v1/competitions/list";
const SITE: &str = "https://www.kaggle.com/competitions/";

#[derive(Debug, Clone)]
pub struct Kaggle {
    pub(crate) spec: SourceSpec,
    url: Url,
}

/// Minimal base64 (standard alphabet, padded) for the Basic header.
fn base64(input: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Authorization header value for the configured credential.
pub fn authorization(credential: &str) -> String {
    let credential = credential.trim();
    if credential.contains(':') {
        format!("Basic {}", base64(credential.as_bytes()))
    } else {
        format!("Bearer {credential}")
    }
}

impl Kaggle {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        if spec.token.is_none() {
            return Err(SourceError::Config(format!(
                "{}: Kaggle needs credentials; set token_env to a variable holding \"username:key\" or an API token",
                spec.name
            )));
        }
        let mut url = Url::parse(spec.url.as_deref().unwrap_or(API))
            .map_err(|e| SourceError::Config(e.to_string()))?;
        if url.query().is_none() {
            url.query_pairs_mut()
                .append_pair("sortBy", "latestDeadline");
            if let Some(q) = spec.query.as_deref().filter(|q| !q.trim().is_empty()) {
                url.query_pairs_mut().append_pair("search", q);
            }
        }
        Ok(Self { spec, url })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let token = self.spec.token.as_deref().unwrap_or_default();
        let headers = [("authorization", authorization(token))];
        let page = http.get_with(&self.url, &headers).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse(&json, &self.spec.name, now)
    }
}

pub fn parse(
    json: &Value,
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let list = json
        .as_array()
        .ok_or_else(|| SourceError::Parse("expected a list of competitions".into()))?;
    Ok(list
        .iter()
        .filter_map(|c| competition(c, source_name, now))
        .filter(|o| o.dates.deadline.is_none_or(|d| d >= now))
        .collect())
}

fn competition(c: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let s = |k: &str| {
        c.get(k)
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
    };
    let reference = s("ref")?;
    let link = if reference.starts_with("https://") {
        reference.to_owned()
    } else {
        format!("{SITE}{}", reference.trim_start_matches('/'))
    };
    let canonical = CanonicalUrl::parse(&link).ok()?;
    let mut o = Opportunity::new(
        SourceKind::Kaggle,
        source_name,
        &link,
        &canonical,
        s("title")?,
        now,
    );
    o.category = Category::Competition;
    o.external_id = c.get("id").map(Value::to_string);
    o.summary = s("description").map(str::to_owned);
    o.modality = Modality::Online;
    o.cost = Cost::Free;
    let date = |k: &str| {
        s(k).and_then(|d| DateTime::parse_from_rfc3339(d).ok())
            .map(|d| d.to_utc())
    };
    o.dates.deadline = date("deadline");
    o.dates.starts_at = date("enabledDate");
    if let Some(reward) = s("reward")
        .filter(|r| !r.eq_ignore_ascii_case("knowledge") && !r.eq_ignore_ascii_case("kudos"))
    {
        o.perks.prizes = Some(reward.to_owned());
    }
    o.author = Author {
        name: s("organizationName").map(str::to_owned),
        ..Author::default()
    };
    let mut tags: Vec<String> = c
        .get("tags")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(|x| x.get("name")?.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    tags.extend(s("category").map(str::to_lowercase));
    o.tags = tags;
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_basic_or_bearer_auth() {
        assert_eq!(authorization("user:key"), "Basic dXNlcjprZXk=");
        assert_eq!(authorization("KGAT_abc"), "Bearer KGAT_abc");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"a"), "YQ==");
    }

    #[test]
    fn requires_credentials() {
        let spec = SourceSpec {
            name: "kaggle".into(),
            kind: SourceKind::Kaggle,
            url: None,
            query: None,
            target: None,
            category: None,
            tags: vec![],
            max_items: 20,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        };
        assert!(matches!(Kaggle::new(spec), Err(SourceError::Config(_))));
    }

    #[test]
    fn maps_competitions_and_skips_closed_ones() {
        let now = Utc::now();
        let future = (now + chrono::Duration::days(30)).to_rfc3339();
        let past = (now - chrono::Duration::days(3)).to_rfc3339();
        let body = json!([
            {"id": 1, "ref": "https://www.kaggle.com/competitions/llm-arena", "title": "LLM Arena",
             "description": "Predict preferences", "deadline": future, "category": "Featured",
             "reward": "$100,000", "organizationName": "Google", "tags": [{"name": "nlp"}]},
            {"id": 2, "ref": "titanic", "title": "Titanic", "deadline": future, "reward": "Knowledge"},
            {"id": 3, "ref": "old", "title": "Closed", "deadline": past}
        ]);
        let items = parse(&body, "kaggle", now).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].perks.prizes.as_deref(), Some("$100,000"));
        assert_eq!(items[0].tags, ["nlp", "featured"]);
        assert_eq!(
            items[1].original_url,
            "https://www.kaggle.com/competitions/titanic"
        );
        assert_eq!(
            items[1].perks.prizes, None,
            "knowledge competitions have no prize"
        );
        assert!(parse(&json!({"code": 401}), "k", now).is_err());
    }
}
