//! Connector abstraction.
//!
//! Connectors form a closed set (an enum): adding one means adding a variant,
//! which keeps dispatch simple and avoids boxed async traits.

use chrono::{DateTime, Utc};
use sentri_core::{Category, ContentKind, Opportunity, SourceKind, Stance, classify};
use thiserror::Error;

use crate::SafeClient;
use crate::gdg::Gdg;
use crate::github::GitHub;
use crate::hackernews::HackerNews;
use crate::http::FetchError;
use crate::ical::Calendar;
use crate::rss::Feed;

/// Everything a connector needs, independent of the configuration format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpec {
    pub name: String,
    pub kind: SourceKind,
    pub url: Option<String>,
    pub query: Option<String>,
    /// Connector-specific target (e.g. GitHub `issues` or `repositories`).
    pub target: Option<String>,
    /// Category used when the item does not reveal one.
    pub category: Option<Category>,
    pub tags: Vec<String>,
    pub max_items: usize,
    /// Secret token, already read from the environment. Never logged.
    pub token: Option<String>,
    /// Whether the source publishes opportunities or news.
    pub content: ContentKind,
    /// Default stance for news (e.g. `Official` for a company blog).
    pub stance: Option<Stance>,
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("{0}")]
    Fetch(#[from] FetchError),
    #[error("cannot parse response: {0}")]
    Parse(String),
    #[error("source misconfigured: {0}")]
    Config(String),
    #[error("{0} is not supported as an automatic source (see docs/sources.md)")]
    Unsupported(SourceKind),
}

/// A configured, ready-to-run connector.
#[derive(Debug, Clone)]
pub enum Connector {
    Feed(Feed),
    GitHub(GitHub),
    HackerNews(HackerNews),
    Calendar(Calendar),
    Gdg(Gdg),
}

impl Connector {
    /// Builds the connector for `spec`, validating its configuration.
    pub fn from_spec(spec: SourceSpec) -> Result<Self, SourceError> {
        match spec.kind {
            // Platforms that publish standard feeds use the feed connector.
            SourceKind::Rss
            | SourceKind::GoogleNews
            | SourceKind::Mastodon
            | SourceKind::Bluesky
            | SourceKind::YouTube => Feed::new(spec).map(Self::Feed),
            SourceKind::GitHub => GitHub::new(spec).map(Self::GitHub),
            SourceKind::HackerNews => HackerNews::new(spec).map(Self::HackerNews),
            SourceKind::Gdg => Gdg::new(spec).map(Self::Gdg),
            // Calendar feeds (`.ics`, `webcal://`).
            SourceKind::ICal | SourceKind::Luma | SourceKind::Meetup => {
                Calendar::new(spec).map(Self::Calendar)
            }
            other => Err(SourceError::Unsupported(other)),
        }
    }

    pub fn spec(&self) -> &SourceSpec {
        match self {
            Self::Feed(c) => &c.spec,
            Self::GitHub(c) => &c.spec,
            Self::HackerNews(c) => &c.spec,
            Self::Calendar(c) => &c.spec,
            Self::Gdg(c) => &c.spec,
        }
    }

    /// Downloads and normalizes items. Items are sanitized and capped at
    /// `max_items`.
    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let items = match self {
            Self::Feed(c) => c.fetch(http, now).await?,
            Self::GitHub(c) => c.fetch(http, now).await?,
            Self::HackerNews(c) => c.fetch(http, now).await?,
            Self::Calendar(c) => c.fetch(http, now).await?,
            Self::Gdg(c) => c.fetch(http, now).await?,
        };
        let spec = self.spec();
        Ok(items
            .into_iter()
            .take(spec.max_items)
            .map(|o| finish(o, spec))
            .collect())
    }
}

/// Applies source defaults and classification, then sanitizes.
fn finish(mut o: Opportunity, spec: &SourceSpec) -> Opportunity {
    o.tags.extend(spec.tags.iter().cloned());
    if spec.content == ContentKind::News {
        classify::enrich_news(&mut o, spec.stance);
        return o.sanitized();
    }
    classify::enrich(&mut o);
    if o.category == Category::Other
        && let Some(category) = spec.category
    {
        o.category = category;
    }
    o.sanitized()
}
