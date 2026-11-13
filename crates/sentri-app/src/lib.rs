//! SENTRI application layer: configuration and use cases.
//!
//! Interfaces (TUI, CLI, daemon, future API) call into this crate instead of
//! talking to storage or connectors directly.

pub mod calendar;
pub mod collect;
pub mod config;
pub mod daemon;
pub mod images;
pub mod links;
pub mod paths;
pub mod ranking;
pub mod service;

pub use calendar::{CalendarData, GoogleSync};
pub use collect::CollectReport;
pub use config::{
    ColorMode, Config, ConfigError, DaemonConfig, HttpConfig, ImageMode, SourceConfig, UiConfig,
};
pub use daemon::DaemonError;
pub use images::{ImageError, Thumbnail};
pub use links::{AddLinkError, AddOutcome, LinkOverrides};
pub use ranking::{Ranked, rank};
pub use sentri_storage::{
    CollectionRun, Daily, DayActivity, DayEntry, EditState, HistoryEntry, Insights, NewEntry,
    Override, RunStatus, Stats,
};
pub use service::{AppError, Sentri};
