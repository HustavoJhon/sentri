//! SENTRI connectors and the safe HTTP client they share.
//!
//! All content obtained here is untrusted: it is size-limited, sanitized by
//! `sentri-core` before storage and never executed.

pub mod connector;
pub mod devevents;
pub mod devpost;
pub mod gdg;
pub mod getonboard;
pub mod github;
pub mod hack0;
pub mod hackernews;
pub mod http;
pub mod ical;
pub mod kaggle;
pub mod metadata;
pub mod net;
pub mod peruanos;
pub mod rss;

pub use connector::{Connector, Incremental, SourceError, SourceSpec};
pub use http::{FetchError, Fetched, HttpLimits, SafeClient, Validators};
