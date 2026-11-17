//! SENTRI application layer: configuration and use cases.
//!
//! Interfaces (TUI, CLI, daemon, future API) call into this crate instead of
//! talking to storage or connectors directly.

pub mod calendar;
pub mod collect;
pub mod config;
pub mod daemon;
pub mod google;
pub mod images;
pub mod keys;
pub mod links;
pub mod paths;
pub mod profile;
pub mod ranking;
pub mod service;
pub mod settings;

pub use calendar::{CalendarData, GoogleSync};
pub use collect::{CollectReport, draft_config};
pub use config::{
    ColorMode, Config, ConfigError, DaemonConfig, GitHubConfig, HttpConfig, ImageMode,
    SourceConfig, UiConfig,
};
pub use daemon::DaemonError;
pub use images::{ImageError, Thumbnail};
pub use links::{AddLinkError, AddOutcome, LinkOverrides};
pub use profile::GitHubProfile;
pub use ranking::{Ranked, rank};
pub use sentri_storage::{
    CollectionRun, Daily, DayActivity, DayEntry, EditState, HistoryEntry, Insights, NewEntry,
    Override, RunStatus, Sighting, SourceState, StanceChange, Stats,
};
pub use service::{AppError, Sentri};
