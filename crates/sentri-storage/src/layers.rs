//! Detected data, manual overrides and personal annotations.
//!
//! - `opportunity_sightings`: original URL and external id per source.
//! - `opportunity_detected`: normalized data merged from every sighting.
//! - `opportunity_overrides`: fields the user corrected.
//! - `opportunities`: effective values = detected + overrides (what queries
//!   filter and search on).
//!
//! Collections only ever touch the detected snapshot, so manual edits
//! survive; deleting an override restores the detected value.

use chrono::{DateTime, Utc};
use sentri_core::edit::{apply_overrides, merge_detected};
use sentri_core::{Action, Annotations, Field, Opportunity, OpportunityId, Priority};
use sqlx::{AssertSqlSafe, Row, SqliteConnection};

use crate::opportunities::write_effective;
use crate::rows::{FEED_COLUMNS, FEED_FROM, feed_item};
use crate::time::{from_db, to_db};
use crate::{StorageError, Store};

/// One manual override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub field: Field,
    pub value: String,
    pub edited_at: DateTime<Utc>,
}

/// Everything the editor needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditState {
    pub detected: Opportunity,
    pub effective: Opportunity,
    pub overrides: Vec<Override>,
    pub annotations: Annotations,
}

impl EditState {
    pub fn is_overridden(&self, field: Field) -> bool {
        self.overrides.iter().any(|o| o.field == field)
    }
}

