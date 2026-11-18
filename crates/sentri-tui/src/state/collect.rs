//! Collection progress and the start-up refresh.
//!
//! When the interface opens with an empty database (or data older than the
//! daemon interval), SENTRI collects in the background so the first screen
//! fills in as sources answer instead of staying empty.

use std::fmt::Write as _;

use chrono::{DateTime, Duration, Utc};
use sentri_app::CollectReport;

use super::Overview;

/// Progress of a running collection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectProgress {
    /// Sources that answered (or failed) so far.
    pub done: usize,
    /// Sources that will run; `0` until the runtime reports it.
    pub total: usize,
    /// New items stored so far.
    pub new: i64,
    pub failed: Vec<String>,
    /// Started automatically because the database was empty.
    pub first_run: bool,
}

impl CollectProgress {
    pub fn started(first_run: bool) -> Self {
        Self {
            first_run,
            ..Self::default()
        }
    }

    /// Records one source; returns whether it brought new items.
    pub fn record(&mut self, report: &CollectReport) -> bool {
        self.done += 1;
        if let Ok((_, new)) = &report.result {
            self.new += new;
            *new > 0
        } else {
            self.failed.push(report.source.clone());
            false
        }
    }

    /// Completed fraction, `0.0..=1.0`.
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let ratio = self.done as f64 / self.total as f64;
        ratio.clamp(0.0, 1.0)
    }

    /// Short status such as `collecting 5/22 · +34`.
    pub fn label(&self) -> String {
        let mut text = if self.total > 0 {
            format!("collecting {}/{}", self.done.min(self.total), self.total)
        } else {
            "collecting".to_owned()
        };
        if self.new > 0 {
            let _ = write!(text, " · +{}", self.new);
        }
        text
    }
}

/// Whether the start-up collection was already decided.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StartCheck {
    #[default]
    Pending,
    Done,
}

/// Why the interface should collect when it opens, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartReason {
    /// Nothing stored yet.
    Empty,
    /// No source succeeded within the daemon interval.
    Stale,
    /// `ui.collect_on_start = "always"`.
    Always,
}

/// Decides the start-up collection from `ui.collect_on_start`.
pub fn on_start(overview: &Overview, now: DateTime<Utc>) -> Option<StartReason> {
    let config = &overview.config;
    if !config.sources.iter().any(|s| s.enabled) {
        return None;
    }
    match config.ui.collect_on_start.as_str() {
        "never" => None,
        "always" => Some(StartReason::Always),
        _ => {
            if overview.stats.total == 0 {
                return Some(StartReason::Empty);
            }
            let minutes = i64::try_from(config.daemon.interval_minutes).unwrap_or(i64::MAX);
            let fresh_after = now - Duration::minutes(minutes);
            let fresh = overview
                .states
                .iter()
                .filter_map(|s| s.last_success_at)
                .any(|t| t >= fresh_after);
            (!fresh).then_some(StartReason::Stale)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overview(total: i64, last_success: Option<DateTime<Utc>>, mode: &str) -> Overview {
        let mut config = sentri_app::Config::parse(
            sentri_app::config::DEFAULT_CONFIG,
            std::path::Path::new("d"),
        )
        .unwrap();
        config.ui.collect_on_start = mode.into();
        Overview {
            config,
            stats: sentri_app::Stats {
                total,
                ..sentri_app::Stats::default()
            },
            states: vec![sentri_app::SourceState {
                source_name: "x".into(),
                last_success_at: last_success,
                ..sentri_app::SourceState::default()
            }],
            ..Overview::default()
        }
    }

    #[test]
    fn collects_when_empty_or_stale() {
        let now = Utc::now();
        assert_eq!(
            on_start(&overview(0, None, "auto"), now),
            Some(StartReason::Empty)
        );
        let old = now - Duration::hours(10);
        assert_eq!(
            on_start(&overview(40, Some(old), "auto"), now),
            Some(StartReason::Stale)
        );
        let recent = now - Duration::minutes(5);
        assert_eq!(on_start(&overview(40, Some(recent), "auto"), now), None);
        assert_eq!(
            on_start(&overview(40, Some(recent), "always"), now),
            Some(StartReason::Always)
        );
        assert_eq!(on_start(&overview(0, None, "never"), now), None);
    }

    #[test]
    fn nothing_to_do_without_enabled_sources() {
        let mut o = overview(0, None, "always");
        for s in &mut o.config.sources {
            s.enabled = false;
        }
        assert_eq!(on_start(&o, Utc::now()), None);
    }

    #[test]
    fn tracks_progress() {
        let mut p = CollectProgress::started(true);
        p.total = 4;
        let ok = CollectReport {
            source: "a".into(),
            result: Ok((10, 3)),
            note: None,
        };
        assert!(p.record(&ok));
        let failed = CollectReport {
            source: "b".into(),
            result: Err("500".into()),
            note: None,
        };
        assert!(!p.record(&failed));
        assert_eq!(p.label(), "collecting 2/4 · +3");
        assert!((p.ratio() - 0.5).abs() < f64::EPSILON);
        assert_eq!(p.failed, ["b"]);
    }

    #[test]
    fn the_first_overview_starts_a_collection_that_fills_the_feed() {
        use crate::state::{Effect, Msg, State};
        let mut state = State::default();
        let effects = state.on_msg(Msg::Overview(Box::new(overview(0, None, "auto"))));
        assert_eq!(effects, vec![Effect::Collect]);
        assert!(state.collecting.as_ref().unwrap().first_run);
        // Later overviews never start another one.
        assert_eq!(
            state.on_msg(Msg::Overview(Box::new(overview(0, None, "auto")))),
            vec![]
        );
        state.on_msg(Msg::CollectStarted(3));
        let report = |new| CollectReport {
            source: "s".into(),
            result: Ok((5, new)),
            note: None,
        };
        assert_eq!(state.on_msg(Msg::SourceCollected(report(0))), vec![]);
        assert_eq!(
            state.on_msg(Msg::SourceCollected(report(4))),
            vec![Effect::Load],
            "new items appear right away"
        );
        assert_eq!(
            state.collecting.as_ref().unwrap().label(),
            "collecting 2/3 · +4"
        );
        state.on_msg(Msg::Collected("done".into()));
        assert!(state.collecting.is_none());
    }
}
