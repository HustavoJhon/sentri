use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt value in column {column}: {value:?}")]
    Corrupt { column: &'static str, value: String },
    #[error("opportunity {0} not found")]
    NotFound(i64),
    #[error("backup target already exists: {}", .0.display())]
    BackupExists(PathBuf),
    #[error("path is not valid UTF-8: {}", .0.display())]
    InvalidPath(PathBuf),
}
