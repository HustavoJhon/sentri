//! Connector abstraction.
//!
//! Connectors form a closed set (an enum): adding one means adding a variant,
//! which keeps dispatch simple and avoids boxed async traits.

use chrono::{DateTime, Utc};
use sentri_core::{Category, ContentKind, Opportunity, SourceKind, Stance, classify};
use thiserror::Error;

use crate::SafeClient;
use crate::devpost::Devpost;
use crate::gdg::Gdg;
use crate::github::GitHub;
use crate::hack0::Hack0;
use crate::hackernews::HackerNews;
use crate::http::FetchError;
use crate::http::Validators;
use crate::ical::Calendar;
use crate::kaggle::Kaggle;
use crate::peruanos::Peruanos;
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

/// Result of an incremental fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incremental {
    /// `None` when the source confirmed nothing changed (HTTP 304).
    pub items: Option<Vec<Opportunity>>,
    /// Validators to send next time.
    pub validators: Validators,
}

/// A configured, ready-to-run connector.
#[derive(Debug, Clone)]
pub enum Connector {
    Feed(Feed),
    GitHub(GitHub),
    HackerNews(HackerNews),
    Calendar(Calendar),
    Gdg(Gdg),
    Devpost(Devpost),
    Kaggle(Kaggle),
    Peruanos(Peruanos),
    Hack0(Hack0),
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
            SourceKind::Devpost => Devpost::new(spec).map(Self::Devpost),
            SourceKind::Kaggle => Kaggle::new(spec).map(Self::Kaggle),
            SourceKind::Peruanos => Peruanos::new(spec).map(Self::Peruanos),
            SourceKind::Hack0 => Hack0::new(spec).map(Self::Hack0),
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
            Self::Devpost(c) => &c.spec,
            Self::Kaggle(c) => &c.spec,
            Self::Peruanos(c) => &c.spec,
            Self::Hack0(c) => &c.spec,
        }
    }

    /// Like [`fetch`](Self::fetch), but sends cache validators where the
    /// source supports them (RSS/Atom and iCal feeds).
    pub async fn fetch_incremental(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
        validators: &Validators,
    ) -> Result<Incremental, SourceError> {
        let fetched = match self {
            Self::Feed(c) => c.fetch_conditional(http, now, validators).await,
            Self::Calendar(c) => c.fetch_conditional(http, now, validators).await,
            Self::Peruanos(c) => {
                let (items, validators) = c.fetch_conditional(http, now, validators).await?;
                return Ok(Incremental {
                    items: Some(self.finish_all(items)),
                    validators,
                });
            }
            Self::Hack0(c) => {
                let (items, validators) = c.fetch_conditional(http, now, validators).await?;
                return Ok(Incremental {
                    items: Some(self.finish_all(items)),
                    validators,
                });
            }
            _ => {
                return Ok(Incremental {
                    items: Some(self.fetch(http, now).await?),
                    validators: Validators::default(),
                });
            }
        };
        match fetched {
            Ok((items, validators)) => Ok(Incremental {
                items: Some(self.finish_all(items)),
                validators,
            }),
            Err(SourceError::Fetch(FetchError::NotModified)) => Ok(Incremental {
                items: None,
                validators: validators.clone(),
            }),
            Err(e) => Err(e),
        }
    }

    fn finish_all(&self, items: Vec<Opportunity>) -> Vec<Opportunity> {
        let spec = self.spec();
        items
            .into_iter()
            .take(spec.max_items)
            .map(|o| finish(o, spec))
            .collect()
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
            Self::Devpost(c) => c.fetch(http, now).await?,
            Self::Kaggle(c) => c.fetch(http, now).await?,
            Self::Peruanos(c) => c.fetch(http, now).await?,
            Self::Hack0(c) => c.fetch(http, now).await?,
        };
        Ok(self.finish_all(items))
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
