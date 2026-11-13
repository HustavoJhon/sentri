//! Mapping between SQLite rows and domain types.

use std::str::FromStr;

use sentri_core::{
    Category, Cost, Dates, Eligibility, FeedItem, InteractionState, Modality, Opportunity,
    OpportunityId, Perks,
};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::StorageError;
use crate::time::{from_db, opt_from_db};

/// Separator used by `GROUP_CONCAT` for tags. Tags are sanitized and can
/// never contain control characters, so this byte is unambiguous.
pub(crate) const TAG_SEPARATOR: &str = "\u{1f}";

/// Column list shared by every query that builds a [`FeedItem`].
pub(crate) const FEED_COLUMNS: &str = "
    o.id, o.fingerprint, o.canonical_url, o.original_url, o.source_kind, o.source_name,
    o.external_id, o.title, o.summary, o.image_url, o.category, o.location, o.country,
    o.modality, o.published_at, o.starts_at, o.ends_at, o.deadline, o.cost_kind,
    o.cost_detail, o.requirements, o.eligibility, o.certificate, o.prizes, o.benefits,
    o.travel_covered, o.accommodation_covered, o.discovered_at,
    (SELECT GROUP_CONCAT(t.tag, char(31)) FROM opportunity_tags t WHERE t.opportunity_id = o.id)
        AS tags,
    COALESCE(i.liked, 0) AS liked, COALESCE(i.saved, 0) AS saved,
    COALESCE(i.dismissed, 0) AS dismissed, COALESCE(i.interesting, 0) AS interesting,
    COALESCE(i.applied, 0) AS applied, COALESCE(i.enrolled, 0) AS enrolled,
    COALESCE(i.completed, 0) AS completed, i.last_viewed_at";

/// `FROM` clause matching [`FEED_COLUMNS`].
pub(crate) const FEED_FROM: &str =
    " FROM opportunities o LEFT JOIN interactions i ON i.opportunity_id = o.id";

fn parse<T: FromStr>(column: &'static str, value: &str) -> Result<T, StorageError> {
    value.parse().map_err(|_| StorageError::Corrupt {
        column,
        value: value.to_owned(),
    })
}

pub(crate) fn cost_to_db(cost: &Cost) -> (&'static str, Option<&str>) {
    match cost {
        Cost::Unknown => ("unknown", None),
        Cost::Free => ("free", None),
        Cost::FreeWithPaidCertificate => ("free_with_paid_certificate", None),
        Cost::Paid(detail) => ("paid", detail.as_deref()),
    }
}

fn cost_from_db(kind: &str, detail: Option<String>) -> Result<Cost, StorageError> {
    Ok(match kind {
        "unknown" => Cost::Unknown,
        "free" => Cost::Free,
        "free_with_paid_certificate" => Cost::FreeWithPaidCertificate,
        "paid" => Cost::Paid(detail),
        other => {
            return Err(StorageError::Corrupt {
                column: "cost_kind",
                value: other.to_owned(),
            });
        }
    })
}

pub(crate) fn feed_item(row: &SqliteRow) -> Result<FeedItem, StorageError> {
    let tags: Option<String> = row.try_get("tags")?;
    let opportunity = Opportunity {
        source: parse("source_kind", row.try_get("source_kind")?)?,
        source_name: row.try_get("source_name")?,
        external_id: row.try_get("external_id")?,
        original_url: row.try_get("original_url")?,
        canonical_url: row.try_get("canonical_url")?,
        fingerprint: row.try_get("fingerprint")?,
        title: row.try_get("title")?,
        summary: row.try_get("summary")?,
        image_url: row.try_get("image_url")?,
        category: parse::<Category>("category", row.try_get("category")?)?,
        tags: tags
            .map(|t| t.split(TAG_SEPARATOR).map(str::to_owned).collect())
            .unwrap_or_default(),
        location: row.try_get("location")?,
        country: row.try_get("country")?,
        modality: parse::<Modality>("modality", row.try_get("modality")?)?,
        dates: Dates {
            published_at: opt_from_db(row.try_get("published_at")?, "published_at")?,
            starts_at: opt_from_db(row.try_get("starts_at")?, "starts_at")?,
            ends_at: opt_from_db(row.try_get("ends_at")?, "ends_at")?,
            deadline: opt_from_db(row.try_get("deadline")?, "deadline")?,
        },
        cost: cost_from_db(row.try_get("cost_kind")?, row.try_get("cost_detail")?)?,
        requirements: row.try_get("requirements")?,
        eligibility: Eligibility::from_db(row.try_get("eligibility")?),
        perks: Perks {
            certificate: row.try_get("certificate")?,
            prizes: row.try_get("prizes")?,
            benefits: row.try_get("benefits")?,
            travel_covered: row.try_get("travel_covered")?,
            accommodation_covered: row.try_get("accommodation_covered")?,
        },
        discovered_at: from_db(row.try_get("discovered_at")?, "discovered_at")?,
    };
    let state = InteractionState {
        liked: row.try_get("liked")?,
        saved: row.try_get("saved")?,
        dismissed: row.try_get("dismissed")?,
        interesting: row.try_get("interesting")?,
        applied: row.try_get("applied")?,
        enrolled: row.try_get("enrolled")?,
        completed: row.try_get("completed")?,
        last_viewed_at: opt_from_db(row.try_get("last_viewed_at")?, "last_viewed_at")?,
    };
    Ok(FeedItem {
        id: OpportunityId(row.try_get("id")?),
        opportunity,
        state,
    })
}
