//! Editable fields, their textual form, validation and the merge rules that
//! keep manual edits safe from later collections.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::opportunity::normalize_tags;
use crate::{Category, ContentKind, Cost, Eligibility, Modality, Opportunity, SourceKind, Stance};

/// Kind of input a field expects (drives the editor widget).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    LongText,
    Url,
    Date,
    /// Comma-separated list.
    List,
    /// One of the given values; empty means unknown when allowed.
    Choice(&'static [&'static str]),
}

macro_rules! fields {
    ($($variant:ident => $id:literal, $label:literal, $kind:expr;)*) => {
        /// A user-editable field of an opportunity.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum Field { $($variant,)* }

        impl Field {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $id,)* }
            }

            pub const fn label(self) -> &'static str {
                match self { $(Self::$variant => $label,)* }
            }

            pub const fn kind(self) -> FieldKind {
                match self { $(Self::$variant => $kind,)* }
            }
        }
    };
}

const BOOL: &[&str] = &["", "yes", "no"];
const COST: &[&str] = &["unknown", "free", "free_with_paid_certificate", "paid"];
const ELIGIBILITY: &[&str] = &["unknown", "open", "not_eligible"];
const MODALITY: &[&str] = &["unknown", "online", "in_person", "hybrid"];
const CONTENT: &[&str] = &["opportunity", "news"];
const STANCE: &[&str] = &[
    "",
    "report",
    "opinion",
    "speculation",
    "discussion",
    "official",
];
const CATEGORY: &[&str] = &[
    "event",
    "conference",
    "workshop",
    "meetup",
    "course",
    "training",
    "hackathon",
    "competition",
    "scholarship",
    "exchange",
    "fellowship",
    "volunteering",
    "call",
    "job",
    "internship",
    "open_source",
    "other",
];
const SOURCE: &[&str] = &[
    "manual",
    "rss",
    "ical",
    "github",
    "hackernews",
    "reddit",
    "linkedin",
    "x",
    "luma",
    "gdg",
    "devpost",
    "meetup",
    "eventbrite",
    "kaggle",
    "mslearn",
    "instagram",
    "threads",
    "facebook",
    "tiktok",
    "youtube",
    "bluesky",
    "mastodon",
    "googlenews",
    "web",
];

