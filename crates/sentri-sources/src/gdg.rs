//! Google Developer Groups (Bevy platform, `gdg.community.dev`).
//!
//! Bevy has no public API or chapter feed, and `robots.txt` disallows
//! `/api/`. Chapter and event pages are public and each event page publishes
//! a schema.org `Event` in JSON-LD, which is what this connector reads:
//! one chapter page plus at most a few event pages per run.

use chrono::{DateTime, Duration, Utc};
use sentri_core::{CanonicalUrl, Category, Cost, Modality, Opportunity, SourceKind};
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::{self, StructuredKind};

/// Event pages fetched per run, to stay polite.
pub const MAX_EVENT_PAGES: usize = 10;

#[derive(Debug, Clone)]
pub struct Gdg {
    pub(crate) spec: SourceSpec,
    chapter: Url,
}

impl Gdg {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let url = spec.url.as_deref().ok_or_else(|| {
            SourceError::Config(format!(
                "{}: `url` must be the chapter page, e.g. https://gdg.community.dev/gdg-cusco/",
                spec.name
            ))
        })?;
        let chapter = Url::parse(url).map_err(|e| SourceError::Config(e.to_string()))?;
        Ok(Self { spec, chapter })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.chapter).await?;
        let links = event_links(&page.text(), &self.chapter);
        if links.is_empty() {
            return Ok(Vec::new());
        }
        let mut items = Vec::new();
        for link in links
            .into_iter()
            .take(MAX_EVENT_PAGES.min(self.spec.max_items))
        {
            match http.get(&link).await {
                Ok(page) => {
                    if let Some(o) = from_event_page(&page.text(), &link, &self.spec.name, now) {
                        items.push(o);
                    }
                }
                Err(error) => tracing::debug!(%error, url = %link, "skipping GDG event page"),
            }
        }
        Ok(items)
    }
}

/// Event detail links on the chapter page (same host only), in page order.
pub fn event_links(html: &str, base: &Url) -> Vec<Url> {
    let mut out: Vec<Url> = Vec::new();
    for part in html.split("href=\"").skip(1) {
        let Some(href) = part.split('"').next() else {
            continue;
        };
        if !href.contains("/events/details/") {
            continue;
        }
        let Ok(url) = base.join(href) else {
            continue;
        };
        if url.host_str() == base.host_str() && !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

/// Builds an opportunity from an event page's schema.org data. Pages
/// without an `Event` or already finished events yield nothing.
pub fn from_event_page(
    html: &str,
    url: &Url,
    source_name: &str,
    now: DateTime<Utc>,
) -> Option<Opportunity> {
    let meta = metadata::extract(html, url);
    let s = meta.structured;
    if s.kind != Some(StructuredKind::Event) {
        return None;
    }
    let starts = s.starts_at?;
    if s.ends_at.unwrap_or(starts) < now - Duration::hours(12) {
        return None;
    }
    let canonical = CanonicalUrl::parse(url.as_str()).ok()?;
    let title = s.name.clone().or(meta.title)?;
    let mut o = Opportunity::new(
        SourceKind::Gdg,
        source_name,
        url.as_str(),
        &canonical,
        &title,
        now,
    );
    o.category = Category::Event;
    o.summary = meta.description.or(s.description);
    o.image_url = meta.image.map(String::from);
    o.dates.starts_at = Some(starts);
    o.dates.ends_at = s.ends_at;
    o.location = s.location;
    o.country = s.country;
    o.modality = s.modality.unwrap_or(Modality::Unknown);
    o.cost = match s.free {
        Some(true) => Cost::Free,
        Some(false) => Cost::Paid(s.price),
        None => Cost::Unknown,
    };
    o.author.name = s.organization.or_else(|| meta.site_name.clone());
    o.tags = vec!["gdg".into()];
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-16T00:00:00Z")
            .unwrap()
            .to_utc()
    }

    #[test]
    fn finds_event_links_on_the_same_host() {
        let base = Url::parse("https://gdg.community.dev/gdg-cusco/").unwrap();
        let html = r#"<a href="/events/details/google-gdg-cusco-presents-devfest/">x</a>
            <a href="/events/details/google-gdg-cusco-presents-devfest/">dup</a>
            <a href="https://evil.example/events/details/x/">other host</a>
            <a href="/gdg-cusco/about/">about</a>"#;
        let links = event_links(html, &base);
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].as_str(),
            "https://gdg.community.dev/events/details/google-gdg-cusco-presents-devfest/"
        );
    }

    #[test]
    fn reads_schema_org_events_and_skips_past_ones() {
        let url = Url::parse("https://gdg.community.dev/events/details/devfest/").unwrap();
        let page = |start: &str| {
            format!(
                r#"<meta property="og:description" content="DevFest Cusco">
                <script type="application/ld+json">{{"@type":"Event","name":"DevFest Cusco 2026",
                "startDate":"{start}","eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode",
                "location":{{"@type":"Place","name":"Universidad Andina","address":{{"addressLocality":"Cusco","addressCountry":"PE"}}}},
                "offers":{{"price":"0"}},"organizer":{{"name":"GDG Cusco"}}}}</script>"#
            )
        };
        let o =
            from_event_page(&page("2026-12-05T09:00:00-05:00"), &url, "gdg-cusco", now()).unwrap();
        assert_eq!(o.title, "DevFest Cusco 2026");
        assert_eq!(o.source, SourceKind::Gdg);
        assert_eq!(o.modality, Modality::InPerson);
        assert_eq!(o.cost, Cost::Free);
        assert_eq!(o.author.name.as_deref(), Some("GDG Cusco"));
        assert_eq!(o.location.as_deref(), Some("Universidad Andina, Cusco"));
        assert!(from_event_page(&page("2026-11-07T09:00:00-05:00"), &url, "g", now()).is_none());
        assert!(from_event_page("<html>no data</html>", &url, "g", now()).is_none());
    }
}
