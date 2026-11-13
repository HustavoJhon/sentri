//! SENTRI application layer: configuration and use cases.
//!
//! Interfaces (TUI, CLI, daemon, future API) call into this crate instead of
//! talking to storage or connectors directly.

pub mod config;
pub mod paths;
pub mod ranking;

pub use config::{Config, ConfigError, HttpConfig, SourceConfig};
pub use ranking::{Ranked, rank};
