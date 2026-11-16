//! SQLite persistence for SENTRI.
//!
//! [`Store`] owns a connection pool and exposes repository methods. Schema
//! changes live in versioned files under `migrations/` that are embedded in the
//! binary and applied by [`Store::open`].

mod calendar;
mod error;
mod insights;
mod interactions;
mod layers;
mod opportunities;
mod private_fs;
mod reclassify;
mod rows;
mod runs;
mod time;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub use calendar::NewEntry;
pub use error::StorageError;
pub use insights::{Daily, DayActivity, DayEntry, Insights};
pub use interactions::HistoryEntry;
pub use layers::{EditState, Override};
pub use opportunities::Upsert;
pub use reclassify::StanceChange;
pub use runs::{CollectionRun, RunStatus, Stats};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Handle to the SENTRI database. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens (creating if needed) the database file and applies migrations.
    pub async fn open(path: &Path) -> Result<Self, StorageError> {
        private_fs::prepare_database_file(path)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let store = Self::migrate(pool).await?;
        // WAL and shared-memory files exist once the first write happened.
        private_fs::restrict_sidecars(path)?;
        Ok(store)
    }

    /// Opens a private in-memory database (tests, dry runs).
    pub async fn in_memory() -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // A single connection: every in-memory connection is a separate database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::migrate(pool).await
    }

    async fn migrate(pool: SqlitePool) -> Result<Self, StorageError> {
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// Writes a consistent snapshot of the database to `target` (`VACUUM INTO`).
    pub async fn backup(&self, target: &Path) -> Result<(), StorageError> {
        // Pre-create an empty owner-only file; SQLite accepts an empty target.
        private_fs::create_private_file(target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                StorageError::BackupExists(target.to_path_buf())
            } else {
                StorageError::Io(e)
            }
        })?;
        let target_str = target
            .to_str()
            .ok_or_else(|| StorageError::InvalidPath(target.to_path_buf()))?;
        sqlx::query("VACUUM INTO ?")
            .bind(target_str)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
