//! "Add a link" use case: turn any URL into an opportunity.

use chrono::{DateTime, Utc};
use sentri_core::{
    CanonicalUrl, Category, Cost, FeedItem, Field, LinkError, Modality, Opportunity, SourceKind,
    classify,
};
use sentri_sources::metadata::{self, PageMetadata, StructuredKind};
use sentri_sources::{FetchError, SafeClient};
use url::Url;

/// Name used for every manually added item.
pub const MANUAL_SOURCE: &str = "manual";

/// Fields the user can provide or correct when adding a link.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkOverrides {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub category: Option<Category>,
    pub modality: Option<Modality>,
    pub location: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

/// What happened when adding a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    /// Stored as a new opportunity. `warnings` explains missing data.
    Added {
        item: FeedItem,
        warnings: Vec<String>,
    },
    /// The same canonical URL was already stored; nothing was fetched.
    Duplicate(FeedItem),
}

/// Errors that prevent adding a link at all.
#[derive(Debug, thiserror::Error)]
pub enum AddLinkError {
    #[error(transparent)]
    InvalidUrl(#[from] LinkError),
    #[error(transparent)]
    Storage(#[from] sentri_storage::StorageError),
}

/// Downloads the page (if possible) and builds an opportunity from it.
///
/// A failed download is not fatal: the link is still stored with the data
/// the user provided, plus a warning, so it can be completed later.
pub(crate) async fn build(
    client: &SafeClient,
    url: &CanonicalUrl,
    original: &str,
    overrides: &LinkOverrides,
    now: DateTime<Utc>,
) -> (Opportunity, Vec<String>) {
    let source =
        SourceKind::from_url_parts(url.host(), url.url().path()).unwrap_or(SourceKind::Manual);
    let mut o = Opportunity::new(source, MANUAL_SOURCE, original, url, url.as_str(), now);
    let mut warnings = Vec::new();

    match fetch_metadata(client, original).await {
        Ok(Some(meta)) => apply_metadata(&mut o, meta),
        Ok(None) => warnings.push("The link is not an HTML page; metadata not extracted.".into()),
        Err(error) => warnings.push(format!("Could not download the page: {error}")),
    }
    classify::enrich(&mut o);
    // Classify from the text the user typed when the page said nothing.
    if o.category == Category::Other && overrides.category.is_none() {
        let typed = [overrides.title.as_deref(), overrides.summary.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(category) = classify::category(&typed) {
            o.category = category;
        }
    }
    let o = o.sanitized();
    if o.title == o.canonical_url && overrides.title.is_none() {
        warnings.push("No title found; edit the item to add one.".into());
    }
    if o.category == Category::Other && overrides.category.is_none() {
        warnings.push("Category could not be determined; edit the item to set it.".into());
    }
    (o, warnings)
}

/// Fetches the URL exactly as given (the canonical form may switch scheme),
/// still subject to every [`SafeClient`] check.
async fn fetch_metadata(
    client: &SafeClient,
    original: &str,
) -> Result<Option<PageMetadata>, FetchError> {
    let url = Url::parse(original).map_err(|e| FetchError::Forbidden(e.to_string()))?;
    let page = client.get(&url).await?;
    Ok(page
        .is_html()
        .then(|| metadata::extract(&page.text(), &page.url)))
}

fn apply_metadata(o: &mut Opportunity, meta: PageMetadata) {
    let s = meta.structured;
    o.author = meta.author;
    if let Some(title) = meta.title {
        o.title = title;
    }
    o.summary = meta.description;
    o.image_url = meta.image.map(String::from);
    o.dates.published_at = meta.published_at;
    o.dates.starts_at = s.starts_at;
    o.dates.ends_at = s.ends_at;
    o.dates.deadline = s.deadline;
    o.location = s.location;
    o.country = s.country;
    o.modality = s.modality.unwrap_or_default();
    o.cost = match (s.free, s.price) {
        (Some(true), _) => Cost::Free,
        (Some(false), price) => Cost::Paid(price),
        (None, _) => Cost::Unknown,
    };
    o.category = match s.kind {
        Some(StructuredKind::Event) if s.hackathon => Category::Hackathon,
        Some(StructuredKind::Event) => classify::category(&o.title).unwrap_or(Category::Event),
        Some(StructuredKind::JobPosting) => classify::category(&o.title)
            .filter(|c| c.is_job())
            .unwrap_or(Category::Job),
        Some(StructuredKind::Course) => Category::Course,
        None => Category::Other,
    };
    let mut tags = meta.keywords;
    if let Some(org) = s.organization {
        tags.push(org);
    }
    if let Some(site) = meta.site_name {
        tags.push(site);
    }
    o.tags = tags;
}

impl LinkOverrides {
    /// Field values to store as manual overrides on top of `current`.
    pub fn values(&self, current: &Opportunity) -> Vec<(Field, String)> {
        let date = |d: Option<DateTime<Utc>>| d.map(sentri_core::edit::format_date);
        let mut values: Vec<(Field, String)> = [
            (Field::Title, self.title.clone()),
            (Field::Summary, self.summary.clone()),
            (
                Field::Category,
                self.category.map(|c| c.as_str().to_owned()),
            ),
            (
                Field::Modality,
                self.modality.map(|m| m.as_str().to_owned()),
            ),
            (Field::Location, self.location.clone()),
            (Field::StartsAt, date(self.starts_at)),
            (Field::Deadline, date(self.deadline)),
        ]
        .into_iter()
        .filter_map(|(field, value)| value.map(|v| (field, v)))
        .collect();
        if !self.tags.is_empty() {
            let mut tags = current.tags.clone();
            tags.extend(self.tags.iter().cloned());
            values.push((Field::Tags, tags.join(", ")));
        }
        values
    }
}
