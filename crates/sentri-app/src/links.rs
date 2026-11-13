//! "Add a link" use case: turn any URL into an opportunity.

use chrono::{DateTime, Utc};
use sentri_core::{
    CanonicalUrl, Category, Cost, FeedItem, LinkError, Modality, Opportunity, SourceKind, classify,
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
    apply_overrides(&mut o, overrides);
    classify::enrich(&mut o);
    let o = o.sanitized();
    if o.title == o.canonical_url {
        warnings.push("No title found; edit the item to add one.".into());
    }
    if o.category == Category::Other {
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

pub(crate) fn apply_overrides(o: &mut Opportunity, v: &LinkOverrides) {
    if let Some(title) = &v.title {
        o.title.clone_from(title);
    }
    if v.summary.is_some() {
        o.summary.clone_from(&v.summary);
    }
    if let Some(category) = v.category {
        o.category = category;
    }
    if let Some(modality) = v.modality {
        o.modality = modality;
    }
    if v.location.is_some() {
        o.location.clone_from(&v.location);
    }
    if v.starts_at.is_some() {
        o.dates.starts_at = v.starts_at;
    }
    if v.deadline.is_some() {
        o.dates.deadline = v.deadline;
    }
    o.tags.extend(v.tags.iter().cloned());
}
