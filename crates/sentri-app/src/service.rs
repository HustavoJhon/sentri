//! [`Sentri`]: the facade every interface uses.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sentri_core::{
    Action, Affinity, CanonicalUrl, FeedQuery, Flag, InteractionState, Opportunity, OpportunityId,
};
use sentri_sources::{FetchError, HttpLimits, SafeClient};
use sentri_storage::{HistoryEntry, Stats, StorageError, Store};

use crate::links::{self, AddLinkError, AddOutcome, LinkOverrides};
use crate::{Config, Ranked, rank};

/// Errors surfaced by use cases.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Http(#[from] FetchError),
    #[error(transparent)]
    AddLink(#[from] AddLinkError),
}

/// Application context: configuration, storage and network access.
#[derive(Debug, Clone)]
pub struct Sentri {
    config: Config,
    store: Store,
    http: SafeClient,
}

impl Sentri {
    /// Opens the database configured in `config`.
    pub async fn open(config: Config) -> Result<Self, AppError> {
        let store = Store::open(&config.database_path()).await?;
        Self::with_store(config, store, false)
    }

    /// Builds a context around an existing store. `allow_private_networks`
    /// exists only for tests against local mock servers.
    pub fn with_store(
        config: Config,
        store: Store,
        allow_private_networks: bool,
    ) -> Result<Self, AppError> {
        let limits = HttpLimits {
            timeout: Duration::from_secs(config.http.timeout_secs),
            max_body_bytes: config.http.max_body_bytes,
            max_redirects: config.http.max_redirects,
            allow_private_networks,
        };
        Ok(Self {
            http: SafeClient::new(limits)?,
            config,
            store,
        })
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub const fn store(&self) -> &Store {
        &self.store
    }

    pub const fn http(&self) -> &SafeClient {
        &self.http
    }

    /// Adds a URL found while browsing. Duplicates are detected before any
    /// network access.
    pub async fn add_link(
        &self,
        input: &str,
        overrides: &LinkOverrides,
    ) -> Result<AddOutcome, AddLinkError> {
        let url = CanonicalUrl::parse(input)?;
        if let Some(existing) = self.store.find_by_fingerprint(url.fingerprint()).await? {
            return Ok(AddOutcome::Duplicate(existing));
        }
        let (opportunity, warnings) =
            links::build(&self.http, &url, input.trim(), overrides, Utc::now()).await;
        let id = self.store.upsert_opportunity(&opportunity).await?.id();
        let item = self.store.get(id).await?;
        Ok(AddOutcome::Added { item, warnings })
    }

    /// Ranked feed for `query`.
    pub async fn feed(&self, query: &FeedQuery) -> Result<Vec<Ranked>, AppError> {
        let now = Utc::now();
        let affinity = self.affinity().await?;
        let mut storage_query = query.clone();
        // Relevance is computed here, so storage must return every candidate.
        if query.sort == sentri_core::SortOrder::Relevance {
            storage_query.limit = None;
        }
        let items = self.store.list(&storage_query, now).await?;
        Ok(rank(
            items,
            query,
            &self.config.profile,
            &self.config.scoring,
            &affinity,
            now,
        ))
    }

    /// One item with its relevance explanation.
    pub async fn explain(&self, id: OpportunityId) -> Result<Ranked, AppError> {
        let item = self.store.get(id).await?;
        let affinity = self.affinity().await?;
        let mut ranked = rank(
            vec![item],
            &FeedQuery::default(),
            &self.config.profile,
            &self.config.scoring,
            &affinity,
            Utc::now(),
        );
        Ok(ranked.remove(0))
    }

    async fn affinity(&self) -> Result<Affinity, AppError> {
        let interacted = self.store.interacted().await?;
        Ok(Affinity::learn(
            interacted.iter().map(|i| (&i.opportunity, &i.state)),
        ))
    }

    pub async fn act(
        &self,
        id: OpportunityId,
        action: Action,
    ) -> Result<InteractionState, AppError> {
        Ok(self.store.apply_action(id, action, Utc::now()).await?)
    }

    pub async fn toggle(
        &self,
        id: OpportunityId,
        flag: Flag,
    ) -> Result<InteractionState, AppError> {
        Ok(self.store.toggle_flag(id, flag, Utc::now()).await?)
    }

    /// Saves manual corrections to an opportunity.
    pub async fn edit(&self, id: OpportunityId, edited: Opportunity) -> Result<(), AppError> {
        Ok(self
            .store
            .update_opportunity(id, &edited.sanitized())
            .await?)
    }

    /// Applies user corrections to an existing opportunity and returns it.
    pub async fn correct(
        &self,
        id: OpportunityId,
        overrides: &LinkOverrides,
    ) -> Result<sentri_core::FeedItem, AppError> {
        let mut opportunity = self.store.get(id).await?.opportunity;
        links::apply_overrides(&mut opportunity, overrides);
        self.edit(id, opportunity).await?;
        Ok(self.store.get(id).await?)
    }

    pub async fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>, AppError> {
        Ok(self.store.history(limit).await?)
    }

    /// Dashboard counters; "new" means discovered in the last 24 hours.
    pub async fn stats(&self) -> Result<Stats, AppError> {
        let now: DateTime<Utc> = Utc::now();
        Ok(self
            .store
            .stats(now - chrono::Duration::hours(24), now)
            .await?)
    }
}
