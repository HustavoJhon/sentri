//! "Collect" use case: run every enabled connector independently.

use std::time::Duration;

use chrono::Utc;
use sentri_core::SourceKind;
use sentri_sources::{Connector, SourceSpec};

use crate::{Sentri, SourceConfig};

/// Outcome of one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectReport {
    pub source: String,
    /// `(items seen, items new)` or the error message.
    pub result: Result<(i64, i64), String>,
    /// Extra information: "not modified", "backing off until …".
    pub note: Option<String>,
}

/// Builds the connector spec, reading the token from the environment.
fn spec(config: &SourceConfig) -> SourceSpec {
    let token = config.token_env.as_deref().and_then(|var| {
        let value = std::env::var(var).ok().filter(|v| !v.trim().is_empty());
        if value.is_none() {
            tracing::debug!(source = %config.name, var, "token variable not set");
        }
        value
    });
    SourceSpec {
        name: config.name.clone(),
        kind: config.kind,
        url: config.url.clone(),
        query: config.query.clone(),
        target: config.target.clone(),
        category: config.category,
        tags: config.tags.clone(),
        max_items: config.max_items,
        token,
        content: config.content,
        stance: config.stance,
    }
}

/// Converts a form draft into a source configuration.
pub fn draft_config(d: &crate::settings::SourceDraft) -> Result<SourceConfig, String> {
    Ok(SourceConfig {
        name: d.name.trim().to_owned(),
        kind: d
            .kind
            .parse()
            .map_err(|e: sentri_core::ParseError| e.to_string())?,
        enabled: d.enabled,
        url: Some(d.url.trim().to_owned()),
        query: None,
        target: None,
        category: if d.category.trim().is_empty() {
            None
        } else {
            Some(
                d.category
                    .parse()
                    .map_err(|e: sentri_core::ParseError| e.to_string())?,
            )
        },
        tags: Vec::new(),
        max_items: 50,
        token_env: None,
        content: if d.content == "news" {
            sentri_core::ContentKind::News
        } else {
            sentri_core::ContentKind::Opportunity
        },
        stance: None,
    })
}

impl Sentri {
    /// Fetches and parses a source without storing anything, to validate a
    /// URL and its format before saving it. Returns how many items it has.
    pub async fn test_source(&self, source: &SourceConfig) -> Result<usize, String> {
        let url = source.url.as_deref().unwrap_or_default().trim();
        // Peruanos.dev and hack0.dev have public defaults; a URL is optional.
        if !url.is_empty() || !matches!(source.kind, SourceKind::Peruanos | SourceKind::Hack0) {
            let normalized = url
                .strip_prefix("webcal://")
                .map_or_else(|| url.to_owned(), |r| format!("https://{r}"));
            sentri_core::CanonicalUrl::parse(&normalized).map_err(|e| e.to_string())?;
        }
        let connector = Connector::from_spec(spec(source)).map_err(|e| e.to_string())?;
        let budget = Duration::from_secs(self.config().http.timeout_secs.saturating_mul(2));
        let items = tokio::time::timeout(budget, connector.fetch(self.http(), Utc::now()))
            .await
            .map_err(|_| format!("timed out after {}s", budget.as_secs()))?
            .map_err(|e| e.to_string())?;
        Ok(items.len())
    }

    /// Runs enabled sources (or only `only`), one after another to stay polite
    /// with rate limits. A failing source is recorded and skipped; sources
    /// that failed repeatedly are skipped until their back-off expires,
    /// unless requested explicitly with `only`.
    pub async fn collect(&self, only: Option<&str>) -> Vec<CollectReport> {
        let config = self.config();
        let selected: Vec<&SourceConfig> = config
            .sources
            .iter()
            .filter(|s| only.map_or(s.enabled, |name| s.name == name))
            .collect();
        let mut reports = Vec::with_capacity(selected.len());
        let now = Utc::now();
        for source in selected {
            let state = self
                .store()
                .source_state(&source.name)
                .await
                .unwrap_or_default();
            if only.is_none() && state.backing_off(now) {
                let until = state
                    .next_attempt_at
                    .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
                    .unwrap_or_default();
                reports.push(CollectReport {
                    source: source.name.clone(),
                    result: Ok((0, 0)),
                    note: Some(format!(
                        "skipped after {} failures; retry after {until}",
                        state.consecutive_failures
                    )),
                });
                continue;
            }
            let (result, note) = match self.collect_one(source, &state).await {
                Ok((counts, note)) => (Ok(counts), note),
                Err(e) => (Err(e), None),
            };
            match &result {
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(source = %source.name, %error, "collection failed");
                    let _ = self
                        .store()
                        .record_source_failure(&source.name, error, Utc::now())
                        .await;
                }
            }
            reports.push(CollectReport {
                source: source.name.clone(),
                result,
                note,
            });
        }
        reports
    }

    async fn collect_one(
        &self,
        source: &SourceConfig,
        state: &sentri_storage::SourceState,
    ) -> Result<((i64, i64), Option<String>), String> {
        let store = self.store();
        let run = store
            .start_run(&source.name, Utc::now())
            .await
            .map_err(|e| e.to_string())?;
        let result = self.fetch_and_store(source, state).await;
        let finish = match &result {
            Ok((counts, _)) => Ok(*counts),
            Err(e) => Err(e.as_str()),
        };
        store
            .finish_run(run, Utc::now(), finish)
            .await
            .map_err(|e| e.to_string())?;
        result
    }

    async fn fetch_and_store(
        &self,
        source: &SourceConfig,
        state: &sentri_storage::SourceState,
    ) -> Result<((i64, i64), Option<String>), String> {
        let connector = Connector::from_spec(spec(source)).map_err(|e| e.to_string())?;
        // Generous bound on top of the per-request timeout, so a stuck source
        // can never block the others.
        let budget = Duration::from_secs(self.config().http.timeout_secs.saturating_mul(3));
        let validators = sentri_sources::Validators {
            etag: state.etag.clone(),
            last_modified: state.last_modified.clone(),
        };
        let fetched = tokio::time::timeout(
            budget,
            connector.fetch_incremental(self.http(), Utc::now(), &validators),
        )
        .await
        .map_err(|_| format!("timed out after {}s", budget.as_secs()))?
        .map_err(|e| e.to_string())?;
        let v = &fetched.validators;
        self.store()
            .record_source_success(
                &source.name,
                v.etag.as_deref(),
                v.last_modified.as_deref(),
                Utc::now(),
            )
            .await
            .map_err(|e| e.to_string())?;
        let Some(items) = fetched.items else {
            return Ok(((0, 0), Some("not modified".into())));
        };
        let mut new = 0;
        for item in &items {
            let outcome = self
                .store()
                .upsert_opportunity(item)
                .await
                .map_err(|e| e.to_string())?;
            if outcome.is_new() {
                new += 1;
            }
        }
        Ok(((i64::try_from(items.len()).unwrap_or(i64::MAX), new), None))
    }
}
