//! SENTRI domain: opportunity models and business rules.
//!
//! This crate is pure: it performs no I/O and has no async runtime, so it can be
//! reused from the TUI, the CLI, the daemon and a future HTTP API.

pub mod category;
pub mod classify;
pub mod edit;
pub mod error;
pub mod interaction;
pub mod link;
pub mod modality;
pub mod opportunity;
pub mod profile;
pub mod query;
pub mod scoring;
pub mod source;
pub mod text;

pub use category::Category;
pub use edit::{EditError, Field, FieldKind};
pub use error::ParseError;
pub use interaction::{Action, Flag, InteractionState};
pub use link::{CanonicalUrl, LinkError};
pub use modality::Modality;
pub use opportunity::{
    Author, ContentKind, Cost, Dates, Eligibility, FeedItem, Opportunity, OpportunityId, Perks,
    Stance,
};
pub use profile::{Interest, Priority, Profile};
pub use query::{FeedQuery, FeedView, PriceFilter, SortOrder};
pub use scoring::{Affinity, Reason, Relevance, Scorer, Weights};
pub use source::SourceKind;
