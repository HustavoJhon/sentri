//! [`Sentri`]: the facade every interface uses.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sentri_core::{
    Action, Affinity, Annotations, CanonicalUrl, FeedQuery, Field, Flag, InteractionState,
    OpportunityId,
};
use sentri_sources::{FetchError, HttpLimits, SafeClient};
use sentri_storage::{
    DayActivity, DayEntry, EditState, HistoryEntry, Insights, Sighting, Stats, StorageError, Store,
};

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
    #[error(transparent)]
    Image(#[from] crate::ImageError),
    #[error("GitHub profile: {0}")]
    Profile(String),
}

/// Application context: configuration, storage and network access.
#[derive(Debug, Clone)]
pub struct Sentri {
    /// Shared and replaceable, so saved settings and new sources apply
    /// without restarting.
    config: std::sync::Arc<std::sync::RwLock<std::sync::Arc<Config>>>,
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
            config: std::sync::Arc::new(std::sync::RwLock::new(std::sync::Arc::new(config))),
            store,
        })
    }

    /// Snapshot of the current configuration.
    pub fn config(&self) -> std::sync::Arc<Config> {
        self.config
            .read()
            .map_or_else(|poisoned| poisoned.into_inner().clone(), |c| c.clone())
    }

    /// Replaces the configuration (e.g. after saving settings or sources).
    /// Every clone of this `Sentri` sees the new values.
    pub fn reload_config(&self, config: Config) {
        let mut slot = self
            .config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = std::sync::Arc::new(config);
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
        // What the user typed is manual data: it overrides, never replaces,
        // what the page said.
        let values = overrides.values(&opportunity);
        if !values.is_empty() {
            self.store.set_overrides(id, &values, Utc::now()).await?;
        }
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
        let config = self.config();
        Ok(rank(
            items,
            query,
            &config.profile,
            &config.scoring,
            &affinity,
            now,
        ))
    }

    /// How many items each view holds right now (no search or filters), for
    /// the counters next to each section.
    pub async fn view_counts(
        &self,
        views: &[sentri_core::FeedView],
    ) -> Result<Vec<(sentri_core::FeedView, usize)>, AppError> {
        let now = Utc::now();
        let mut counts = Vec::with_capacity(views.len());
        for view in views {
            let query = FeedQuery {
                sort: sentri_core::SortOrder::Newest,
                ..FeedQuery::view(*view)
            };
            counts.push((*view, self.store.list(&query, now).await?.len()));
        }
        Ok(counts)
    }

    /// One item with its relevance explanation.
    pub async fn explain(&self, id: OpportunityId) -> Result<Ranked, AppError> {
        let item = self.store.get(id).await?;
        let affinity = self.affinity().await?;
        let config = self.config();
        let mut ranked = rank(
            vec![item],
            &FeedQuery::default(),
            &config.profile,
            &config.scoring,
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
    /// Applies user corrections to an existing opportunity and returns it.
    pub async fn correct(
        &self,
        id: OpportunityId,
        overrides: &LinkOverrides,
    ) -> Result<sentri_core::FeedItem, AppError> {
        let current = self.store.get(id).await?.opportunity;
        self.store
            .set_overrides(id, &overrides.values(&current), Utc::now())
            .await?;
        Ok(self.store.get(id).await?)
    }

    /// Detected values, manual overrides and personal notes of an item.
    pub async fn edit_state(&self, id: OpportunityId) -> Result<EditState, AppError> {
        Ok(self.store.edit_state(id).await?)
    }

    /// Every source that published an item (for cross-source attribution).
    pub async fn sightings(&self, id: OpportunityId) -> Result<Vec<Sighting>, AppError> {
        Ok(self.store.sightings(id).await?)
    }

    /// Saves an editor session atomically per layer: manual values, fields
    /// reset to their detected value, and annotations.
    pub async fn save_edit(
        &self,
        id: OpportunityId,
        values: &[(Field, String)],
        reset: &[Field],
        annotations: Option<&Annotations>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        if !reset.is_empty() {
            self.store.reset_overrides(id, reset).await?;
        }
        if !values.is_empty() {
            self.store.set_overrides(id, values, now).await?;
        }
        if let Some(annotations) = annotations {
            self.store.set_annotations(id, annotations, now).await?;
        }
        Ok(())
    }

    /// Dashboard aggregates over the last `days` days (local time).
    pub async fn insights(&self, days: i64) -> Result<Insights, AppError> {
        Ok(self
            .store
            .insights(Utc::now(), days, local_offset_secs())
            .await?)
    }

    /// Deduplicated activity per local day since `days` ago.
    pub async fn activity(
        &self,
        days: i64,
    ) -> Result<std::collections::BTreeMap<chrono::NaiveDate, DayActivity>, AppError> {
        let since = Utc::now() - chrono::Duration::days(days);
        Ok(self.store.activity(since, local_offset_secs()).await?)
    }

    /// What happened on one local day.
    pub async fn day_entries(&self, day: chrono::NaiveDate) -> Result<Vec<DayEntry>, AppError> {
        Ok(self.store.day_entries(day, local_offset_secs()).await?)
    }

    /// Recent actions on one opportunity.
    pub async fn history_for(
        &self,
        id: OpportunityId,
        limit: u32,
    ) -> Result<Vec<HistoryEntry>, AppError> {
        Ok(self.store.history_for(id, limit).await?)
    }

    /// Re-applies the current news stance rules to stored news. Dry run
    /// unless `apply`. Returns the batch id (for undo) and the changes.
    pub async fn reclassify_news(
        &self,
        apply: bool,
    ) -> Result<(String, Vec<sentri_storage::StanceChange>), AppError> {
        let batch = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let config = self.config();
        let sources = &config.sources;
        let fallback = |name: &str| {
            sources
                .iter()
                .find(|s| s.name == name)
                .and_then(|s| s.stance)
        };
        let changes = self
            .store
            .reclassify_news(&batch, fallback, apply, Utc::now())
            .await?;
        Ok((batch, changes))
    }

    /// Undoes a reclassification batch.
    pub async fn undo_reclassification(&self, batch: &str) -> Result<u64, AppError> {
        Ok(self.store.undo_reclassification(batch).await?)
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

/// Offset of the local time zone, so "today" matches the user's clock.
pub fn local_offset_secs() -> i64 {
    i64::from(chrono::Local::now().offset().local_minus_utc())
}
