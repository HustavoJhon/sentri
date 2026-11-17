//! Peruanos.dev connector.
//!
//! Peruanos.dev publishes a small, unauthenticated JSON API for the Peruvian
//! tech ecosystem (documented at <https://www.peruanos.dev/apis>). One source
//! instance selects one endpoint through `target`:
//!
//! - `events` → `GET /api/events` (optionally `?city=lima&type=presencial`)
//! - `communities` → `GET /api/communities`
//! - `projects` → `GET /api/projects` (GitHub repository objects)
//! - `cfs` → `GET /api/cfs` (Call for Speakers)
//!
//! The site is read-only for SENTRI: nothing is ever sent back.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, Utc};
use sentri_core::{Author, CanonicalUrl, Category, Modality, Opportunity, SourceKind};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::github;

/// Default public base; `url` may override it (tests, mirrors).
pub const DEFAULT_BASE: &str = "https://www.peruanos.dev";

/// Peru Standard Time (UTC-5, no daylight saving).
fn peru_offset() -> FixedOffset {
    FixedOffset::west_opt(5 * 3600).expect("UTC-5 is a valid offset")
}

/// The four public collections SENTRI reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Events,
    Communities,
    Projects,
    CallForSpeakers,
}

impl Target {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Communities => "communities",
            Self::Projects => "projects",
            Self::CallForSpeakers => "cfs",
        }
    }

    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("events" | "eventos" | "event") => Ok(Self::Events),
            Some("communities" | "community" | "comunidades") => Ok(Self::Communities),
            Some("projects" | "project" | "repositories" | "proyectos") => Ok(Self::Projects),
            Some("cfs" | "call_for_speakers" | "call-for-speakers" | "calls") => {
                Ok(Self::CallForSpeakers)
            }
            Some(other) => Err(format!(
                "target must be events, communities, projects or cfs, not {other:?}"
            )),
        }
    }
}

/// A configured Peruanos.dev collection.
#[derive(Debug, Clone)]
pub struct Peruanos {
    pub(crate) spec: SourceSpec,
    target: Target,
    base: Url,
}

impl Peruanos {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let target = Target::parse(spec.target.as_deref())
            .map_err(|e| SourceError::Config(format!("{}: {e}", spec.name)))?;
        let base = Url::parse(spec.url.as_deref().unwrap_or(DEFAULT_BASE))
            .map_err(|e| SourceError::Config(format!("{}: {e}", spec.name)))?;
        Ok(Self { spec, target, base })
    }

    /// Endpoint for this target. A `url` that already points at `/api/...` is
    /// used as-is (including its query filter); otherwise the API path is
    /// appended to the base.
    fn endpoint(&self) -> Url {
        let mut url = self.base.clone();
        if !url.path().contains("/api/") {
            let base = url.path().trim_end_matches('/');
            url.set_path(&format!("{base}/api/{}", self.target.slug()));
        }
        url.set_fragment(None);
        url
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.endpoint()).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse_items(&json, self.target, &self.spec.name, now)
    }

    /// Conditional fetch, so endpoints that publish `ETag`/`Last-Modified`
    /// can answer `304` and skip parsing and storage.
    pub async fn fetch_conditional(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
        validators: &crate::http::Validators,
    ) -> Result<(Vec<Opportunity>, crate::http::Validators), SourceError> {
        let page = http.get_conditional(&self.endpoint(), validators).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        let items = parse_items(&json, self.target, &self.spec.name, now)?;
        Ok((items, page.validators))
    }
}

/// Finds the item array: the API returns a bare array, but wrappers
/// (`{"items": [...]}`) are tolerated so schema changes do not break a run.
fn items_of(value: &Value, target: Target) -> Result<&[Value], SourceError> {
    if let Some(items) = value.as_array() {
        return Ok(items);
    }
    for key in [target.slug(), "items", "data", "results"] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            return Ok(items);
        }
    }
    Err(SourceError::Parse(format!(
        "expected a JSON array or an object with an `{}`/`items` array",
        target.slug()
    )))
}

