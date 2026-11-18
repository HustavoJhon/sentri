//! Get on Board connector (tech jobs in Latin America).
//!
//! Get on Board publishes a public, unauthenticated JSON API with the jobs
//! already visible on its site (<https://www.getonbrd.com/api/v0/>, see
//! <https://api-doc.getonbrd.com>). SENTRI reads one category per source and
//! asks the API to keep only jobs open to a country (`country_code`), so a
//! Peruvian user sees remote roles and roles located in Peru.
//!
//! Configuration:
//! - `target`: category slug (`programming`, `machine-learning-ai`,
//!   `data-science-analytics`, `sysadmin-devops-qa`…). Default `programming`.
//! - `query`: ISO country code. Default `PE`.
//! - `url`: optional full API endpoint, for other filters or a sandbox.
//!
//! Nothing is ever sent back: SENTRI never applies on your behalf.

use chrono::{DateTime, Utc};
use sentri_core::{CanonicalUrl, Category, Eligibility, Modality, Opportunity, SourceKind};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::html_to_text;

pub const API_BASE: &str = "https://www.getonbrd.com/api/v0/";
const DEFAULT_CATEGORY: &str = "programming";
const DEFAULT_COUNTRY: &str = "PE";
/// Largest page the API serves.
const MAX_PER_PAGE: usize = 100;

#[derive(Debug, Clone)]
pub struct GetOnBoard {
    pub(crate) spec: SourceSpec,
    endpoint: Url,
}

fn slug_ok(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl GetOnBoard {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let config = |e: String| SourceError::Config(format!("{}: {e}", spec.name));
        let endpoint =
            if let Some(url) = spec.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
                let url = Url::parse(url).map_err(|e| config(e.to_string()))?;
                let host = url.host_str().unwrap_or_default();
                let allowed = matches!(
                    host,
                    "www.getonbrd.com" | "getonbrd.com" | "sandbox.getonbrd.dev"
                );
                if !allowed || !url.path().starts_with("/api/v0/") {
                    return Err(config(format!(
                        "url must be a Get on Board API endpoint under {API_BASE}"
                    )));
                }
                url
            } else {
                let category = spec
                    .target
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .unwrap_or(DEFAULT_CATEGORY)
                    .to_ascii_lowercase();
                let country = spec
                    .query
                    .as_deref()
                    .map(str::trim)
                    .filter(|q| !q.is_empty())
                    .unwrap_or(DEFAULT_COUNTRY)
                    .to_ascii_uppercase();
                if !slug_ok(&category) {
                    return Err(config(format!("invalid category {category:?}")));
                }
                if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
                    return Err(config(format!(
                        "query must be a two-letter country code such as PE, not {country:?}"
                    )));
                }
                let mut url = Url::parse(API_BASE)
                    .and_then(|b| b.join(&format!("categories/{category}/jobs")))
                    .map_err(|e| config(e.to_string()))?;
                url.query_pairs_mut()
                    .append_pair("country_code", &country)
                    .append_pair("expand", "[\"company\"]");
                url
            };
        Ok(Self { spec, endpoint })
    }

    fn page_url(&self) -> Url {
        let mut url = self.endpoint.clone();
        if !url.query_pairs().any(|(k, _)| k == "per_page") {
            url.query_pairs_mut().append_pair(
                "per_page",
                &self.spec.max_items.clamp(1, MAX_PER_PAGE).to_string(),
            );
        }
        url
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.page_url()).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse_jobs(&json, &self.spec.name, self.country().as_deref(), now)
    }

    /// Country the API filtered by, when known.
    fn country(&self) -> Option<String> {
        self.endpoint
            .query_pairs()
            .find(|(k, _)| k == "country_code")
            .map(|(_, v)| v.to_ascii_uppercase())
    }
}

fn text<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Parses an API response (`{"data": [{"id", "attributes", "links"}]}`).
pub fn parse_jobs(
    json: &Value,
    source_name: &str,
    country: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let items = json
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("expected a \"data\" list".into()))?;
    Ok(items
        .iter()
        .filter_map(|item| job(item, source_name, country, now))
        .collect())
}

