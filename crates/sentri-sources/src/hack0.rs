//! hack0.dev connector.
//!
//! hack0.dev is a Next.js directory of tech events in Latin America. It has no
//! public JSON feed: `/api/` is disallowed by `robots.txt`. What *is* published
//! for public consumption is the HTML listing (`/eventos`, `/ciudad/<city>`)
//! and one schema.org `Event` JSON-LD block per event page. This connector
//! reads only those pages, at most a handful per run, and keeps just events
//! located in Peru.

use chrono::{DateTime, Duration, Utc};
use sentri_core::{CanonicalUrl, Category, Cost, Modality, Opportunity, SourceKind};
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::{self, Structured, StructuredKind};

/// Event pages fetched per run, to stay polite.
pub const MAX_EVENT_PAGES: usize = 10;

pub const DEFAULT_LISTING: &str = "https://hack0.dev/eventos";

/// Peruvian cities used when a page omits `addressCountry`.
const PERU_CITIES: &[&str] = &[
    "lima",
    "callao",
    "cusco",
    "cuzco",
    "arequipa",
    "piura",
    "trujillo",
    "chimbote",
    "huancayo",
    "ica",
    "tacna",
    "ayacucho",
    "puno",
    "juliaca",
    "sullana",
    "talara",
    "cajamarca",
    "huanuco",
    "huánuco",
    "moquegua",
    "tumbes",
    "abancay",
    "huaraz",
    "barranco",
    "miraflores",
    "san isidro",
    "surco",
    "san borja",
    "la molina",
    "villa el salvador",
];

#[derive(Debug, Clone)]
pub struct Hack0 {
    pub(crate) spec: SourceSpec,
    listing: Url,
}

impl Hack0 {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let listing = Url::parse(
            spec.url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .unwrap_or(DEFAULT_LISTING),
        )
        .map_err(|e| SourceError::Config(format!("{}: {e}", spec.name)))?;
        Ok(Self { spec, listing })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.listing).await?;
        self.fetch_pages(http, &page.text(), now).await
    }

    pub async fn fetch_conditional(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
        validators: &crate::http::Validators,
    ) -> Result<(Vec<Opportunity>, crate::http::Validators), SourceError> {
        let page = http.get_conditional(&self.listing, validators).await?;
        let items = self.fetch_pages(http, &page.text(), now).await?;
        Ok((items, page.validators))
    }

    async fn fetch_pages(
        &self,
        http: &SafeClient,
        listing: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let mut items = Vec::new();
        for link in event_links(listing, &self.listing)
            .into_iter()
            .take(MAX_EVENT_PAGES.min(self.spec.max_items))
        {
            match http.get(&link).await {
                Ok(page) => {
                    if let Some(o) = from_event_page(&page.text(), &link, &self.spec.name, now) {
                        items.push(o);
                    }
                }
                Err(error) => tracing::debug!(%error, url = %link, "skipping hack0 event page"),
            }
        }
        Ok(items)
    }
}

/// Event detail links on a listing page (same host only), deduplicated.
pub fn event_links(html: &str, base: &Url) -> Vec<Url> {
    let mut out: Vec<Url> = Vec::new();
    for part in html.split("href=\"").skip(1) {
        let Some(href) = part.split('"').next() else {
            continue;
        };
        let path = href.split(['?', '#']).next().unwrap_or_default();
        if !path.starts_with("/eventos/") || path.trim_end_matches('/') == "/eventos" {
            continue;
        }
        let Ok(url) = base.join(path) else {
            continue;
        };
        if url.host_str() == base.host_str()
            && url.path().trim_end_matches('/') != "/eventos"
            && !out.contains(&url)
        {
            out.push(url);
        }
    }
    out
}

fn contains_peru(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("peru")
        || text.contains("perú")
        || PERU_CITIES.iter().any(|city| {
            text.split(|c: char| !c.is_alphanumeric())
                .any(|word| word == *city)
        })
}

/// Whether the declared place is in Peru. A missing country is accepted only
/// when the city is a known Peruvian one, so LATAM events are not imported.
pub fn is_peru(s: &Structured) -> bool {
    match s.country.as_deref().map(str::trim) {
        Some(country) if !country.is_empty() => {
            let country = country.to_ascii_lowercase();
            country == "pe"
                || country == "per"
                || country.contains("peru")
                || country.contains("perú")
        }
        _ => s.location.as_deref().is_some_and(contains_peru),
    }
}

