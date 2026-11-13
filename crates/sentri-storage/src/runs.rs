//! Collection runs (one per connector execution) and dashboard statistics.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::time::{from_db, opt_from_db, to_db};
use crate::{StorageError, Store};

/// Outcome of a connector execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
}

impl RunStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn from_db(s: &str) -> Result<Self, StorageError> {
        match s {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            other => Err(StorageError::Corrupt {
                column: "status",
                value: other.to_owned(),
            }),
        }
    }
}

/// A recorded connector execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionRun {
    pub id: i64,
    pub source_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub items_seen: i64,
    pub items_new: i64,
    pub error: Option<String>,
}

/// Counters shown on the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub total: i64,
    pub new_since: i64,
    pub liked: i64,
    pub saved: i64,
    pub applied: i64,
    pub upcoming_deadlines: i64,
}

impl Store {
    pub async fn start_run(
        &self,
        source_name: &str,
        at: DateTime<Utc>,
    ) -> Result<i64, StorageError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO collection_runs (source_name, started_at, status) VALUES (?, ?, ?)
             RETURNING id",
        )
        .bind(source_name)
        .bind(to_db(at))
        .bind(RunStatus::Running.as_str())
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn finish_run(
        &self,
        run_id: i64,
        at: DateTime<Utc>,
        result: Result<(i64, i64), &str>,
    ) -> Result<(), StorageError> {
        let (status, seen, new, error) = match result {
            Ok((seen, new)) => (RunStatus::Succeeded, seen, new, None),
            Err(e) => (RunStatus::Failed, 0, 0, Some(e)),
        };
        sqlx::query(
            "UPDATE collection_runs
             SET finished_at = ?, status = ?, items_seen = ?, items_new = ?, error = ?
             WHERE id = ?",
        )
        .bind(to_db(at))
        .bind(status.as_str())
        .bind(seen)
        .bind(new)
        .bind(error)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Marks runs left as `running` by a crash or kill as failed. Returns how
    /// many were recovered.
    pub async fn fail_interrupted_runs(&self, at: DateTime<Utc>) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE collection_runs SET status = 'failed', finished_at = ?, error = 'interrupted'
             WHERE status = 'running'",
        )
        .bind(to_db(at))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Latest run of every source.
    pub async fn latest_runs(&self) -> Result<Vec<CollectionRun>, StorageError> {
        let rows = sqlx::query(
            "SELECT r.* FROM collection_runs r
             WHERE r.id = (SELECT MAX(id) FROM collection_runs WHERE source_name = r.source_name)
             ORDER BY r.source_name",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(CollectionRun {
                    id: row.try_get("id")?,
                    source_name: row.try_get("source_name")?,
                    started_at: from_db(row.try_get("started_at")?, "started_at")?,
                    finished_at: opt_from_db(row.try_get("finished_at")?, "finished_at")?,
                    status: RunStatus::from_db(row.try_get("status")?)?,
                    items_seen: row.try_get("items_seen")?,
                    items_new: row.try_get("items_new")?,
                    error: row.try_get("error")?,
                })
            })
            .collect()
    }

    /// Dashboard counters; `since` defines what counts as new.
    pub async fn stats(
        &self,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Stats, StorageError> {
        let row = sqlx::query(
            "SELECT
                (SELECT COUNT(*) FROM opportunities) AS total,
                (SELECT COUNT(*) FROM opportunities WHERE discovered_at >= ?) AS new_since,
                (SELECT COUNT(*) FROM interactions WHERE liked = 1) AS liked,
                (SELECT COUNT(*) FROM interactions WHERE saved = 1) AS saved,
                (SELECT COUNT(*) FROM interactions WHERE applied = 1) AS applied,
                (SELECT COUNT(*) FROM opportunities o
                   LEFT JOIN interactions i ON i.opportunity_id = o.id
                   WHERE o.deadline >= ? AND COALESCE(i.dismissed, 0) = 0) AS upcoming",
        )
        .bind(to_db(since))
        .bind(to_db(now))
        .fetch_one(&self.pool)
        .await?;
        Ok(Stats {
            total: row.try_get("total")?,
            new_since: row.try_get("new_since")?,
            liked: row.try_get("liked")?,
            saved: row.try_get("saved")?,
            applied: row.try_get("applied")?,
            upcoming_deadlines: row.try_get("upcoming")?,
        })
    }
}
