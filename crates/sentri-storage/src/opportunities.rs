//! Opportunity repository: insertion with cross-source deduplication,
//! updates and feed queries.

use chrono::{DateTime, Utc};
use sentri_core::{
    FeedItem, FeedQuery, FeedView, Flag, Opportunity, OpportunityId, PriceFilter, SortOrder,
};
use sqlx::{AssertSqlSafe, QueryBuilder, Sqlite, SqliteConnection};

use crate::rows::{FEED_COLUMNS, FEED_FROM, cost_to_db, feed_item};
use crate::time::{opt_to_db, to_db};
use crate::{StorageError, Store};

/// Result of storing an opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upsert {
    /// It was new.
    Inserted(OpportunityId),
    /// It already existed (same canonical URL or same source item) and the
    /// stored copy was completed with any newly known fields.
    Merged(OpportunityId),
}

impl Upsert {
    pub const fn id(self) -> OpportunityId {
        match self {
            Self::Inserted(id) | Self::Merged(id) => id,
        }
    }

    pub const fn is_new(self) -> bool {
        matches!(self, Self::Inserted(_))
    }
}

impl Store {
    /// Inserts an opportunity or merges it into an existing duplicate.
    pub async fn upsert_opportunity(&self, o: &Opportunity) -> Result<Upsert, StorageError> {
        let mut tx = self.write_tx().await?;
        let now = to_db(Utc::now());
        let existing = find_duplicate(&mut tx, o).await?;
        let outcome = if let Some(id) = existing {
            crate::layers::merge_sighting(&mut tx, id, o).await?;
            Upsert::Merged(OpportunityId(id))
        } else {
            let id = insert(&mut tx, o, now).await?;
            add_tags(&mut tx, id, &o.tags).await?;
            crate::layers::save_detected(&mut tx, id, o).await?;
            Upsert::Inserted(OpportunityId(id))
        };
        let id = outcome.id().0;
        sqlx::query(
            "INSERT INTO opportunity_sightings
                 (opportunity_id, source_name, external_id, original_url, first_seen_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (opportunity_id, source_name)
             DO UPDATE SET last_seen_at = excluded.last_seen_at",
        )
        .bind(id)
        .bind(&o.source_name)
        .bind(&o.external_id)
        .bind(&o.original_url)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Replaces the detected data of an opportunity (e.g. after re-reading
    /// its page). Manual overrides still apply on top. URL identity,
    /// fingerprint and discovery date are kept.
    pub async fn update_opportunity(
        &self,
        id: OpportunityId,
        o: &Opportunity,
    ) -> Result<(), StorageError> {
        let mut tx = self.write_tx().await?;
        let current = crate::layers::load_detected(&mut tx, id.0).await?;
        let mut detected = o.clone();
        detected.fingerprint = current.fingerprint;
        detected.canonical_url = current.canonical_url;
        detected.discovered_at = current.discovered_at;
        crate::layers::save_detected(&mut tx, id.0, &detected).await?;
        crate::layers::recompute(&mut tx, id.0).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get(&self, id: OpportunityId) -> Result<FeedItem, StorageError> {
        let sql = format!("SELECT {FEED_COLUMNS}{FEED_FROM} WHERE o.id = ?");
        let row = // Built only from crate constants; every value is bound.
        sqlx::query(AssertSqlSafe(sql))
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StorageError::NotFound(id.0))?;
        feed_item(&row)
    }

    pub async fn find_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<FeedItem>, StorageError> {
        let sql = format!("SELECT {FEED_COLUMNS}{FEED_FROM} WHERE o.fingerprint = ?");
        // Built only from crate constants; every value is bound.
        sqlx::query(AssertSqlSafe(sql))
            .bind(fingerprint)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(feed_item)
            .transpose()
    }

    /// Returns the items matching `query`. Relevance ordering is applied by
    /// the caller, which owns the profile; here it falls back to newest first.
    pub async fn list(
        &self,
        query: &FeedQuery,
        now: DateTime<Utc>,
    ) -> Result<Vec<FeedItem>, StorageError> {
        let mut qb =
            QueryBuilder::<Sqlite>::new(format!("SELECT {FEED_COLUMNS}{FEED_FROM} WHERE 1 = 1"));
        push_filters(&mut qb, query, now);
        push_order(&mut qb, query);
        if let Some(limit) = query.limit {
            qb.push(" LIMIT ").push_bind(i64::from(limit));
        }
        let rows = qb.build().fetch_all(&self.pool).await?;
        rows.iter().map(feed_item).collect()
    }

    /// Number of stored opportunities.
    pub async fn count(&self) -> Result<i64, StorageError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM opportunities")
            .fetch_one(&self.pool)
            .await?)
    }
}

async fn find_duplicate(
    conn: &mut SqliteConnection,
    o: &Opportunity,
) -> Result<Option<i64>, StorageError> {
    let by_url: Option<i64> =
        sqlx::query_scalar("SELECT id FROM opportunities WHERE fingerprint = ?")
            .bind(&o.fingerprint)
            .fetch_optional(&mut *conn)
            .await?;
    if by_url.is_some() {
        return Ok(by_url);
    }
    if let Some(id) = find_same_story(conn, o).await? {
        return Ok(Some(id));
    }
    let Some(external_id) = &o.external_id else {
        return Ok(None);
    };
    Ok(sqlx::query_scalar(
        "SELECT opportunity_id FROM opportunity_sightings WHERE source_name = ? AND external_id = ?",
    )
    .bind(&o.source_name)
    .bind(external_id)
    .fetch_optional(&mut *conn)
    .await?)
}

/// The same news story already stored from another feed: same normalized
/// headline, published within 48 hours.
async fn find_same_story(
    conn: &mut SqliteConnection,
    o: &Opportunity,
) -> Result<Option<i64>, StorageError> {
    if o.content != sentri_core::ContentKind::News {
        return Ok(None);
    }
    let Some(key) = sentri_core::classify::news_key(&o.title, o.source) else {
        return Ok(None);
    };
    let at = to_db(o.dates.published_at.unwrap_or(o.discovered_at));
    Ok(sqlx::query_scalar(
        "SELECT id FROM opportunities
         WHERE news_key = ? AND content_kind = 'news'
           AND ABS(COALESCE(published_at, discovered_at) - ?) <= 172800
         ORDER BY id LIMIT 1",
    )
    .bind(key)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await?)
}

async fn insert(
    conn: &mut SqliteConnection,
    o: &Opportunity,
    now: i64,
) -> Result<i64, StorageError> {
    let (cost_kind, cost_detail) = cost_to_db(&o.cost);
    let id = sqlx::query_scalar(
        "INSERT INTO opportunities (
            fingerprint, canonical_url, original_url, source_kind, source_name, external_id,
            title, summary, image_url, category, location, country, modality, published_at,
            starts_at, ends_at, deadline, cost_kind, cost_detail, requirements, eligibility,
            certificate, prizes, benefits, travel_covered, accommodation_covered,
            discovered_at, updated_at, author_name, author_handle, author_url, author_avatar,
            content_kind, stance, news_key)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                 ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&o.fingerprint)
    .bind(&o.canonical_url)
    .bind(&o.original_url)
    .bind(o.source.as_str())
    .bind(&o.source_name)
    .bind(&o.external_id)
    .bind(&o.title)
    .bind(&o.summary)
    .bind(&o.image_url)
    .bind(o.category.as_str())
    .bind(&o.location)
    .bind(&o.country)
    .bind(o.modality.as_str())
    .bind(opt_to_db(o.dates.published_at))
    .bind(opt_to_db(o.dates.starts_at))
    .bind(opt_to_db(o.dates.ends_at))
    .bind(opt_to_db(o.dates.deadline))
    .bind(cost_kind)
    .bind(cost_detail)
    .bind(&o.requirements)
    .bind(o.eligibility.as_str())
    .bind(o.perks.certificate)
    .bind(&o.perks.prizes)
    .bind(&o.perks.benefits)
    .bind(o.perks.travel_covered)
    .bind(o.perks.accommodation_covered)
    .bind(to_db(o.discovered_at))
    .bind(now)
    .bind(&o.author.name)
    .bind(&o.author.handle)
    .bind(&o.author.profile_url)
    .bind(&o.author.avatar_url)
    .bind(o.content.as_str())
    .bind(o.stance.map(sentri_core::Stance::as_str))
    .bind(
        (o.content == sentri_core::ContentKind::News)
            .then(|| sentri_core::classify::news_key(&o.title, o.source))
            .flatten(),
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

/// Writes effective values (detected + overrides) into the row and tags.
pub(crate) async fn write_effective(
    conn: &mut SqliteConnection,
    id: i64,
    o: &Opportunity,
) -> Result<(), StorageError> {
    let (cost_kind, cost_detail) = cost_to_db(&o.cost);
    let result = sqlx::query(
        "UPDATE opportunities SET
            title = ?, summary = ?, image_url = ?, category = ?, location = ?, country = ?,
            modality = ?, published_at = ?, starts_at = ?, ends_at = ?, deadline = ?,
            cost_kind = ?, cost_detail = ?, requirements = ?, eligibility = ?,
            certificate = ?, prizes = ?, benefits = ?, travel_covered = ?,
            accommodation_covered = ?, author_name = ?, author_handle = ?, author_url = ?,
            author_avatar = ?, content_kind = ?, stance = ?, source_kind = ?, original_url = ?,
            updated_at = ?
         WHERE id = ?",
    )
    .bind(&o.title)
    .bind(&o.summary)
    .bind(&o.image_url)
    .bind(o.category.as_str())
    .bind(&o.location)
    .bind(&o.country)
    .bind(o.modality.as_str())
    .bind(opt_to_db(o.dates.published_at))
    .bind(opt_to_db(o.dates.starts_at))
    .bind(opt_to_db(o.dates.ends_at))
    .bind(opt_to_db(o.dates.deadline))
    .bind(cost_kind)
    .bind(cost_detail)
    .bind(&o.requirements)
    .bind(o.eligibility.as_str())
    .bind(o.perks.certificate)
    .bind(&o.perks.prizes)
    .bind(&o.perks.benefits)
    .bind(o.perks.travel_covered)
    .bind(o.perks.accommodation_covered)
    .bind(&o.author.name)
    .bind(&o.author.handle)
    .bind(&o.author.profile_url)
    .bind(&o.author.avatar_url)
    .bind(o.content.as_str())
    .bind(o.stance.map(sentri_core::Stance::as_str))
    .bind(o.source.as_str())
    .bind(&o.original_url)
    .bind(to_db(Utc::now()))
    .bind(id)
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(StorageError::NotFound(id));
    }
    sqlx::query("DELETE FROM opportunity_tags WHERE opportunity_id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    add_tags(conn, id, &o.tags).await
}

async fn add_tags(
    conn: &mut SqliteConnection,
    id: i64,
    tags: &[String],
) -> Result<(), StorageError> {
    for tag in tags {
        sqlx::query("INSERT OR IGNORE INTO opportunity_tags (opportunity_id, tag) VALUES (?, ?)")
            .bind(id)
            .bind(tag)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Column backing each interaction flag. Fixed strings, never user input.
const fn flag_column(flag: Flag) -> &'static str {
    match flag {
        Flag::Liked => "i.liked",
        Flag::Saved => "i.saved",
        Flag::Dismissed => "i.dismissed",
        Flag::Interesting => "i.interesting",
        Flag::Applied => "i.applied",
        Flag::Enrolled => "i.enrolled",
        Flag::Completed => "i.completed",
    }
}

/// Escapes `%`, `_` and `\` for a `LIKE ... ESCAPE '\'` pattern.
fn like_pattern(word: &str) -> String {
    let mut out = String::with_capacity(word.len() + 2);
    out.push('%');
    for ch in word.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('%');
    out
}

fn push_in<T>(
    qb: &mut QueryBuilder<Sqlite>,
    column: &str,
    values: impl IntoIterator<Item = T>,
    to_str: impl Fn(T) -> &'static str,
) {
    qb.push(format!(" AND {column} IN ("));
    let mut separated = qb.separated(", ");
    for value in values {
        separated.push_bind(to_str(value));
    }
    qb.push(")");
}

fn push_filters(qb: &mut QueryBuilder<Sqlite>, q: &FeedQuery, now: DateTime<Utc>) {
    let view = q.view;
    if view.hides_dismissed() {
        qb.push(" AND COALESCE(i.dismissed, 0) = 0");
    }
    if let Some(flag) = view.required_flag() {
        qb.push(format!(" AND {} = 1", flag_column(flag)));
    }
    if let Some(content) = view.content() {
        qb.push(" AND o.content_kind = ")
            .push_bind(content.as_str());
    }
    if view == FeedView::History {
        qb.push(" AND i.last_viewed_at IS NOT NULL");
    }
    if view == FeedView::Travel {
        qb.push(" AND (o.travel_covered = 1 OR o.accommodation_covered = 1)");
    }
    if let Some(categories) = view.categories() {
        push_in(
            qb,
            "o.category",
            categories.iter().copied(),
            sentri_core::Category::as_str,
        );
    }
    if !q.categories.is_empty() {
        push_in(qb, "o.category", q.categories.iter().copied(), |c| {
            c.as_str()
        });
    }
    if !q.sources.is_empty() {
        push_in(qb, "o.source_kind", q.sources.iter().copied(), |s| {
            s.as_str()
        });
    }
    if !q.modalities.is_empty() {
        push_in(qb, "o.modality", q.modalities.iter().copied(), |m| {
            m.as_str()
        });
    }
    for word in q.words() {
        let pattern = like_pattern(&word);
        qb.push(" AND (o.title LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR COALESCE(o.summary, '') LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR COALESCE(o.location, '') LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR EXISTS (SELECT 1 FROM opportunity_tags t WHERE t.opportunity_id = o.id AND t.tag LIKE ")
            .push_bind(pattern)
            .push(" ESCAPE '\\'))");
    }
    if let Some(location) = q.location.as_deref().filter(|l| !l.trim().is_empty()) {
        let pattern = like_pattern(location.trim());
        qb.push(" AND (COALESCE(o.location, '') LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR COALESCE(o.country, '') LIKE ")
            .push_bind(pattern)
            .push(" ESCAPE '\\')");
    }
    match q.price {
        Some(PriceFilter::Free) => {
            qb.push(" AND o.cost_kind IN ('free', 'free_with_paid_certificate')");
        }
        Some(PriceFilter::Paid) => {
            qb.push(" AND o.cost_kind = 'paid'");
        }
        None => {}
    }
    if q.with_certificate {
        qb.push(" AND o.certificate = 1");
    }
    if q.with_prizes {
        qb.push(" AND o.prizes IS NOT NULL");
    }
    if q.with_travel {
        qb.push(" AND (o.travel_covered = 1 OR o.accommodation_covered = 1)");
    }
    if q.open_to_country {
        qb.push(" AND o.eligibility = 'open'");
    }
    if let Some(before) = q.deadline_before {
        qb.push(" AND o.deadline <= ").push_bind(to_db(before));
    }
    if q.only_upcoming {
        // All-day deadlines (midnight UTC) stay open until the end of the day.
        qb.push(" AND (COALESCE(o.deadline, o.starts_at) IS NULL OR COALESCE(o.deadline, o.starts_at) >= ")
            .push_bind(to_db(now))
            .push(" OR (o.deadline IS NOT NULL AND o.deadline % 86400 = 0 AND o.deadline + 86400 >= ")
            .push_bind(to_db(now))
            .push("))");
    }
    if q.only_past {
        qb.push(" AND COALESCE(o.deadline, o.starts_at) IS NOT NULL AND COALESCE(o.deadline, o.starts_at) < ")
            .push_bind(to_db(now))
            .push(" AND NOT (o.deadline IS NOT NULL AND o.deadline % 86400 = 0 AND o.deadline + 86400 >= ")
            .push_bind(to_db(now))
            .push(")");
    }
}

fn push_order(qb: &mut QueryBuilder<Sqlite>, q: &FeedQuery) {
    let order = match (q.view, q.sort) {
        (FeedView::History, _) => " ORDER BY i.last_viewed_at DESC, o.id DESC",
        (_, SortOrder::Deadline) => {
            " ORDER BY o.deadline IS NULL, o.deadline ASC, o.discovered_at DESC, o.id DESC"
        }
        // News is ordered by publication; opportunities by discovery.
        (FeedView::News, SortOrder::Newest | SortOrder::Relevance) => {
            " ORDER BY COALESCE(o.published_at, o.discovered_at) DESC, o.id DESC"
        }
        (_, SortOrder::Newest | SortOrder::Relevance) => {
            " ORDER BY o.discovered_at DESC, o.id DESC"
        }
    };
    qb.push(order);
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }
}