fn job(
    item: &Value,
    source_name: &str,
    country: Option<&str>,
    now: DateTime<Utc>,
) -> Option<Opportunity> {
    let a = item.get("attributes")?;
    let id = text(item, "id")?;
    let url = item
        .get("links")
        .and_then(|l| text(l, "public_url"))
        .map_or_else(
            || format!("https://www.getonbrd.com/jobs/{id}"),
            str::to_owned,
        );
    let canonical = CanonicalUrl::parse(&url).ok()?;
    let title = text(a, "title")?;
    let mut o = Opportunity::new(
        SourceKind::GetOnBoard,
        source_name,
        &url,
        &canonical,
        title,
        now,
    );
    o.external_id = Some(format!("getonboard:{id}"));
    o.category = match sentri_core::classify::category(title) {
        Some(Category::Internship) => Category::Internship,
        _ => Category::Job,
    };
    let company = a.get("company").and_then(|c| c.get("data"));
    let company_attrs = company.and_then(|c| c.get("attributes"));
    o.author.name = company_attrs
        .and_then(|c| text(c, "name"))
        .map(str::to_owned);
    o.author.profile_url = company_attrs
        .and_then(|c| text(c, "web"))
        .map(str::to_owned);
    o.author.avatar_url = company_attrs
        .and_then(|c| text(c, "logo"))
        .filter(|l| l.starts_with("https://"))
        .map(str::to_owned);
    let summary = [
        text(a, "description_headline"),
        text(a, "description"),
        text(a, "functions"),
    ]
    .into_iter()
    .flatten()
    .map(html_to_text)
    .collect::<Vec<_>>()
    .join("\n");
    o.summary = (!summary.trim().is_empty()).then_some(summary);
    o.requirements = text(a, "desirable").map(html_to_text);
    o.dates.published_at = a
        .get("published_at")
        .and_then(Value::as_i64)
        .and_then(|t| DateTime::from_timestamp(t, 0));
    o.modality = match (
        a.get("remote").and_then(Value::as_bool),
        text(a, "remote_modality"),
    ) {
        (_, Some(m)) if m.contains("hybrid") => Modality::Hybrid,
        (Some(true), _) => Modality::Online,
        (Some(false), _) => Modality::InPerson,
        _ => Modality::Unknown,
    };
    let countries: Vec<&str> = a
        .get("countries")
        .and_then(Value::as_array)
        .map(|c| c.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !countries.is_empty() {
        o.location = Some(countries.join(", "));
    }
    o.country = countries
        .iter()
        .find(|c| !c.eq_ignore_ascii_case("remote"))
        .map(|c| (*c).to_owned());
    // The API only returned jobs open to `country_code`.
    if country.is_some() {
        o.eligibility = Eligibility::Open;
    }
    let salary = match (
        a.get("min_salary").and_then(Value::as_i64),
        a.get("max_salary").and_then(Value::as_i64),
    ) {
        (Some(min), Some(max)) if min != max => Some(format!("USD {min}–{max} / month")),
        (Some(s), _) | (None, Some(s)) => Some(format!("USD {s} / month")),
        (None, None) => None,
    };
    o.perks.benefits = salary;
    o.tags = vec!["getonboard".into()];
    if let Some(category) = text(a, "category_name") {
        o.tags.push(category.to_lowercase());
    }
    if countries
        .iter()
        .any(|c| c.eq_ignore_ascii_case("peru") || c.eq_ignore_ascii_case("perú"))
    {
        o.tags.push("peru".into());
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-18T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn spec(url: Option<&str>, target: Option<&str>, query: Option<&str>) -> SourceSpec {
        SourceSpec {
            name: "getonboard".into(),
            kind: SourceKind::GetOnBoard,
            url: url.map(str::to_owned),
            query: query.map(str::to_owned),
            target: target.map(str::to_owned),
            category: None,
            tags: Vec::new(),
            max_items: 40,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        }
    }

    #[test]
    fn builds_the_category_endpoint_for_a_country() {
        let c = GetOnBoard::new(spec(None, None, None)).unwrap();
        let url = c.page_url();
        assert_eq!(url.path(), "/api/v0/categories/programming/jobs");
        let q: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        assert!(q.contains(&("country_code".into(), "PE".into())));
        assert!(q.contains(&("per_page".into(), "40".into())));
        let c = GetOnBoard::new(spec(None, Some("machine-learning-ai"), Some("cl"))).unwrap();
        assert!(
            c.page_url()
                .as_str()
                .contains("machine-learning-ai/jobs?country_code=CL")
        );
    }

    #[test]
    fn rejects_bad_configuration() {
        assert!(GetOnBoard::new(spec(None, Some("../admin"), None)).is_err());
        assert!(GetOnBoard::new(spec(None, None, Some("Peru"))).is_err());
        assert!(
            GetOnBoard::new(spec(Some("https://evil.example/api/v0/jobs"), None, None)).is_err()
        );
        assert!(GetOnBoard::new(spec(Some("https://www.getonbrd.com/jobs"), None, None)).is_err());
        assert!(
            GetOnBoard::new(spec(
                Some("https://www.getonbrd.com/api/v0/search/jobs?query=rust"),
                None,
                None
            ))
            .is_ok()
        );
    }

    #[test]
    fn maps_jobs_with_company_salary_and_modality() {
        let body = json!({"data": [{
            "id": "backend-developer-acme-lima",
            "type": "job",
            "attributes": {
                "title": "Backend Developer",
                "description_headline": "Who you are:",
                "description": "<ul><li>Rust &amp; Postgres</li></ul>",
                "desirable": "<p>Kubernetes</p>",
                "remote": false,
                "remote_modality": "no_remote",
                "countries": ["Peru"],
                "category_name": "Programming",
                "min_salary": 1500,
                "max_salary": 2200,
                "published_at": 1_794_841_752,
                "company": {"data": {"id": "acme", "type": "company", "attributes": {
                    "name": "Acme", "web": "https://acme.example", "logo": "https://cdn.example/acme.png"}}}
            },
            "links": {"public_url": "https://www.getonbrd.com/jobs/backend-developer-acme-lima"}
        }, {
            "id": "practicante-de-datos-beta-remote",
            "attributes": {"title": "Practicante de datos", "remote": true,
                "remote_modality": "remote_local", "countries": ["Remote"]}
        }, {"id": "broken"}]});
        let jobs = parse_jobs(&body, "getonboard", Some("PE"), now()).unwrap();
        assert_eq!(jobs.len(), 2);
        let j = &jobs[0];
        assert_eq!(j.source, SourceKind::GetOnBoard);
        assert_eq!(j.category, Category::Job);
        assert_eq!(j.author.name.as_deref(), Some("Acme"));
        assert_eq!(
            j.author.avatar_url.as_deref(),
            Some("https://cdn.example/acme.png")
        );
        assert_eq!(j.modality, Modality::InPerson);
        assert_eq!(j.country.as_deref(), Some("Peru"));
        assert_eq!(j.eligibility, Eligibility::Open);
        assert_eq!(j.perks.benefits.as_deref(), Some("USD 1500–2200 / month"));
        assert!(j.summary.as_deref().unwrap().contains("Rust & Postgres"));
        assert!(j.tags.contains(&"peru".to_owned()));
        assert!(j.dates.published_at.is_some());
        let intern = &jobs[1];
        assert_eq!(intern.category, Category::Internship);
        assert_eq!(intern.modality, Modality::Online);
        assert_eq!(
            intern.original_url,
            "https://www.getonbrd.com/jobs/practicante-de-datos-beta-remote"
        );
        assert!(intern.country.is_none());
    }

    #[test]
    fn rejects_unexpected_payloads() {
        assert!(parse_jobs(&json!({"error": "x"}), "g", None, now()).is_err());
    }
}
