#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sentri_core::{CanonicalUrl, Category, Opportunity, SourceKind};

pub fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-13T12:00:00Z")
        .unwrap()
        .to_utc()
}

pub fn opp(url: &str, title: &str, category: Category) -> Opportunity {
    let canonical = CanonicalUrl::parse(url).unwrap();
    let mut o = Opportunity::new(SourceKind::Rss, "feed", url, &canonical, title, now());
    o.category = category;
    o
}
