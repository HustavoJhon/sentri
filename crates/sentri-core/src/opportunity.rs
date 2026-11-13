//! The central domain entity: an opportunity discovered from any source.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::text::{clean, clean_opt};
use crate::{CanonicalUrl, Category, Modality, SourceKind};

/// Maximum lengths applied to untrusted text fields.
pub mod limits {
    pub const TITLE: usize = 300;
    pub const SUMMARY: usize = 2_000;
    pub const SHORT: usize = 200;
    pub const LONG: usize = 1_000;
    pub const TAG: usize = 40;
    pub const MAX_TAGS: usize = 20;
}

/// Database identifier of a stored opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OpportunityId(pub i64);

impl std::fmt::Display for OpportunityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What it costs to take part.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum Cost {
    #[default]
    Unknown,
    Free,
    /// Free to attend or study, but the certificate is paid.
    FreeWithPaidCertificate,
    /// Paid; the optional detail keeps the price as published (e.g. "USD 49").
    Paid(Option<String>),
}

impl Cost {
    pub const fn is_free(&self) -> bool {
        matches!(self, Self::Free | Self::FreeWithPaidCertificate)
    }

    pub fn label(&self) -> String {
        match self {
            Self::Unknown => "Unknown".into(),
            Self::Free => "Free".into(),
            Self::FreeWithPaidCertificate => "Free · paid certificate".into(),
            Self::Paid(Some(price)) => format!("Paid · {price}"),
            Self::Paid(None) => "Paid".into(),
        }
    }
}

/// Whether participants from the user's country can apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eligibility {
    #[default]
    Unknown,
    Open,
    NotEligible,
}

impl Eligibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Open => "open",
            Self::NotEligible => "not_eligible",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "open" => Self::Open,
            "not_eligible" => Self::NotEligible,
            _ => Self::Unknown,
        }
    }
}

/// Relevant dates. They are kept separate on purpose: an event date, an
/// application deadline and a course start are different things.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Dates {
    pub published_at: Option<DateTime<Utc>>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
}

/// Benefits and perks; `None` means "not stated", never "no".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Perks {
    pub certificate: Option<bool>,
    pub prizes: Option<String>,
    pub benefits: Option<String>,
    pub travel_covered: Option<bool>,
    pub accommodation_covered: Option<bool>,
}

impl Perks {
    pub fn offers_travel(&self) -> bool {
        self.travel_covered == Some(true) || self.accommodation_covered == Some(true)
    }
}

/// Who published an item. Only what the source states; never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Author {
    pub name: Option<String>,
    /// Handle on the platform (e.g. `@user`, `u/user`).
    pub handle: Option<String>,
    pub profile_url: Option<String>,
    pub avatar_url: Option<String>,
}

impl Author {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.handle.is_none()
            && self.profile_url.is_none()
            && self.avatar_url.is_none()
    }

    /// Best short label: name, then handle.
    pub fn display(&self) -> Option<&str> {
        self.name.as_deref().or(self.handle.as_deref())
    }

    #[must_use]
    fn sanitized(self) -> Self {
        Self {
            name: clean_opt(self.name.as_deref(), limits::SHORT),
            handle: clean_opt(self.handle.as_deref(), limits::SHORT),
            profile_url: self.profile_url.filter(|u| is_web_url(u)),
            avatar_url: self
                .avatar_url
                .filter(|u| u.starts_with("https://") && u.len() < 2_048),
        }
    }
}

fn is_web_url(u: &str) -> bool {
    (u.starts_with("https://") || u.starts_with("http://")) && u.len() < 2_048
}

/// Whether an item is an opportunity to act on or a news item to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    #[default]
    Opportunity,
    News,
}

impl ContentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Opportunity => "opportunity",
            Self::News => "news",
        }
    }

    pub fn from_db(s: &str) -> Self {
        if s == "news" {
            Self::News
        } else {
            Self::Opportunity
        }
    }
}

/// Nature of a news item, as signalled by its own wording. SENTRI cannot
/// verify facts, so there is deliberately no "verified" value: the default
/// is a report attributed to its outlet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// Reported by the outlet; attribute it, do not treat it as confirmed.
    Report,
    Opinion,
    /// Rumors, leaks, "reportedly", anonymous sources.
    Speculation,
    /// Community discussion (e.g. Hacker News threads).
    Discussion,
    /// Official announcement from the company or project itself.
    Official,
}

impl Stance {
    pub const ALL: [Self; 5] = [
        Self::Report,
        Self::Opinion,
        Self::Speculation,
        Self::Discussion,
        Self::Official,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::Opinion => "opinion",
            Self::Speculation => "speculation",
            Self::Discussion => "discussion",
            Self::Official => "official",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Report => "Reported",
            Self::Opinion => "Opinion",
            Self::Speculation => "Unconfirmed",
            Self::Discussion => "Discussion",
            Self::Official => "Official",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// An opportunity as produced by a connector or entered by the user.
///
/// Every optional field means "unknown": SENTRI never invents data that the
/// source did not publish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opportunity {
    pub source: SourceKind,
    /// Name of the configured source instance (e.g. `"freecodecamp-news"`).
    pub source_name: String,
    /// Identifier of the item inside its source, if it has one.
    pub external_id: Option<String>,
    /// URL exactly as received.
    pub original_url: String,
    /// Canonical URL used for deduplication.
    pub canonical_url: String,
    pub fingerprint: String,
    pub title: String,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub category: Category,
    pub tags: Vec<String>,
    pub location: Option<String>,
    pub country: Option<String>,
    pub modality: Modality,
    pub dates: Dates,
    pub cost: Cost,
    pub requirements: Option<String>,
    pub eligibility: Eligibility,
    pub perks: Perks,
    pub author: Author,
    pub content: ContentKind,
    /// Only meaningful for news.
    pub stance: Option<Stance>,
    pub discovered_at: DateTime<Utc>,
}

impl Opportunity {
    /// Creates an opportunity with the mandatory fields; the rest start unknown.
    pub fn new(
        source: SourceKind,
        source_name: &str,
        original_url: &str,
        url: &CanonicalUrl,
        title: &str,
        discovered_at: DateTime<Utc>,
    ) -> Self {
        Self {
            source,
            source_name: clean(source_name, limits::SHORT),
            external_id: None,
            original_url: original_url.trim().to_owned(),
            canonical_url: url.as_str().to_owned(),
            fingerprint: url.fingerprint().to_owned(),
            title: clean(title, limits::TITLE),
            summary: None,
            image_url: None,
            category: Category::Other,
            tags: Vec::new(),
            location: None,
            country: None,
            modality: Modality::Unknown,
            dates: Dates::default(),
            cost: Cost::Unknown,
            requirements: None,
            eligibility: Eligibility::Unknown,
            perks: Perks::default(),
            author: Author::default(),
            content: ContentKind::Opportunity,
            stance: None,
            discovered_at,
        }
    }

