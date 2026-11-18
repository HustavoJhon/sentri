//! dev.events connector.
//!
//! dev.events is a public directory of developer conferences and meetups. Its
//! `robots.txt` allows every path for regular agents, and each listing page
//! (for example `/meetups/SA/PE`, the meetups listed for Peru) embeds one
//! schema.org `Event` JSON-LD block per event. One request per run reads the
//! whole listing; no event page is fetched.

use chrono::{DateTime, Duration, Utc};
use sentri_core::{CanonicalUrl, Category, Cost, Modality, Opportunity, SourceKind};
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::{self, Structured, StructuredKind};

/// Meetups and conferences dev.events lists for Peru.
pub const DEFAULT_LISTING: &str = "https://dev.events/meetups/SA/PE";

/// JSON-LD blocks read from one listing page.
const MAX_BLOCKS: usize = 120;

#[derive(Debug, Clone)]
pub struct DevEvents {
    pub(crate) spec: SourceSpec,
    listing: Url,
}

impl DevEvents {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let listing = Url::parse(
            spec.url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .unwrap_or(DEFAULT_LISTING),
        )
        .map_err(|e| SourceError::Config(format!("{}: {e}", spec.name)))?;
        let host = listing.host_str().unwrap_or_default();
        if host != "dev.events" && host != "www.dev.events" {
            return Err(SourceError::Config(format!(
                "{}: url must be a dev.events listing such as {DEFAULT_LISTING}",
                spec.name
            )));
        }
        Ok(Self { spec, listing })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.listing).await?;
        Ok(parse_listing(&page.text(), &self.spec.name, now))
    }

    pub async fn fetch_conditional(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
        validators: &crate::http::Validators,
    ) -> Result<(Vec<Opportunity>, crate::http::Validators), SourceError> {
        let page = http.get_conditional(&self.listing, validators).await?;
        let items = parse_listing(&page.text(), &self.spec.name, now);
        Ok((items, page.validators))
    }
}

/// Upcoming events declared on a listing page.
pub fn parse_listing(html: &str, source_name: &str, now: DateTime<Utc>) -> Vec<Opportunity> {
    let mut seen = std::collections::HashSet::new();
    metadata::structured_items(html, MAX_BLOCKS)
        .into_iter()
        .filter_map(|s| from_structured(s, source_name, now))
        .filter(|o| seen.insert(o.canonical_url.clone()))
        .collect()
}

fn from_structured(s: Structured, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    if s.kind != Some(StructuredKind::Event) {
        return None;
    }
    let starts = s.starts_at?;
    if s.ends_at.unwrap_or(starts) < now - Duration::hours(12) {
        return None;
    }
    let url = s.url.as_deref()?;
    let canonical = CanonicalUrl::parse(url).ok()?;
    let title = s.name.clone()?;
    let mut o = Opportunity::new(
        SourceKind::DevEvents,
        source_name,
        url,
        &canonical,
        &title,
        now,
    );
    o.external_id = Url::parse(url).ok().and_then(|u| {
        u.path_segments()?
            .next_back()
            .map(|id| format!("devevents:{id}"))
    });
    let text = format!("{title} {}", s.description.as_deref().unwrap_or_default());
    o.category = if s.hackathon {
        Category::Hackathon
    } else {
        sentri_core::classify::category(&text).unwrap_or(Category::Event)
    };
    o.summary = s.description;
    o.image_url = s.image;
    o.dates.starts_at = Some(starts);
    o.dates.ends_at = s.ends_at;
    o.modality = s.modality.unwrap_or(Modality::Unknown);
    o.location = s.location;
    // dev.events puts the country in `addressRegion` ("Peru").
    o.country = s.country.or(s.region);
    o.cost = match s.free {
        Some(true) => Cost::Free,
        Some(false) => Cost::Paid(s.price),
        None => Cost::Unknown,
    };
    o.author.name = s.organization;
    o.tags = vec!["devevents".into()];
    if o.country.as_deref().is_some_and(|c| {
        let c = c.to_lowercase();
        c.contains("peru") || c.contains("perú") || c == "pe"
    }) {
        o.tags.push("peru".into());
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-18T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    const LISTING: &str = r#"<html><head>
<script type="application/ld+json">{"@context":"http://schema.org","@type":"BreadcrumbList","itemListElement":[]}</script>
<script type="application/ld+json">{"@context":"http://schema.org","@type":"EducationEvent","name":"PyDay Lima 2026",
 "eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode","description":"Python conference Lima",
 "startDate":"2026-12-11T00:00:00.000+00:00","endDate":"2026-12-12T00:00:00.000+00:00",
 "url":"https://dev.events/conferences/pyday-lima-2026-abc","image":["https://res.cloudinary.com/x/pyday.png"],
 "location":{"@type":"Place","address":{"@type":"PostalAddress","addressLocality":"Lima","addressRegion":"Peru"},"name":"Lima, Peru"},
 "performer":{"@type":"Organization","name":"Python Peru"}}</script>
<script type="application/ld+json">{"@context":"http://schema.org","@type":"EducationEvent","name":"Angular Community Meetup - En Español",
 "eventAttendanceMode":"https://schema.org/OnlineEventAttendanceMode","description":"Angular meetup Online",
 "startDate":"2026-12-09T00:00:00.000+00:00","url":"https://dev.events/meetups/angular-community-meetup-xyz"}</script>
<script type="application/ld+json">{"@context":"http://schema.org","@type":"EducationEvent","name":"Old meetup",
 "startDate":"2026-10-01T00:00:00.000+00:00","url":"https://dev.events/meetups/old"}</script>
<script type="application/ld+json">{"@type":"EducationEvent","name":"No link","startDate":"2026-12-01T00:00:00Z"}</script>
<script type="application/ld+json">{ not json </script>
</head></html>"#;

    #[test]
    fn reads_every_upcoming_event_on_the_listing() {
        let items = parse_listing(LISTING, "dev-events-peru", now());
        let titles: Vec<&str> = items.iter().map(|o| o.title.as_str()).collect();
        assert_eq!(
            titles,
            ["PyDay Lima 2026", "Angular Community Meetup - En Español"]
        );
        let pyday = &items[0];
        assert_eq!(pyday.source, SourceKind::DevEvents);
        assert_eq!(pyday.country.as_deref(), Some("Peru"));
        assert_eq!(pyday.location.as_deref(), Some("Lima, Peru"));
        assert_eq!(pyday.modality, Modality::InPerson);
        assert_eq!(
            pyday.image_url.as_deref(),
            Some("https://res.cloudinary.com/x/pyday.png")
        );
        assert!(pyday.tags.contains(&"peru".to_owned()));
        assert_eq!(
            pyday.external_id.as_deref(),
            Some("devevents:pyday-lima-2026-abc")
        );
        let online = &items[1];
        assert_eq!(online.modality, Modality::Online);
        assert!(online.country.is_none());
    }

    #[test]
    fn deduplicates_repeated_blocks() {
        let html = format!("{LISTING}{LISTING}");
        assert_eq!(parse_listing(&html, "d", now()).len(), 2);
    }

    #[test]
    fn rejects_listings_on_other_hosts() {
        let spec = |url: &str| SourceSpec {
            name: "d".into(),
            kind: SourceKind::DevEvents,
            url: Some(url.into()),
            query: None,
            target: None,
            category: None,
            tags: Vec::new(),
            max_items: 10,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        };
        assert!(DevEvents::new(spec("https://dev.events/conferences/SA/PE")).is_ok());
        assert!(DevEvents::new(spec("https://evil.example/meetups/SA/PE")).is_err());
    }
}
