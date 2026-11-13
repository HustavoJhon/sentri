//! SENTRI application layer: configuration and use cases.
//!
//! Interfaces (TUI, CLI, daemon, future API) call into this crate instead of
//! talking to storage or connectors directly.

pub mod config;
pub mod links;
pub mod paths;
pub mod ranking;
pub mod service;

pub use config::{Config, ConfigError, HttpConfig, SourceConfig};
pub use links::{AddLinkError, AddOutcome, LinkOverrides};
pub use ranking::{Ranked, rank};
pub use sentri_storage::{CollectionRun, HistoryEntry, RunStatus, Stats};
pub use service::{AppError, Sentri};