fields! {
    Title => "title", "Title", FieldKind::Text;
    Summary => "summary", "Description", FieldKind::LongText;
    Content => "content", "Type", FieldKind::Choice(CONTENT);
    Category => "category", "Category", FieldKind::Choice(CATEGORY);
    Source => "source", "Platform", FieldKind::Choice(SOURCE);
    Url => "url", "URL", FieldKind::Url;
    AuthorName => "author_name", "Author", FieldKind::Text;
    AuthorHandle => "author_handle", "Username", FieldKind::Text;
    AuthorUrl => "author_url", "Profile URL", FieldKind::Url;
    AuthorAvatar => "author_avatar", "Avatar URL", FieldKind::Url;
    Image => "image", "Image URL", FieldKind::Url;
    Tags => "tags", "Tags", FieldKind::List;
    Location => "location", "Location", FieldKind::Text;
    Country => "country", "Country", FieldKind::Text;
    Modality => "modality", "Modality", FieldKind::Choice(MODALITY);
    StartsAt => "starts_at", "Starts", FieldKind::Date;
    EndsAt => "ends_at", "Ends", FieldKind::Date;
    Deadline => "deadline", "Deadline", FieldKind::Date;
    Cost => "cost", "Cost", FieldKind::Choice(COST);
    Price => "price", "Price", FieldKind::Text;
    Currency => "currency", "Currency", FieldKind::Text;
    Requirements => "requirements", "Requirements", FieldKind::LongText;
    Eligibility => "eligibility", "Eligibility", FieldKind::Choice(ELIGIBILITY);
    Prizes => "prizes", "Prizes", FieldKind::Text;
    Benefits => "benefits", "Benefits", FieldKind::Text;
    Certificate => "certificate", "Certificate", FieldKind::Choice(BOOL);
    Travel => "travel", "Travel covered", FieldKind::Choice(BOOL);
    Lodging => "lodging", "Lodging covered", FieldKind::Choice(BOOL);
    Stance => "stance", "News stance", FieldKind::Choice(STANCE);
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Field {
    type Err = EditError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|f| f.as_str() == s)
            .ok_or_else(|| EditError::UnknownField(s.to_owned()))
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EditError {
    #[error("unknown field {0:?}")]
    UnknownField(String),
    #[error("{field}: {message}")]
    Invalid {
        field: &'static str,
        message: String,
    },
}

fn invalid(field: Field, message: impl Into<String>) -> EditError {
    EditError::Invalid {
        field: field.label(),
        message: message.into(),
    }
}

fn opt(value: &str) -> Option<String> {
    let v = value.trim();
    (!v.is_empty()).then(|| v.to_owned())
}

fn bool_text(v: Option<bool>) -> String {
    match v {
        Some(true) => "yes".into(),
        Some(false) => "no".into(),
        None => String::new(),
    }
}

/// Formats a date for editing: `YYYY-MM-DD` at midnight, else with time.
pub fn format_date(d: DateTime<Utc>) -> String {
    if d.format("%H:%M").to_string() == "00:00" {
        d.format("%Y-%m-%d").to_string()
    } else {
        d.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Parses `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` or RFC 3339 (UTC assumed).
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|d| d.and_utc());
    }
    if let Ok(d) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Some(d.and_utc());
    }
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.to_utc())
}

/// Splits a paid price detail into `(amount, currency)`. A leading
/// three-letter uppercase code is treated as the currency.
fn split_price(detail: &str) -> (String, String) {
    match detail.split_once(' ') {
        Some((cur, amount)) if cur.len() == 3 && cur.chars().all(|c| c.is_ascii_uppercase()) => {
            (amount.trim().to_owned(), cur.to_owned())
        }
        _ => (detail.to_owned(), String::new()),
    }
}

fn join_price(amount: &str, currency: &str) -> Option<String> {
    opt(&format!("{} {}", currency.trim(), amount.trim()))
}

impl Field {
    /// Current value as editable text (empty = unknown).
    #[allow(clippy::too_many_lines)]
    pub fn get(self, o: &Opportunity) -> String {
        let s = |v: &Option<String>| v.clone().unwrap_or_default();
        let date = |d: Option<DateTime<Utc>>| d.map(format_date).unwrap_or_default();
        let paid_detail = match &o.cost {
            Cost::Paid(Some(d)) => d.clone(),
            _ => String::new(),
        };
        match self {
            Self::Title => o.title.clone(),
            Self::Summary => s(&o.summary),
            Self::Content => o.content.as_str().into(),
            Self::Category => o.category.as_str().into(),
            Self::Source => o.source.as_str().into(),
            Self::Url => o.original_url.clone(),
            Self::AuthorName => s(&o.author.name),
            Self::AuthorHandle => s(&o.author.handle),
            Self::AuthorUrl => s(&o.author.profile_url),
            Self::AuthorAvatar => s(&o.author.avatar_url),
            Self::Image => s(&o.image_url),
            Self::Tags => o.tags.join(", "),
            Self::Location => s(&o.location),
            Self::Country => s(&o.country),
            Self::Modality => o.modality.as_str().into(),
            Self::StartsAt => date(o.dates.starts_at),
            Self::EndsAt => date(o.dates.ends_at),
            Self::Deadline => date(o.dates.deadline),
            Self::Cost => match o.cost {
                Cost::Unknown => "unknown",
                Cost::Free => "free",
                Cost::FreeWithPaidCertificate => "free_with_paid_certificate",
                Cost::Paid(_) => "paid",
            }
            .into(),
            Self::Price => split_price(&paid_detail).0,
            Self::Currency => split_price(&paid_detail).1,
            Self::Requirements => s(&o.requirements),
            Self::Eligibility => o.eligibility.as_str().into(),
            Self::Prizes => s(&o.perks.prizes),
            Self::Benefits => s(&o.perks.benefits),
            Self::Certificate => bool_text(o.perks.certificate),
            Self::Travel => bool_text(o.perks.travel_covered),
            Self::Lodging => bool_text(o.perks.accommodation_covered),
            Self::Stance => o.stance.map(Stance::as_str).unwrap_or_default().into(),
        }
    }

