//! SENTRI application layer: configuration and use cases.
//!
//! Interfaces (TUI, CLI, daemon, future API) call into this crate instead of
//! talking to storage or connectors directly.

pub mod collect;
pub mod config;
pub mod daemon;
pub mod links;
pub mod paths;
pub mod ranking;
pub mod service;

pub use collect::CollectReport;
pub use config::{Config, ConfigError, DaemonConfig, HttpConfig, SourceConfig};
pub use daemon::DaemonError;
pub use links::{AddLinkError, AddOutcome, LinkOverrides};
pub use ranking::{Ranked, rank};
pub use sentri_storage::{
    CollectionRun, Daily, DayActivity, DayEntry, EditState, HistoryEntry, Insights, Override,
    RunStatus, Stats,
};
pub use service::{AppError, Sentri};
