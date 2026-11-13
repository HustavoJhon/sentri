//! Background collection loop, independent of any interface.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use thiserror::Error;

use crate::Sentri;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("another SENTRI daemon is already running (lock: {})", .0.display())]
    AlreadyRunning(PathBuf),
    #[error("cannot create lock file {}: {source}", path.display())]
    Lock {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Exclusive lock held while the daemon runs; released on drop or crash
/// (the OS releases file locks when the process exits).
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> Result<Self, DaemonError> {
        let lock_err = |source| DaemonError::Lock {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(lock_err)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(lock_err)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(DaemonError::AlreadyRunning(path.to_path_buf()))
            }
            Err(std::fs::TryLockError::Error(e)) => Err(lock_err(e)),
        }
    }
}

impl Sentri {
    /// Lock file placed next to the database.
    pub fn daemon_lock_path(&self) -> PathBuf {
        self.config().database_path().with_extension("daemon.lock")
    }

    /// Collects now and then every `interval` until `shutdown` resolves.
    pub async fn run_daemon(
        &self,
        interval: Duration,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> Result<(), DaemonError> {
        let _lock = InstanceLock::acquire(&self.daemon_lock_path())?;
        match self.store().fail_interrupted_runs(Utc::now()).await {
            Ok(0) => {}
            Ok(n) => tracing::warn!(runs = n, "marked interrupted collection runs as failed"),
            Err(error) => tracing::error!(%error, "could not recover interrupted runs"),
        }
        tracing::info!(interval_minutes = interval.as_secs() / 60, "daemon started");
        tokio::pin!(shutdown);
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                _ = ticker.tick() => self.daemon_cycle().await,
            }
        }
        tracing::info!("daemon stopped");
        Ok(())
    }

    async fn daemon_cycle(&self) {
        let reports = self.collect(None).await;
        for report in &reports {
            match &report.result {
                Ok((seen, new)) => {
                    tracing::info!(source = %report.source, seen, new, "collected");
                }
                Err(error) => tracing::warn!(source = %report.source, %error, "source failed"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sentri.daemon.lock");
        let first = InstanceLock::acquire(&path).unwrap();
        assert!(matches!(
            InstanceLock::acquire(&path),
            Err(DaemonError::AlreadyRunning(_))
        ));
        drop(first);
        assert!(InstanceLock::acquire(&path).is_ok());
    }
}