pub(crate) async fn save_detected(
    conn: &mut SqliteConnection,
    id: i64,
    o: &Opportunity,
) -> Result<(), StorageError> {
    let data = serde_json::to_string(o).map_err(|e| StorageError::Invalid(e.to_string()))?;
    sqlx::query(
        "INSERT INTO opportunity_detected (opportunity_id, data, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (opportunity_id) DO UPDATE SET data = excluded.data,
                                                    updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(data)
    .bind(to_db(Utc::now()))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Detected snapshot; rows created before snapshots existed fall back to
/// their stored values (which had no overrides at that time).
pub(crate) async fn load_detected(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Opportunity, StorageError> {
    let data: Option<String> =
        sqlx::query_scalar("SELECT data FROM opportunity_detected WHERE opportunity_id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some(json) = data
        && let Ok(o) = serde_json::from_str(&json)
    {
        return Ok(o);
    }
    let sql = format!("SELECT {FEED_COLUMNS}{FEED_FROM} WHERE o.id = ?");
    // Built only from crate constants; the id is bound.
    let row = sqlx::query(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(StorageError::NotFound(id))?;
    Ok(feed_item(&row)?.opportunity)
}

async fn load_overrides(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Vec<Override>, StorageError> {
    let rows = sqlx::query(
        "SELECT field, value, edited_at FROM opportunity_overrides
         WHERE opportunity_id = ? ORDER BY field",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let field: String = row.try_get("field")?;
        // Unknown field names (from a newer version) are ignored, not fatal.
        let Ok(field) = field.parse::<Field>() else {
            continue;
        };
        out.push(Override {
            field,
            value: row
                .try_get::<Option<String>, _>("value")?
                .unwrap_or_default(),
            edited_at: from_db(row.try_get("edited_at")?, "edited_at")?,
        });
    }
    Ok(out)
}

fn effective(detected: &Opportunity, overrides: &[Override]) -> Opportunity {
    let (effective, errors) = apply_overrides(
        detected,
        overrides.iter().map(|o| (o.field, o.value.as_str())),
    );
    for error in errors {
        tracing::warn!(%error, "ignoring invalid stored override");
    }
    effective
}

/// Rewrites effective values from the detected snapshot and overrides.
pub(crate) async fn recompute(conn: &mut SqliteConnection, id: i64) -> Result<(), StorageError> {
    let detected = load_detected(conn, id).await?;
    let overrides = load_overrides(conn, id).await?;
    write_effective(conn, id, &effective(&detected, &overrides)).await
}

/// Folds a new sighting into the detected snapshot and refreshes the row.
pub(crate) async fn merge_sighting(
    conn: &mut SqliteConnection,
    id: i64,
    new: &Opportunity,
) -> Result<(), StorageError> {
    let mut detected = load_detected(conn, id).await?;
    merge_detected(&mut detected, new);
    save_detected(conn, id, &detected).await?;
    recompute(conn, id).await
}

async fn log_action(
    conn: &mut SqliteConnection,
    id: i64,
    action: Action,
    at: DateTime<Utc>,
) -> Result<(), StorageError> {
    sqlx::query("INSERT INTO interaction_events (opportunity_id, action, at) VALUES (?, ?, ?)")
        .bind(id)
        .bind(action.as_db())
        .bind(to_db(at))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

impl Store {
    /// Stores manual values for several fields atomically. Every value is
    /// validated first; on any error nothing is written.
    pub async fn set_overrides(
        &self,
        id: OpportunityId,
        values: &[(Field, String)],
        at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut probe = load_detected(&mut tx, id.0).await?;
        for (field, value) in values {
            field
                .set(&mut probe, value)
                .map_err(|e| StorageError::Invalid(e.to_string()))?;
        }
        for (field, value) in values {
            sqlx::query(
                "INSERT INTO opportunity_overrides (opportunity_id, field, value, edited_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT (opportunity_id, field)
                 DO UPDATE SET value = excluded.value, edited_at = excluded.edited_at",
            )
            .bind(id.0)
            .bind(field.as_str())
            .bind(value.trim())
            .bind(to_db(at))
            .execute(&mut *tx)
            .await?;
        }
        recompute(&mut tx, id.0).await?;
        if !values.is_empty() {
            log_action(&mut tx, id.0, Action::Edited, at).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Drops manual values so the detected ones apply again.
    pub async fn reset_overrides(
        &self,
        id: OpportunityId,
        fields: &[Field],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        for field in fields {
            sqlx::query("DELETE FROM opportunity_overrides WHERE opportunity_id = ? AND field = ?")
                .bind(id.0)
                .bind(field.as_str())
                .execute(&mut *tx)
                .await?;
        }
        recompute(&mut tx, id.0).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Replaces notes, priority and custom fields.
    pub async fn set_annotations(
        &self,
        id: OpportunityId,
        a: &Annotations,
        at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        let notes = a.notes.as_deref().map(str::trim).filter(|n| !n.is_empty());
        sqlx::query(
            "INSERT INTO opportunity_notes (opportunity_id, notes, priority, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (opportunity_id) DO UPDATE SET notes = excluded.notes,
                 priority = excluded.priority, updated_at = excluded.updated_at",
        )
        .bind(id.0)
        .bind(notes)
        .bind(a.priority.map(Priority::as_str))
        .bind(to_db(at))
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM opportunity_custom_fields WHERE opportunity_id = ?")
            .bind(id.0)
            .execute(&mut *tx)
            .await?;
        for (key, value) in &a.custom {
            let key = sentri_core::text::clean(key, 60);
            if key.is_empty() {
                continue;
            }
            sqlx::query(
                "INSERT OR REPLACE INTO opportunity_custom_fields (opportunity_id, key, value)
                 VALUES (?, ?, ?)",
            )
            .bind(id.0)
            .bind(key)
            .bind(sentri_core::text::clean(value, 1_000))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Detected data, overrides and annotations of one opportunity.
    pub async fn edit_state(&self, id: OpportunityId) -> Result<EditState, StorageError> {
        let mut conn = self.pool.acquire().await?;
        let detected = load_detected(&mut conn, id.0).await?;
        let overrides = load_overrides(&mut conn, id.0).await?;
        let notes =
            sqlx::query("SELECT notes, priority FROM opportunity_notes WHERE opportunity_id = ?")
                .bind(id.0)
                .fetch_optional(&mut *conn)
                .await?;
        let custom = sqlx::query(
            "SELECT key, value FROM opportunity_custom_fields WHERE opportunity_id = ? ORDER BY key",
        )
        .bind(id.0)
        .fetch_all(&mut *conn)
        .await?;
        let mut annotations = Annotations::default();
        if let Some(row) = notes {
            annotations.notes = row.try_get("notes")?;
            annotations.priority = row
                .try_get::<Option<String>, _>("priority")?
                .as_deref()
                .and_then(Priority::from_db);
        }
        for row in custom {
            annotations
                .custom
                .push((row.try_get("key")?, row.try_get("value")?));
        }
        Ok(EditState {
            effective: effective(&detected, &overrides),
            detected,
            overrides,
            annotations,
        })
    }
}
