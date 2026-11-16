//! Reversible re-classification of stored news stances.
//!
//! Only the *detected* stance changes; manual overrides, likes, saves,
//! dismissals and notes live in other tables and are untouched. Every change
//! is logged per batch so it can be undone.

use chrono::{DateTime, Utc};
use sentri_core::{OpportunityId, Stance, classify};
use sqlx::Row;

use crate::layers::{load_detected, recompute, save_detected};
use crate::time::to_db;
use crate::{StorageError, Store};

/// One stance that changed (or would change in a dry run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StanceChange {
    pub id: OpportunityId,
    pub title: String,
    pub old: Option<Stance>,
    pub new: Stance,
}

impl Store {
    /// Recomputes the stance of every news item with the current rules.
    /// `fallback` gives the configured default stance of a source. Items
    /// whose stance was edited by the user are skipped. With `apply =
    /// false` nothing is written.
    pub async fn reclassify_news(
        &self,
        batch: &str,
        fallback: impl Fn(&str) -> Option<Stance>,
        apply: bool,
        at: DateTime<Utc>,
    ) -> Result<Vec<StanceChange>, StorageError> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            "SELECT o.id, o.source_name FROM opportunities o
             WHERE o.content_kind = 'news'
               AND NOT EXISTS (SELECT 1 FROM opportunity_overrides v
                               WHERE v.opportunity_id = o.id AND v.field = 'stance')
             ORDER BY o.id",
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut changes = Vec::new();
        for row in rows {
            let id: i64 = row.try_get("id")?;
            let source: String = row.try_get("source_name")?;
            let mut detected = load_detected(&mut tx, id).await?;
            let new = classify::stance(
                &detected.title,
                detected.summary.as_deref().unwrap_or_default(),
                fallback(&source),
            );
            let key = classify::news_key(&detected.title, detected.source);
            if apply {
                sqlx::query(
                    "UPDATE opportunities SET news_key = ? WHERE id = ? AND news_key IS NULL",
                )
                .bind(&key)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            }
            if detected.stance == Some(new) {
                continue;
            }
            changes.push(StanceChange {
                id: OpportunityId(id),
                title: detected.title.clone(),
                old: detected.stance,
                new,
            });
            if apply {
                sqlx::query(
                    "INSERT INTO stance_reclassifications
                        (batch, opportunity_id, old_stance, new_stance, at)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(batch)
                .bind(id)
                .bind(detected.stance.map(Stance::as_str))
                .bind(new.as_str())
                .bind(to_db(at))
                .execute(&mut *tx)
                .await?;
                detected.stance = Some(new);
                save_detected(&mut tx, id, &detected).await?;
                recompute(&mut tx, id).await?;
            }
        }
        if apply {
            tx.commit().await?;
        }
        Ok(changes)
    }

    /// Restores the stances changed by `batch` (only where the stance is
    /// still the one the batch set). Returns how many were restored.
    pub async fn undo_reclassification(&self, batch: &str) -> Result<u64, StorageError> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            "SELECT opportunity_id, old_stance, new_stance FROM stance_reclassifications
             WHERE batch = ? ORDER BY id DESC",
        )
        .bind(batch)
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            return Err(StorageError::Invalid(format!(
                "no reclassification batch {batch:?}"
            )));
        }
        let mut restored = 0;
        for row in rows {
            let id: i64 = row.try_get("opportunity_id")?;
            let old: Option<String> = row.try_get("old_stance")?;
            let new: String = row.try_get("new_stance")?;
            let mut detected = load_detected(&mut tx, id).await?;
            if detected.stance.map(Stance::as_str) == Some(new.as_str()) {
                detected.stance = old.as_deref().and_then(Stance::from_db);
                save_detected(&mut tx, id, &detected).await?;
                recompute(&mut tx, id).await?;
                restored += 1;
            }
        }
        sqlx::query("DELETE FROM stance_reclassifications WHERE batch = ?")
            .bind(batch)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(restored)
    }
}
