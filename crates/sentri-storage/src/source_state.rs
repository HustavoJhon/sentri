//! Per-source health: last success, last error, back-off and HTTP cache
//! validators for incremental refresh.

use chrono::{DateTime, Duration, Utc};
use sqlx::Row;

use crate::time::{opt_from_db, to_db};
use crate::{StorageError, Store};

/// First back-off after a failure; doubles each time, up to [`MAX_BACKOFF`].
pub const BASE_BACKOFF: Duration = Duration::minutes(15);
pub const MAX_BACKOFF: Duration = Duration::hours(24);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceState {
    pub source_name: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i64,
    /// While in the future, scheduled collection skips the source.
    pub next_attempt_at: Option<DateTime<Utc>>,
}

impl SourceState {
    pub fn backing_off(&self, now: DateTime<Utc>) -> bool {
        self.next_attempt_at.is_some_and(|t| t > now)
    }
}

/// Back-off after `failures` consecutive failures.
pub fn backoff(failures: i64) -> Duration {
    let exponent = u32::try_from((failures - 1).clamp(0, 10)).unwrap_or(10);
    let minutes = BASE_BACKOFF.num_minutes().saturating_mul(1 << exponent);
    Duration::minutes(minutes).min(MAX_BACKOFF)
}

impl Store {
    pub async fn source_state(&self, name: &str) -> Result<SourceState, StorageError> {
        let row = sqlx::query("SELECT * FROM source_state WHERE source_name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(SourceState {
                source_name: name.to_owned(),
                ..SourceState::default()
            });
        };
        Ok(SourceState {
            source_name: row.try_get("source_name")?,
            etag: row.try_get("etag")?,
            last_modified: row.try_get("last_modified")?,
            last_success_at: opt_from_db(row.try_get("last_success_at")?, "last_success_at")?,
            last_error: row.try_get("last_error")?,
            last_error_at: opt_from_db(row.try_get("last_error_at")?, "last_error_at")?,
            consecutive_failures: row.try_get("consecutive_failures")?,
            next_attempt_at: opt_from_db(row.try_get("next_attempt_at")?, "next_attempt_at")?,
        })
    }

    pub async fn source_states(&self) -> Result<Vec<SourceState>, StorageError> {
        let names: Vec<String> =
            sqlx::query_scalar("SELECT source_name FROM source_state ORDER BY source_name")
                .fetch_all(&self.pool)
                .await?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            out.push(self.source_state(&name).await?);
        }
        Ok(out)
    }

    /// Records a success, storing new validators (`None` keeps the old ones).
    pub async fn record_source_success(
        &self,
        name: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO source_state (source_name, etag, last_modified, last_success_at,
                                       consecutive_failures, next_attempt_at)
             VALUES (?, ?, ?, ?, 0, NULL)
             ON CONFLICT (source_name) DO UPDATE SET
                etag = COALESCE(excluded.etag, etag),
                last_modified = COALESCE(excluded.last_modified, last_modified),
                last_success_at = excluded.last_success_at,
                consecutive_failures = 0, next_attempt_at = NULL",
        )
        .bind(name)
        .bind(etag)
        .bind(last_modified)
        .bind(to_db(at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records a failure and schedules the next attempt with back-off.
    pub async fn record_source_failure(
        &self,
        name: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> Result<SourceState, StorageError> {
        let failures = self.source_state(name).await?.consecutive_failures + 1;
        let error: String = sentri_core::text::clean(error, 500);
        sqlx::query(
            "INSERT INTO source_state (source_name, last_error, last_error_at,
                                       consecutive_failures, next_attempt_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (source_name) DO UPDATE SET
                last_error = excluded.last_error, last_error_at = excluded.last_error_at,
                consecutive_failures = excluded.consecutive_failures,
                next_attempt_at = excluded.next_attempt_at",
        )
        .bind(name)
        .bind(&error)
        .bind(to_db(at))
        .bind(failures)
        .bind(to_db(at + backoff(failures)))
        .execute(&self.pool)
        .await?;
        self.source_state(name).await
    }

    /// Forgets the state of a source (after it is deleted from the config).
    pub async fn forget_source(&self, name: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM source_state WHERE source_name = ?")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_is_capped() {
        assert_eq!(backoff(1), Duration::minutes(15));
        assert_eq!(backoff(2), Duration::minutes(30));
        assert_eq!(backoff(4), Duration::hours(2));
        assert_eq!(backoff(50), MAX_BACKOFF);
    }
}
