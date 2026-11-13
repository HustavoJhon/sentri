//! SQLite persistence for SENTRI.
//!
//! [`Store`] owns a connection pool and exposes repository methods. Schema
//! changes live in versioned files under `migrations/` that are embedded in the
//! binary and applied by [`Store::open`].

mod error;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub use error::StorageError;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Handle to the SENTRI database. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens (creating if needed) the database file and applies migrations.
    pub async fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
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
        restrict_permissions(path);
        Self::migrate(pool).await
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
        if target.exists() {
            return Err(StorageError::BackupExists(target.to_path_buf()));
        }
        let target_str = target
            .to_str()
            .ok_or_else(|| StorageError::InvalidPath(target.to_path_buf()))?;
        sqlx::query("VACUUM INTO ?")
            .bind(target_str)
            .execute(&self.pool)
            .await?;
        restrict_permissions(target);
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// The database holds personal data: make it readable by the owner only.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!(%error, path = %path.display(), "could not restrict database permissions");
        }
    }
}