/// Builds an opportunity from an event page's schema.org data.
pub fn from_event_page(
    html: &str,
    page_url: &Url,
    source_name: &str,
    now: DateTime<Utc>,
) -> Option<Opportunity> {
    let meta = metadata::extract(html, page_url);
    let s = meta.structured;
    if s.kind != Some(StructuredKind::Event) || !is_peru(&s) {
        return None;
    }
    let starts = s.starts_at?;
    if s.ends_at.unwrap_or(starts) < now - Duration::hours(12) {
        return None;
    }
    // Registration pages (Luma, Eventbrite…) are the identity other sources
    // also publish, so prefer them for deduplication.
    let declared = s
        .offer_url
        .as_deref()
        .or(s.url.as_deref())
        .filter(|url| CanonicalUrl::parse(url).is_ok());
    let original = declared.map_or_else(|| page_url.as_str().to_owned(), str::to_owned);
    let canonical = CanonicalUrl::parse(&original)
        .or_else(|_| CanonicalUrl::parse(page_url.as_str()))
        .ok()?;
    let title = s.name.clone().or(meta.title)?;
    let mut o = Opportunity::new(
        SourceKind::Hack0,
        source_name,
        &original,
        &canonical,
        &title,
        now,
    );
    o.external_id = Some(format!(
        "hack0:{}",
        page_url
            .path_segments()
            .and_then(|mut p| p.next_back())
            .unwrap_or("event")
    ));
    o.category = if s.hackathon {
        Category::Hackathon
    } else {
        sentri_core::classify::category(&format!(
            "{title} {}",
            s.description.clone().unwrap_or_default()
        ))
        .unwrap_or(Category::Event)
    };
    o.summary = s.description.clone().or(meta.description);
    o.image_url = meta.image.map(String::from);
    o.dates.starts_at = Some(starts);
    o.dates.ends_at = s.ends_at;
    o.location = s.location;
    o.country = s.country.or_else(|| Some("Peru".into()));
    o.modality = s.modality.unwrap_or(Modality::Unknown);
    o.cost = match s.free {
        Some(true) => Cost::Free,
        Some(false) => Cost::Paid(s.price),
        None => Cost::Unknown,
    };
    o.author.name = s.organization.or_else(|| meta.site_name.clone());
    o.tags = vec!["hack0".into(), "peru".into()];
    o.tags.extend(meta.keywords);
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-17T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn base() -> Url {
        Url::parse("https://hack0.dev/eventos").unwrap()
    }

    #[test]
    fn finds_event_links_only_on_the_listing_host() {
        let html = r#"<a href="/eventos/bvc-forum-peru-2026-abc">x</a>
            <a href="/eventos/bvc-forum-peru-2026-abc">dup</a>
            <a href="/eventos">self</a>
            <a href="https://evil.example/eventos/nope">other</a>
            <a href="/ciudad/lima">city</a>"#;
        let links = event_links(html, &base());
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].as_str(),
            "https://hack0.dev/eventos/bvc-forum-peru-2026-abc"
        );
    }

    #[test]
    fn reads_peruvian_events_and_skips_other_countries() {
        let page = |country: &str, city: &str, start: &str| {
            format!(
                r#"<script type="application/ld+json">{{"@type":"Event","name":"BVC Forum",
                "startDate":"{start}","eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode",
                "location":{{"@type":"Place","name":"{city}","address":{{"addressLocality":"{city}","addressCountry":"{country}"}}}},
                "offers":{{"@type":"Offer","url":"https://luma.com/9qy5vcu8"}},
                "organizer":{{"@type":"Organization","name":"B Venture Capital"}}}}</script>"#
            )
        };
        let url = Url::parse("https://hack0.dev/eventos/bvc-forum-peru-2026-abc").unwrap();
        let o = from_event_page(
            &page("PE", "Lima", "2026-11-18T21:00:00Z"),
            &url,
            "hack0",
            now(),
        )
        .unwrap();
        assert_eq!(o.source, SourceKind::Hack0);
        assert_eq!(o.title, "BVC Forum");
        assert_eq!(o.original_url, "https://luma.com/9qy5vcu8");
        assert!(o.tags.contains(&"peru".to_owned()));
        assert_eq!(o.author.name.as_deref(), Some("B Venture Capital"));
        assert!(
            from_event_page(
                &page("CL", "Santiago", "2026-11-18T21:00:00Z"),
                &url,
                "h",
                now()
            )
            .is_none()
        );
        assert!(
            from_event_page(
                &page("PE", "Lima", "2026-11-01T21:00:00Z"),
                &url,
                "h",
                now()
            )
            .is_none(),
            "past events are skipped"
        );
    }

    #[test]
    fn accepts_a_known_city_when_the_country_is_missing() {
        let html = r#"<script type="application/ld+json">{"@type":"Event","name":"Meetup",
            "startDate":"2026-12-01T19:00:00Z",
            "location":{"@type":"Place","name":"Cusco"}}</script>"#;
        let url = Url::parse("https://hack0.dev/eventos/meetup-cusco").unwrap();
        let o = from_event_page(html, &url, "hack0", now()).unwrap();
        assert_eq!(o.country.as_deref(), Some("Peru"));
        assert_eq!(o.location.as_deref(), Some("Cusco"));
        assert_eq!(o.original_url, "https://hack0.dev/eventos/meetup-cusco");
    }

    #[test]
    fn ignores_pages_without_event_data() {
        let url = Url::parse("https://hack0.dev/eventos/x").unwrap();
        assert!(from_event_page("<html>nothing</html>", &url, "h", now()).is_none());
    }
}