    /// Re-applies length limits and control character stripping to every
    /// free-text field. Connectors call this before handing data over.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.title = clean(&self.title, limits::TITLE);
        if self.title.is_empty() {
            self.title = self.canonical_url.clone();
        }
        self.source_name = clean(&self.source_name, limits::SHORT);
        self.external_id = clean_opt(self.external_id.as_deref(), limits::SHORT);
        self.summary = clean_opt(self.summary.as_deref(), limits::SUMMARY);
        self.image_url = self
            .image_url
            .filter(|u| u.starts_with("https://") && u.len() < 2_048);
        self.location = clean_opt(self.location.as_deref(), limits::SHORT);
        self.country = clean_opt(self.country.as_deref(), limits::SHORT);
        self.requirements = clean_opt(self.requirements.as_deref(), limits::LONG);
        self.perks.prizes = clean_opt(self.perks.prizes.as_deref(), limits::LONG);
        self.perks.benefits = clean_opt(self.perks.benefits.as_deref(), limits::LONG);
        if let Cost::Paid(price) = &mut self.cost {
            *price = clean_opt(price.as_deref(), limits::SHORT);
        }
        self.tags = normalize_tags(self.tags);
        self.author = self.author.sanitized();
        self
    }

    /// All searchable text, lowercased, for keyword matching.
    pub fn haystack(&self) -> String {
        let mut text = self.title.to_lowercase();
        for part in [&self.summary, &self.requirements, &self.location]
            .into_iter()
            .flatten()
        {
            text.push(' ');
            text.push_str(&part.to_lowercase());
        }
        for tag in &self.tags {
            text.push(' ');
            text.push_str(tag);
        }
        text
    }

    /// The next relevant date in the future, preferring the deadline.
    pub fn next_date(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        [self.dates.deadline, self.dates.starts_at]
            .into_iter()
            .flatten()
            .filter(|d| *d >= now)
            .min()
    }
}

/// A stored opportunity together with the user's interaction state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: OpportunityId,
    pub opportunity: Opportunity,
    pub state: crate::InteractionState,
}

/// Lowercases, trims, deduplicates and caps tags.
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tags.len().min(limits::MAX_TAGS));
    for tag in tags {
        let tag = clean(&tag.to_lowercase(), limits::TAG);
        if !tag.is_empty() && !out.contains(&tag) {
            out.push(tag);
        }
        if out.len() == limits::MAX_TAGS {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Opportunity {
        let url = CanonicalUrl::parse("https://example.com/hack").unwrap();
        Opportunity::new(
            SourceKind::Manual,
            "manual",
            "https://example.com/hack",
            &url,
            "AI Hackathon",
            Utc::now(),
        )
    }

    #[test]
    fn new_starts_with_unknowns() {
        let o = sample();
        assert_eq!(o.cost, Cost::Unknown);
        assert_eq!(o.eligibility, Eligibility::Unknown);
        assert_eq!(o.perks.certificate, None);
        assert_eq!(o.fingerprint.len(), 64);
    }

    #[test]
    fn sanitized_cleans_untrusted_fields() {
        let mut o = sample();
        o.title = "\u{1b}[2J".into();
        o.summary = Some("  ".into());
        o.image_url = Some("javascript:alert(1)".into());
        o.tags = vec!["Rust".into(), "rust ".into(), String::new()];
        let o = o.sanitized();
        assert_eq!(o.title, "[2J");
        assert_eq!(o.summary, None);
        assert_eq!(o.image_url, None);
        assert_eq!(o.tags, vec!["rust".to_owned()]);
    }

    #[test]
    fn empty_title_falls_back_to_url() {
        let mut o = sample();
        o.title = "\u{7}".into();
        assert_eq!(o.sanitized().title, "https://example.com/hack");
    }

    #[test]
    fn author_fields_are_sanitized() {
        let mut o = sample();
        o.author = Author {
            name: Some("Ana \u{1b}[1m".into()),
            handle: Some(" ".into()),
            profile_url: Some("javascript:x".into()),
            avatar_url: Some("http://insecure/a.png".into()),
        };
        let a = o.sanitized().author;
        assert_eq!(a.name.as_deref(), Some("Ana [1m"));
        assert_eq!(a.handle, None);
        assert_eq!(a.profile_url, None);
        assert_eq!(a.avatar_url, None);
        assert_eq!(a.display(), Some("Ana [1m"));
    }

    #[test]
    fn cost_labels() {
        assert!(Cost::FreeWithPaidCertificate.is_free());
        assert_eq!(Cost::Paid(Some("USD 10".into())).label(), "Paid · USD 10");
    }
}
