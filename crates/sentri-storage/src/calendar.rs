//! SENTRI's own calendar entries.
//!
//! Every row here belongs to SENTRI; the user's personal Google events are
//! never stored or modified. `google_event_id` keeps the sync identifier.

use chrono::{DateTime, Utc};
use sentri_core::calendar::schedulable_dates;
use sentri_core::{Action, CalendarEntry, EntryKind, FeedItem, OpportunityId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::rows::{FEED_COLUMNS, FEED_FROM, feed_item};
use crate::time::{from_db, opt_from_db, to_db};
use crate::{StorageError, Store};

/// Fields to create or update an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub kind: EntryKind,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub notes: Option<String>,
}

fn entry(row: &SqliteRow) -> Result<CalendarEntry, StorageError> {
    let kind: String = row.try_get("kind")?;
    Ok(CalendarEntry {
        id: row.try_get("id")?,
        opportunity_id: row.try_get("opportunity_id")?,
        kind: EntryKind::from_db(&kind).ok_or(StorageError::Corrupt {
            column: "kind",
            value: kind.clone(),
        })?,
        title: row.try_get("title")?,
        starts_at: from_db(row.try_get("starts_at")?, "starts_at")?,
        ends_at: opt_from_db(row.try_get("ends_at")?, "ends_at")?,
        all_day: row.try_get("all_day")?,
        notes: row.try_get("notes")?,
        google_event_id: row.try_get("google_event_id")?,
    })
}

fn validate(new: &NewEntry) -> Result<String, StorageError> {
    let title = sentri_core::text::clean(&new.title, 300);
    if title.is_empty() {
        return Err(StorageError::Invalid("calendar entry needs a title".into()));
    }
    if new.ends_at.is_some_and(|end| end < new.starts_at) {
        return Err(StorageError::Invalid(
            "an entry cannot end before it starts".into(),
        ));
    }
    Ok(title)
}

impl Store {
    /// Adds every date of an opportunity (event, course start/end, deadline)
    /// to the calendar. Existing entries are kept, so repeating it never
    /// creates duplicates. Returns the entries of that opportunity.
    pub async fn schedule_opportunity(
        &self,
        id: OpportunityId,
        at: DateTime<Utc>,
    ) -> Result<Vec<CalendarEntry>, StorageError> {
        let item = self.get(id).await?;
        let dates = schedulable_dates(&item.opportunity);
        if dates.is_empty() {
            return Err(StorageError::Invalid(
                "this opportunity has no date; add one with Edit first".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let mut created = 0;
        for (kind, date) in dates {
            let result = sqlx::query(
                "INSERT INTO calendar_entries
                    (opportunity_id, kind, title, starts_at, all_day, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (opportunity_id, kind) WHERE opportunity_id IS NOT NULL
                 DO NOTHING",
            )
            .bind(id.0)
            .bind(kind.as_str())
            .bind(&item.opportunity.title)
            .bind(to_db(date))
            .bind(sentri_core::dates::is_all_day(date))
            .bind(to_db(at))
            .bind(to_db(at))
            .execute(&mut *tx)
            .await?;
            created += result.rows_affected();
        }
        if created > 0 {
            sqlx::query(
                "INSERT INTO interaction_events (opportunity_id, action, at) VALUES (?, ?, ?)",
            )
            .bind(id.0)
            .bind(Action::Scheduled.as_db())
            .bind(to_db(at))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        let rows = sqlx::query(
            "SELECT * FROM calendar_entries WHERE opportunity_id = ? ORDER BY starts_at",
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(entry).collect()
    }

    pub async fn create_entry(
        &self,
        new: &NewEntry,
        at: DateTime<Utc>,
    ) -> Result<CalendarEntry, StorageError> {
        let title = validate(new)?;
        let row = sqlx::query(
            "INSERT INTO calendar_entries
                (kind, title, starts_at, ends_at, all_day, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
        )
        .bind(new.kind.as_str())
        .bind(title)
        .bind(to_db(new.starts_at))
        .bind(new.ends_at.map(to_db))
        .bind(new.all_day)
        .bind(&new.notes)
        .bind(to_db(at))
        .bind(to_db(at))
        .fetch_one(&self.pool)
        .await?;
        entry(&row)
    }

    pub async fn update_entry(
        &self,
        id: i64,
        new: &NewEntry,
        at: DateTime<Utc>,
    ) -> Result<CalendarEntry, StorageError> {
        let title = validate(new)?;
        let row = sqlx::query(
            "UPDATE calendar_entries SET kind = ?, title = ?, starts_at = ?, ends_at = ?,
                all_day = ?, notes = ?, updated_at = ?
             WHERE id = ? RETURNING *",
        )
        .bind(new.kind.as_str())
        .bind(title)
        .bind(to_db(new.starts_at))
        .bind(new.ends_at.map(to_db))
        .bind(new.all_day)
        .bind(&new.notes)
        .bind(to_db(at))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::NotFound(id))?;
        entry(&row)
    }

    /// Deletes one of SENTRI's own entries.
    pub async fn delete_entry(&self, id: i64) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM calendar_entries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id));
        }
        Ok(())
    }

    /// Entries starting in `[from, to)`.
    pub async fn entries_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<CalendarEntry>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM calendar_entries WHERE starts_at >= ? AND starts_at < ?
             ORDER BY starts_at, id",
        )
        .bind(to_db(from))
        .bind(to_db(to))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(entry).collect()
    }

    /// Non-dismissed opportunities with any date in `[from, to)`, to show
    /// as suggestions next to scheduled entries.
    pub async fn dated_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<FeedItem>, StorageError> {
        let sql = format!(
            "SELECT {FEED_COLUMNS}{FEED_FROM}
             WHERE COALESCE(i.dismissed, 0) = 0 AND (
                (o.starts_at >= ?1 AND o.starts_at < ?2) OR
                (o.deadline >= ?1 AND o.deadline < ?2) OR
                (o.ends_at >= ?1 AND o.ends_at < ?2))"
        );
        // Built only from crate constants; values are bound.
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(to_db(from))
            .bind(to_db(to))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(feed_item).collect()
    }
}
