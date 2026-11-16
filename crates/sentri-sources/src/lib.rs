//! SENTRI connectors and the safe HTTP client they share.
//!
//! All content obtained here is untrusted: it is size-limited, sanitized by
//! `sentri-core` before storage and never executed.

pub mod connector;
pub mod github;
pub mod hackernews;
pub mod http;
pub mod ical;
pub mod metadata;
pub mod net;
pub mod rss;

pub use connector::{Connector, SourceError, SourceSpec};
pub use http::{FetchError, Fetched, HttpLimits, SafeClient};
