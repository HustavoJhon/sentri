//! Interaction state and append-only history.

use chrono::{DateTime, Utc};
use sentri_core::{Action, FeedItem, Flag, InteractionState, OpportunityId};
use sqlx::{AssertSqlSafe, Row};

use crate::rows::{FEED_COLUMNS, FEED_FROM, feed_item};
use crate::time::{from_db, opt_to_db, to_db};
use crate::{StorageError, Store};

/// One entry of the history log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub opportunity_id: OpportunityId,
    pub title: String,
    pub action: Action,
    pub at: DateTime<Utc>,
}

impl Store {
    /// Applies `action` atomically: updates the current state and appends it
    /// to the history. Returns the new state.
    pub async fn apply_action(
        &self,
        id: OpportunityId,
        action: Action,
        at: DateTime<Utc>,
    ) -> Result<InteractionState, StorageError> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query(
            "SELECT liked, saved, dismissed, interesting, applied, enrolled, completed,
                    last_viewed_at
             FROM interactions WHERE opportunity_id = ?",
        )
        .bind(id.0)
        .fetch_optional(&mut *tx)
        .await?;
        let state = match current {
            Some(row) => InteractionState {
                liked: row.try_get("liked")?,
                saved: row.try_get("saved")?,
                dismissed: row.try_get("dismissed")?,
                interesting: row.try_get("interesting")?,
                applied: row.try_get("applied")?,
                enrolled: row.try_get("enrolled")?,
                completed: row.try_get("completed")?,
                last_viewed_at: crate::time::opt_from_db(
                    row.try_get("last_viewed_at")?,
                    "last_viewed_at",
                )?,
            },
            None => InteractionState::default(),
        }
        .apply(action, at);

        let result = sqlx::query(
            "INSERT INTO interactions (opportunity_id, liked, saved, dismissed, interesting,
                                       applied, enrolled, completed, last_viewed_at, updated_at)
             SELECT id, ?, ?, ?, ?, ?, ?, ?, ?, ? FROM opportunities WHERE id = ?
             ON CONFLICT (opportunity_id) DO UPDATE SET
                liked = excluded.liked, saved = excluded.saved,
                dismissed = excluded.dismissed, interesting = excluded.interesting,
                applied = excluded.applied, enrolled = excluded.enrolled,
                completed = excluded.completed, last_viewed_at = excluded.last_viewed_at,
                updated_at = excluded.updated_at",
        )
        .bind(state.liked)
        .bind(state.saved)
        .bind(state.dismissed)
        .bind(state.interesting)
        .bind(state.applied)
        .bind(state.enrolled)
        .bind(state.completed)
        .bind(opt_to_db(state.last_viewed_at))
        .bind(to_db(at))
        .bind(id.0)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(id.0));
        }
        sqlx::query("INSERT INTO interaction_events (opportunity_id, action, at) VALUES (?, ?, ?)")
            .bind(id.0)
            .bind(action.as_db())
            .bind(to_db(at))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(state)
    }

    /// Toggles a flag and returns the new state.
    pub async fn toggle_flag(
        &self,
        id: OpportunityId,
        flag: Flag,
        at: DateTime<Utc>,
    ) -> Result<InteractionState, StorageError> {
        let state = self.get(id).await?.state;
        self.apply_action(id, state.toggle(flag), at).await
    }

    /// Most recent actions first.
    pub async fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>, StorageError> {
        let rows = sqlx::query(
            "SELECT e.opportunity_id, o.title, e.action, e.at
             FROM interaction_events e JOIN opportunities o ON o.id = e.opportunity_id
             ORDER BY e.at DESC, e.id DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let action: String = row.try_get("action")?;
                Ok(HistoryEntry {
                    opportunity_id: OpportunityId(row.try_get("opportunity_id")?),
                    title: row.try_get("title")?,
                    action: Action::from_db(&action).map_err(|_| StorageError::Corrupt {
                        column: "action",
                        value: action.clone(),
                    })?,
                    at: from_db(row.try_get("at")?, "at")?,
                })
            })
            .collect()
    }

    /// Items with any positive or negative signal, used to learn affinity.
    pub async fn interacted(&self) -> Result<Vec<FeedItem>, StorageError> {
        let sql = format!(
            "SELECT {FEED_COLUMNS}{FEED_FROM}
             WHERE i.liked = 1 OR i.saved = 1 OR i.dismissed = 1 OR i.interesting = 1
                OR i.applied = 1 OR i.enrolled = 1"
        );
        // Built only from crate constants; no values are interpolated.
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(feed_item).collect()
    }
}