/// Parses a complete API response.
pub fn parse_items(
    json: &Value,
    target: Target,
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let items = items_of(json, target)?;
    Ok(items
        .iter()
        .filter_map(|item| match target {
            Target::Events => event(item, source_name, now),
            Target::Communities => community(item, source_name, now),
            Target::Projects => project(item, source_name, now),
            Target::CallForSpeakers => call_for_speakers(item, source_name, now),
        })
        .collect())
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Parses `YYYY-MM-DD` plus optional `HH:MM` as Peru local time.
fn peru_datetime(date: &str, time: Option<&str>) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    let time = time
        .and_then(|t| NaiveTime::parse_from_str(t.trim(), "%H:%M").ok())
        .unwrap_or(NaiveTime::MIN);
    date.and_time(time)
        .and_local_timezone(peru_offset())
        .single()
        .map(|dt| dt.to_utc())
}

/// A date without a published time is treated as the end of that day, so a
/// submission stays open through its deadline day.
fn peru_end_of_day(date: &str) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").ok()?;
    date.and_hms_opt(23, 59, 59)?
        .and_local_timezone(peru_offset())
        .single()
        .map(|dt| dt.to_utc())
}

fn modality(kind: Option<&str>) -> Modality {
    match kind.map(|k| k.trim().to_ascii_lowercase()).as_deref() {
        Some("presencial" | "in-person" | "in_person" | "offline") => Modality::InPerson,
        Some("virtual" | "online" | "remoto" | "remote") => Modality::Online,
        Some("híbrido" | "hibrido" | "hybrid" | "mixto") => Modality::Hybrid,
        _ => Modality::Unknown,
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(80).collect()
}

fn join_location(venue: Option<&str>, city: Option<&str>) -> Option<String> {
    match (
        venue.map(str::trim).filter(|s| !s.is_empty()),
        city.map(str::trim).filter(|s| !s.is_empty()),
    ) {
        (Some(venue), Some(city))
            if !venue.eq_ignore_ascii_case(city)
                && !venue
                    .to_ascii_lowercase()
                    .contains(&city.to_ascii_lowercase()) =>
        {
            Some(format!("{venue}, {city}"))
        }
        (Some(venue), _) => Some(venue.to_owned()),
        (None, Some(city)) => Some(city.to_owned()),
        (None, None) => None,
    }
}

/// A base URL used when an item does not publish one. The query keeps the
/// identity unique instead of collapsing every item into one canonical URL.
fn fallback_url(section: &str, key: &str) -> String {
    format!("{DEFAULT_BASE}/{section}?item={key}")
}

fn valid_url(value: Option<&str>) -> Option<&str> {
    value.filter(|url| CanonicalUrl::parse(url).is_ok())
}

fn event(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let title = text(item, "title")?;
    let date = text(item, "date");
    let city = text(item, "city");
    let registration = valid_url(text(item, "registration_url"));
    let original = registration.map_or_else(
        || {
            fallback_url(
                "events",
                &slugify(&format!("{title}-{}", date.unwrap_or(""))),
            )
        },
        str::to_owned,
    );
    let canonical = CanonicalUrl::parse(&original).ok()?;
    let mut o = Opportunity::new(
        SourceKind::Peruanos,
        source_name,
        &original,
        &canonical,
        title,
        now,
    );
    o.external_id = Some(format!(
        "event:{}-{}",
        date.unwrap_or("undated"),
        slugify(title)
    ));
    let haystack = format!("{title} {}", text(item, "description").unwrap_or_default());
    o.category = sentri_core::classify::category(&haystack).unwrap_or(Category::Event);
    o.summary = text_at(item, "description");
    o.image_url = text(item, "image_url").map(str::to_owned);
    o.location = join_location(text(item, "location"), city);
    o.country = Some("Peru".into());
    o.modality = modality(text(item, "type"));
    o.dates.starts_at = date.and_then(|d| peru_datetime(d, text(item, "time")));
    o.author.name = text(item, "organizer").map(str::to_owned);
    o.tags = strings(item, "tags");
    o.tags.push("peru".into());
    Some(o)
}

fn text_at(value: &Value, key: &str) -> Option<String> {
    text(value, key).map(str::to_owned)
}

fn community(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let name = text(item, "name")?;
    let contact = item.get("contact");
    let website = contact.and_then(|c| text(c, "website"));
    let original = valid_url(website).map_or_else(
        || {
            format!(
                "{DEFAULT_BASE}/community/{}",
                slugify(text(item, "name").unwrap_or("community"))
            )
        },
        str::to_owned,
    );
    let canonical = CanonicalUrl::parse(&original).ok()?;
    let mut o = Opportunity::new(
        SourceKind::Peruanos,
        source_name,
        &original,
        &canonical,
        name,
        now,
    );
    o.external_id = Some(format!("community:{}", slugify(name)));
    o.category = Category::Community;
    o.location = join_location(None, text(item, "city"));
    o.country = Some("Peru".into());
    o.image_url = text(item, "logo_url").map(str::to_owned);
    o.tags = strings(item, "topics");
    o.tags.push("community".into());
    o.tags.push("peru".into());
    o.author = Author {
        name: Some(name.to_owned()),
        profile_url: valid_url(website).map(str::to_owned),
        ..Author::default()
    };
    o.summary = community_summary(item, contact);
    Some(o)
}

/// Description plus the contact links the directory publishes. Empty social
/// fields are treated as absent.
fn community_summary(item: &Value, contact: Option<&Value>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(description) = text(item, "description") {
        parts.push(description.to_owned());
    }
    if let Some(contact) = contact {
        for (label, key) in [
            ("Website", "website"),
            ("Email", "email"),
            ("GitHub", "github"),
            ("X", "twitter"),
            ("LinkedIn", "linkedin"),
            ("Discord", "discord"),
            ("Facebook", "facebook"),
            ("YouTube", "youtube"),
            ("Instagram", "instagram"),
        ] {
            let value = if key == "email" {
                text(contact, "email")
            } else {
                contact
                    .get("socialMedia")
                    .and_then(|s| text(s, key))
                    .or_else(|| text(contact, key))
            };
            if let Some(value) = value {
                parts.push(format!("{label}: {value}"));
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn call_for_speakers(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let title = text(item, "title")?;
    let id = item.get("id").and_then(|id| match id {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    });
    let website = valid_url(text(item, "website_url"));
    let submission = valid_url(text(item, "cfs_url"));
    let fallback_id = slugify(title);
    let original = submission.or(website).map_or_else(
        || {
            format!(
                "{DEFAULT_BASE}/cfs?item={}",
                id.as_deref().unwrap_or(fallback_id.as_str())
            )
        },
        str::to_owned,
    );
    let canonical = CanonicalUrl::parse(&original).ok()?;
    let mut o = Opportunity::new(
        SourceKind::Peruanos,
        source_name,
        &original,
        &canonical,
        title,
        now,
    );
    o.external_id = id.or_else(|| Some(format!("cfs:{}", slugify(title))));
    o.category = Category::CallForSpeakers;
    o.summary = text_at(item, "description");
    o.image_url = text(item, "image_url").map(str::to_owned);
    o.location = join_location(text(item, "location"), text(item, "city"));
    o.country = Some("Peru".into());
    o.modality = modality(text(item, "type"));
    o.dates.deadline = text(item, "deadline").and_then(peru_end_of_day);
    o.dates.starts_at = text(item, "event_date").and_then(|d| peru_datetime(d, None));
    o.author = Author {
        name: text(item, "community").map(str::to_owned),
        profile_url: website.map(str::to_owned),
        ..Author::default()
    };
    o.tags = strings(item, "topics");
    if let Some(status) = text(item, "status") {
        o.tags.push(status.replace('_', " "));
    }
    o.tags.push("call for speakers".into());
    o.tags.push("peru".into());
    Some(o)
}

fn project(item: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let mut o = github::repository(item, SourceKind::Peruanos, source_name, now)?;
    o.author.name = item
        .get("owner")
        .and_then(|owner| text(owner, "login"))
        .map(str::to_owned)
        .or(o.author.name);
    o.author.profile_url = item
        .get("owner")
        .and_then(|owner| text(owner, "html_url"))
        .map(str::to_owned);
    o.author.avatar_url = item
        .get("owner")
        .and_then(|owner| text(owner, "avatar_url"))
        .map(str::to_owned);
    o.tags.push("peru".into());
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-17T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    #[test]
    fn builds_endpoints_from_a_base_or_a_full_url() {
        let spec = |url: Option<&str>, target: Option<&str>| SourceSpec {
            name: "peruanos".into(),
            kind: SourceKind::Peruanos,
            url: url.map(str::to_owned),
            query: None,
            target: target.map(str::to_owned),
            category: None,
            tags: vec![],
            max_items: 50,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        };
        let c = Peruanos::new(spec(None, Some("events"))).unwrap();
        assert_eq!(c.endpoint().as_str(), "https://www.peruanos.dev/api/events");
        let c = Peruanos::new(spec(
            Some("https://www.peruanos.dev/api/cfs?status=open"),
            None,
        ))
        .unwrap();
        assert_eq!(
            c.endpoint().as_str(),
            "https://www.peruanos.dev/api/cfs?status=open"
        );
        assert!(
            Peruanos::new(spec(None, Some("nope")))
                .unwrap_err()
                .to_string()
                .contains("target must be events, communities, projects or cfs")
        );
    }

    #[test]
    fn parses_events_and_never_discards_missing_optional_fields() {
        let body = json!([{
            "title": "DevFest Cusco 2026",
            "description": "Conferencia de la comunidad",
            "date": "2026-12-05",
            "time": "09:00",
            "location": "Universidad Andina",
            "city": "Cusco",
            "type": "Presencial",
            "tags": ["IA", "Web"],
            "organizer": "GDG Cusco",
            "registration_url": "https://gdg.community.dev/events/details/devfest-cusco/"
        }, {
            "title": "Charla sin fecha",
            "date": "not-a-date"
        }]);
        let items = parse_items(&body, Target::Events, "peruanos", now()).unwrap();
        assert_eq!(items.len(), 2);
        let o = &items[0];
        assert_eq!(o.source, SourceKind::Peruanos);
        assert_eq!(o.category, Category::Conference);
        assert_eq!(o.modality, Modality::InPerson);
        assert_eq!(o.location.as_deref(), Some("Universidad Andina, Cusco"));
        assert_eq!(o.country.as_deref(), Some("Peru"));
        assert_eq!(
            o.dates.starts_at.unwrap().to_rfc3339(),
            "2026-12-05T14:00:00+00:00"
        );
        assert_eq!(o.author.name.as_deref(), Some("GDG Cusco"));
        assert!(o.tags.contains(&"peru".to_owned()));
        assert_eq!(
            o.original_url,
            "https://gdg.community.dev/events/details/devfest-cusco/"
        );
        let undated = &items[1];
        assert!(
            undated.dates.starts_at.is_none(),
            "invalid date is not invented"
        );
        assert!(undated.original_url.contains("item=charla-sin-fecha"));
    }

    #[test]
    fn parses_communities_and_skips_empty_contacts() {
        let body = json!([{
            "name": "GDG Cusco",
            "description": "Comunidad de desarrolladores",
            "city": "Cusco",
            "topics": ["Cloud", "Android"],
            "logo_url": "https://example.com/logo.png",
            "contact": {
                "email": "",
                "website": "https://gdg.community.dev/gdg-cusco/",
                "socialMedia": {"github": "https://github.com/gdg-cusco", "discord": ""}
            }
        }, {
            "name": "Sin contacto"
        }]);
        let items = parse_items(&body, Target::Communities, "peruanos", now()).unwrap();
        assert_eq!(items.len(), 2);
        let o = &items[0];
        assert_eq!(o.category, Category::Community);
        assert_eq!(o.location.as_deref(), Some("Cusco"));
        assert_eq!(o.tags, ["Cloud", "Android", "community", "peru"]);
        let summary = o.summary.as_deref().unwrap();
        assert!(
            summary.contains("GitHub: https://github.com/gdg-cusco"),
            "{summary}"
        );
        assert!(!summary.contains("Discord"), "empty links are absent");
        assert_eq!(o.author.name.as_deref(), Some("GDG Cusco"));
        assert_eq!(
            o.author.profile_url.as_deref(),
            Some("https://gdg.community.dev/gdg-cusco/")
        );
        let fallback = &items[1];
        assert!(fallback.original_url.ends_with("/community/sin-contacto"));
    }

    #[test]
    fn parses_calls_for_speakers_with_deadline_and_event_date() {
        let body = json!([{
            "id": "cfs-devfest-lima-2026",
            "title": "DevFest Lima 2026 - Call for Speakers",
            "community": "GDG Lima",
            "description": "Buscamos ponentes",
            "event_date": "2026-11-14",
            "deadline": "2026-10-03",
            "cfs_url": "https://sessionize.com/devfestlima2026",
            "website_url": "https://devfest.gdglima.com/",
            "location": "Lima, Perú",
            "city": "Lima",
            "type": "Presencial",
            "topics": ["IA", "Android"],
            "status": "closing_soon"
        }]);
        let items = parse_items(&body, Target::CallForSpeakers, "peruanos", now()).unwrap();
        let o = &items[0];
        assert_eq!(o.category, Category::CallForSpeakers);
        assert_eq!(o.external_id.as_deref(), Some("cfs-devfest-lima-2026"));
        assert_eq!(o.original_url, "https://sessionize.com/devfestlima2026");
        assert_eq!(o.author.name.as_deref(), Some("GDG Lima"));
        assert_eq!(
            o.author.profile_url.as_deref(),
            Some("https://devfest.gdglima.com/")
        );
        assert_eq!(
            o.dates.deadline.unwrap().to_rfc3339(),
            "2026-10-04T04:59:59+00:00",
            "deadline stays open through the day in Peru"
        );
        assert_eq!(
            o.dates.starts_at.unwrap().to_rfc3339(),
            "2026-11-14T05:00:00+00:00"
        );
        assert!(o.tags.contains(&"closing soon".to_owned()));
    }

    #[test]
    fn parses_github_repositories_and_keeps_their_identity() {
        let body = json!([{
            "id": 7,
            "full_name": "lperezp/peruanos.dev",
            "html_url": "https://github.com/lperezp/peruanos.dev",
            "description": "Directorio tech peruano",
            "stargazers_count": 42,
            "open_issues_count": 3,
            "language": "TypeScript",
            "topics": ["events"],
            "owner": {"login": "lperezp", "html_url": "https://github.com/lperezp",
                      "avatar_url": "https://avatars.example/lperezp.png"},
            "pushed_at": "2026-11-12T10:00:00Z"
        }]);
        let items = parse_items(&body, Target::Projects, "peruanos", now()).unwrap();
        let o = &items[0];
        assert_eq!(o.source, SourceKind::Peruanos);
        assert_eq!(o.category, Category::OpenSource);
        assert_eq!(o.canonical_url, "https://github.com/lperezp/peruanos.dev");
        assert_eq!(o.author.name.as_deref(), Some("lperezp"));
        assert!(o.tags.contains(&"peru".to_owned()));
    }

    #[test]
    fn accepts_wrapped_responses_and_reports_unexpected_shapes() {
        let wrapped = json!({"data": [{"title": "X", "date": "2026-12-01"}]});
        assert_eq!(
            parse_items(&wrapped, Target::Events, "p", now())
                .unwrap()
                .len(),
            1
        );
        assert!(parse_items(&json!({"error": "nope"}), Target::Events, "p", now()).is_err());
        assert!(parse_items(&json!("nope"), Target::Events, "p", now()).is_err());
    }

    #[test]
    fn empty_responses_are_valid() {
        assert_eq!(
            parse_items(&json!([]), Target::Communities, "p", now())
                .unwrap()
                .len(),
            0
        );
    }
}