    /// Validates `value` and stores it in `o`.
    #[allow(clippy::too_many_lines)]
    pub fn set(self, o: &mut Opportunity, value: &str) -> Result<(), EditError> {
        if let FieldKind::Choice(options) = self.kind()
            && !options.contains(&value.trim())
        {
            return Err(invalid(
                self,
                format!("must be one of: {}", options.join(", ")),
            ));
        }
        let url = |v: &str| -> Result<Option<String>, EditError> {
            match opt(v) {
                None => Ok(None),
                Some(u) if u.starts_with("https://") || u.starts_with("http://") => Ok(Some(u)),
                Some(_) => Err(invalid(self, "must start with http:// or https://")),
            }
        };
        let date = |v: &str| -> Result<Option<DateTime<Utc>>, EditError> {
            match opt(v) {
                None => Ok(None),
                Some(d) => parse_date(&d)
                    .map(Some)
                    .ok_or_else(|| invalid(self, "use YYYY-MM-DD or YYYY-MM-DD HH:MM")),
            }
        };
        let flag = |v: &str| match v.trim() {
            "yes" => Some(true),
            "no" => Some(false),
            _ => None,
        };
        let v = value.trim();
        match self {
            Self::Title => {
                o.title = opt(v).ok_or_else(|| invalid(self, "cannot be empty"))?;
            }
            Self::Summary => o.summary = opt(v),
            Self::Content => {
                o.content = if v == "news" {
                    ContentKind::News
                } else {
                    ContentKind::Opportunity
                };
            }
            Self::Category => {
                o.category = Category::from_str(v).map_err(|e| invalid(self, e.to_string()))?;
            }
            Self::Source => {
                o.source = SourceKind::from_str(v).map_err(|e| invalid(self, e.to_string()))?;
            }
            Self::Url => {
                o.original_url = url(v)?.ok_or_else(|| invalid(self, "cannot be empty"))?;
            }
            Self::AuthorName => o.author.name = opt(v),
            Self::AuthorHandle => o.author.handle = opt(v),
            Self::AuthorUrl => o.author.profile_url = url(v)?,
            Self::AuthorAvatar => o.author.avatar_url = url(v)?,
            Self::Image => o.image_url = url(v)?,
            Self::Tags => {
                o.tags = normalize_tags(v.split(',').map(str::to_owned).collect());
            }
            Self::Location => o.location = opt(v),
            Self::Country => o.country = opt(v),
            Self::Modality => {
                o.modality = Modality::from_str(v).map_err(|e| invalid(self, e.to_string()))?;
            }
            Self::StartsAt => o.dates.starts_at = date(v)?,
            Self::EndsAt => o.dates.ends_at = date(v)?,
            Self::Deadline => o.dates.deadline = date(v)?,
            Self::Cost => {
                let detail = match &o.cost {
                    Cost::Paid(d) => d.clone(),
                    _ => None,
                };
                o.cost = match v {
                    "free" => Cost::Free,
                    "free_with_paid_certificate" => Cost::FreeWithPaidCertificate,
                    "paid" => Cost::Paid(detail),
                    _ => Cost::Unknown,
                };
            }
            Self::Price | Self::Currency => {
                let current = Self::Price.get(o);
                let currency = Self::Currency.get(o);
                let (amount, currency) = if self == Self::Price {
                    (v.to_owned(), currency)
                } else {
                    (current, v.to_uppercase())
                };
                if self == Self::Currency
                    && !currency.is_empty()
                    && !(currency.len() == 3 && currency.chars().all(|c| c.is_ascii_uppercase()))
                {
                    return Err(invalid(self, "use a 3-letter code such as PEN or USD"));
                }
                o.cost = Cost::Paid(join_price(&amount, &currency));
            }
            Self::Requirements => o.requirements = opt(v),
            Self::Eligibility => o.eligibility = Eligibility::from_db(v),
            Self::Prizes => o.perks.prizes = opt(v),
            Self::Benefits => o.perks.benefits = opt(v),
            Self::Certificate => o.perks.certificate = flag(v),
            Self::Travel => o.perks.travel_covered = flag(v),
            Self::Lodging => o.perks.accommodation_covered = flag(v),
            Self::Stance => o.stance = Stance::from_db(v),
        }
        Ok(())
    }
}

/// Personal data attached to an opportunity (never detected from sources).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Annotations {
    pub notes: Option<String>,
    pub priority: Option<crate::Priority>,
    pub custom: Vec<(String, String)>,
}

/// Applies stored overrides on top of detected data. Invalid stored values
/// are skipped (and reported) rather than failing the whole item.
pub fn apply_overrides<'a>(
    detected: &Opportunity,
    overrides: impl IntoIterator<Item = (Field, &'a str)>,
) -> (Opportunity, Vec<EditError>) {
    let mut effective = detected.clone();
    let mut errors = Vec::new();
    for (field, value) in overrides {
        if let Err(e) = field.set(&mut effective, value) {
            errors.push(e);
        }
    }
    (effective.sanitized(), errors)
}

/// Merges a new sighting into the detected snapshot: unknown fields are
/// completed, known ones are kept, tags are united. Identity fields (URL,
/// fingerprint, source, discovery date) never change.
pub fn merge_detected(base: &mut Opportunity, new: &Opportunity) {
    fn fill<T: Clone>(slot: &mut Option<T>, value: Option<&T>) {
        if slot.is_none() {
            *slot = value.cloned();
        }
    }
    fill(&mut base.summary, new.summary.as_ref());
    fill(&mut base.image_url, new.image_url.as_ref());
    fill(&mut base.location, new.location.as_ref());
    fill(&mut base.country, new.country.as_ref());
    fill(&mut base.requirements, new.requirements.as_ref());
    fill(
        &mut base.dates.published_at,
        new.dates.published_at.as_ref(),
    );
    fill(&mut base.dates.starts_at, new.dates.starts_at.as_ref());
    fill(&mut base.dates.ends_at, new.dates.ends_at.as_ref());
    fill(&mut base.dates.deadline, new.dates.deadline.as_ref());
    fill(&mut base.perks.certificate, new.perks.certificate.as_ref());
    fill(&mut base.perks.prizes, new.perks.prizes.as_ref());
    fill(&mut base.perks.benefits, new.perks.benefits.as_ref());
    fill(
        &mut base.perks.travel_covered,
        new.perks.travel_covered.as_ref(),
    );
    fill(
        &mut base.perks.accommodation_covered,
        new.perks.accommodation_covered.as_ref(),
    );
    fill(&mut base.author.name, new.author.name.as_ref());
    fill(&mut base.author.handle, new.author.handle.as_ref());
    fill(
        &mut base.author.profile_url,
        new.author.profile_url.as_ref(),
    );
    fill(&mut base.author.avatar_url, new.author.avatar_url.as_ref());
    fill(&mut base.stance, new.stance.as_ref());
    if base.category == Category::Other {
        base.category = new.category;
    }
    if base.modality == Modality::Unknown {
        base.modality = new.modality;
    }
    if base.cost == Cost::Unknown {
        base.cost = new.cost.clone();
    }
    if base.eligibility == Eligibility::Unknown {
        base.eligibility = new.eligibility;
    }
    let mut tags = base.tags.clone();
    tags.extend(new.tags.iter().cloned());
    base.tags = normalize_tags(tags);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalUrl;

    fn sample() -> Opportunity {
        let url = CanonicalUrl::parse("https://x.example/a").unwrap();
        Opportunity::new(
            SourceKind::Manual,
            "manual",
            url.as_str(),
            &url,
            "Old",
            Utc::now(),
        )
    }

    #[test]
    fn every_field_round_trips_through_text() {
        let mut o = sample();
        let values: &[(Field, &str)] = &[
            (Field::Title, "Beca"),
            (Field::Summary, "Full scholarship"),
            (Field::Content, "news"),
            (Field::Category, "scholarship"),
            (Field::Source, "instagram"),
            (Field::Url, "https://instagram.com/p/1"),
            (Field::AuthorName, "Ana"),
            (Field::AuthorHandle, "@ana"),
            (Field::AuthorUrl, "https://instagram.com/ana"),
            (Field::AuthorAvatar, "https://cdn.example/ana.jpg"),
            (Field::Image, "https://cdn.example/p.jpg"),
            (Field::Tags, "japan, travel"),
            (Field::Location, "Tokyo"),
            (Field::Country, "Japan"),
            (Field::Modality, "in_person"),
            (Field::StartsAt, "2027-04-01"),
            (Field::EndsAt, "2027-04-02 18:30"),
            (Field::Deadline, "2027-01-15"),
            (Field::Cost, "paid"),
            (Field::Price, "49"),
            (Field::Currency, "USD"),
            (Field::Requirements, "Bachelor"),
            (Field::Eligibility, "open"),
            (Field::Prizes, "USD 1000"),
            (Field::Benefits, "Mentoring"),
            (Field::Certificate, "yes"),
            (Field::Travel, "yes"),
            (Field::Lodging, "no"),
            (Field::Stance, "official"),
        ];
        assert_eq!(values.len(), Field::ALL.len());
        for (field, value) in values {
            field.set(&mut o, value).unwrap();
        }
        for (field, value) in values {
            assert_eq!(field.get(&o), *value, "{field}");
        }
        assert_eq!(o.cost, Cost::Paid(Some("USD 49".into())));
    }

    #[test]
    fn validates_input() {
        let mut o = sample();
        assert!(Field::Title.set(&mut o, "  ").is_err());
        assert!(Field::Url.set(&mut o, "ftp://x").is_err());
        assert!(Field::Deadline.set(&mut o, "15/01/2027").is_err());
        assert!(Field::Category.set(&mut o, "party").is_err());
        assert!(Field::Currency.set(&mut o, "dollars").is_err());
        assert!(Field::Deadline.set(&mut o, "").is_ok());
        assert_eq!(o.dates.deadline, None);
        assert_eq!("tags".parse::<Field>(), Ok(Field::Tags));
    }

    #[test]
    fn overrides_win_and_bad_values_are_reported() {
        let detected = sample();
        let (effective, errors) = apply_overrides(
            &detected,
            [(Field::Title, "Mine"), (Field::Deadline, "garbage")],
        );
        assert_eq!(effective.title, "Mine");
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn merge_completes_unknowns_only() {
        let mut base = sample();
        base.location = Some("Cusco".into());
        base.tags = vec!["a".into()];
        let mut new = sample();
        new.location = Some("Lima".into());
        new.summary = Some("New".into());
        new.category = Category::Hackathon;
        new.tags = vec!["b".into(), "a".into()];
        merge_detected(&mut base, &new);
        assert_eq!(base.location.as_deref(), Some("Cusco"));
        assert_eq!(base.summary.as_deref(), Some("New"));
        assert_eq!(base.category, Category::Hackathon);
        assert_eq!(base.tags, ["a", "b"]);
    }
}
