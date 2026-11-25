//! The Observer awakening: startup steps and their real outcome.
//!
//! Each step is marked only when the work behind it has finished (or was
//! handed to the background), so the screen never reports progress that did
//! not happen. Slow, optional work (GitHub, sources) never holds the
//! dashboard: after a short grace period it continues in the background.

use std::time::{Duration, Instant};

use crate::observer::{Eye, SplashMode};

/// Initialization steps, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Observer,
    Environment,
    Sources,
    Profile,
    Dashboard,
}

impl Step {
    pub const ALL: [Self; 5] = [
        Self::Observer,
        Self::Environment,
        Self::Sources,
        Self::Profile,
        Self::Dashboard,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Observer => "Initializing Observer",
            Self::Environment => "Detecting environment",
            Self::Sources => "Connecting opportunity sources",
            Self::Profile => "Loading personal profile",
            Self::Dashboard => "Preparing dashboard",
        }
    }
}

/// What happened to a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Done(String),
    /// Still running, but the dashboard does not wait for it.
    Background(String),
    Failed(String),
}

impl Status {
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Optional profile data (GitHub) gets this long before moving on.
pub const PROFILE_GRACE: Duration = Duration::from_millis(1500);
/// Nothing keeps the splash on screen longer than this.
pub const MAX_WAIT: Duration = Duration::from_secs(4);
/// The open eye stays visible this long once everything is ready.
pub const LINGER: Duration = Duration::from_millis(450);
/// Frame interval while animated.
pub const FRAME: Duration = Duration::from_millis(90);
/// Timer interval for the static splash (timeouts only).
pub const STATIC_TIMER: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct Startup {
    pub mode: SplashMode,
    pub unicode: bool,
    pub steps: Vec<(Step, Status)>,
    pub tick: u64,
    started: Instant,
    finished: Option<Instant>,
    dismissed: bool,
}

impl Startup {
    pub fn new(mode: SplashMode, unicode: bool, now: Instant) -> Self {
        Self {
            mode,
            unicode,
            steps: Step::ALL.iter().map(|s| (*s, Status::Pending)).collect(),
            tick: 0,
            started: now,
            finished: None,
            dismissed: mode == SplashMode::Off,
        }
    }

    pub fn status(&self, step: Step) -> &Status {
        self.steps
            .iter()
            .find(|(s, _)| *s == step)
            .map_or(&Status::Pending, |(_, status)| status)
    }

    /// Records the outcome of `step`; later reports do not overwrite one.
    pub fn resolve(&mut self, step: Step, status: Status, now: Instant) {
        if let Some((_, slot)) = self.steps.iter_mut().find(|(s, _)| *s == step)
            && slot.is_pending()
        {
            *slot = status;
        }
        if self.finished.is_none() && self.steps.iter().all(|(_, s)| !s.is_pending()) {
            self.finished = Some(now);
        }
    }

    pub fn resolved(&self) -> usize {
        self.steps.iter().filter(|(_, s)| !s.is_pending()).count()
    }

    pub fn eye(&self) -> Eye {
        Eye::from_progress(self.resolved(), self.steps.len())
    }

    /// Advances the animation and applies the grace periods.
    pub fn on_timer(&mut self, now: Instant) {
        if self.mode == SplashMode::Animated {
            self.tick = self.tick.wrapping_add(1);
        }
        let waited = now.saturating_duration_since(self.started);
        if waited >= PROFILE_GRACE && self.status(Step::Profile).is_pending() {
            self.resolve(
                Step::Profile,
                Status::Background("GitHub data keeps loading in the background".into()),
                now,
            );
        }
        if waited >= MAX_WAIT {
            for step in Step::ALL {
                if self.status(step).is_pending() {
                    self.resolve(
                        step,
                        Status::Background("continuing in the background".into()),
                        now,
                    );
                }
            }
        }
    }

    /// Any key skips the rest of the splash (the work goes on).
    pub const fn dismiss(&mut self) {
        self.dismissed = true;
    }

    pub fn visible(&self, now: Instant) -> bool {
        !self.dismissed
            && self
                .finished
                .is_none_or(|at| now.saturating_duration_since(at) < LINGER)
    }

    /// How often the event loop should wake up, while the splash shows.
    pub fn timer(&self, now: Instant) -> Option<Duration> {
        if !self.visible(now) {
            return None;
        }
        Some(match self.mode {
            SplashMode::Animated => FRAME,
            _ => STATIC_TIMER,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn startup(mode: SplashMode) -> (Startup, Instant) {
        let now = Instant::now();
        (Startup::new(mode, true, now), now)
    }

    #[test]
    fn opens_only_as_steps_really_finish() {
        let (mut s, t0) = startup(SplashMode::Animated);
        assert_eq!(s.eye(), Eye::Closed);
        s.resolve(Step::Observer, Status::Done("ready".into()), t0);
        s.resolve(Step::Environment, Status::Done("kitty".into()), t0);
        assert_eq!(s.eye(), Eye::Stirring);
        s.resolve(Step::Sources, Status::Done("3 sources".into()), t0);
        s.resolve(Step::Profile, Status::Done("@octocat".into()), t0);
        assert_eq!(s.eye(), Eye::Half);
        assert!(s.visible(t0));
        s.resolve(Step::Dashboard, Status::Done("12 ranked".into()), t0);
        assert_eq!(s.eye(), Eye::Open);
        assert!(
            s.visible(t0 + Duration::from_millis(100)),
            "lingers briefly"
        );
        assert!(!s.visible(t0 + LINGER), "then gets out of the way");
        assert_eq!(s.timer(t0 + LINGER), None);
    }

    #[test]
    fn first_outcome_wins() {
        let (mut s, t0) = startup(SplashMode::Static);
        s.resolve(Step::Dashboard, Status::Failed("db".into()), t0);
        s.resolve(Step::Dashboard, Status::Done("late".into()), t0);
        assert_eq!(s.status(Step::Dashboard), &Status::Failed("db".into()));
    }

    #[test]
    fn slow_optional_work_moves_to_the_background() {
        let (mut s, t0) = startup(SplashMode::Animated);
        s.on_timer(t0 + Duration::from_millis(200));
        assert!(s.status(Step::Profile).is_pending());
        s.on_timer(t0 + PROFILE_GRACE);
        assert!(matches!(s.status(Step::Profile), Status::Background(_)));
        assert!(s.status(Step::Sources).is_pending());
        s.on_timer(t0 + MAX_WAIT);
        assert_eq!(s.resolved(), Step::ALL.len());
        assert!(!s.visible(t0 + MAX_WAIT + LINGER), "never blocks startup");
    }

    #[test]
    fn static_and_off_modes() {
        let (mut s, t0) = startup(SplashMode::Static);
        s.on_timer(t0 + Duration::from_millis(300));
        assert_eq!(s.tick, 0, "a static splash does not animate");
        assert_eq!(s.timer(t0), Some(STATIC_TIMER));
        let (off, t0) = startup(SplashMode::Off);
        assert!(!off.visible(t0));
        assert_eq!(off.timer(t0), None);
        let (mut skipped, t0) = startup(SplashMode::Animated);
        skipped.dismiss();
        assert!(!skipped.visible(t0));
    }

    #[test]
    fn messages_mark_steps_and_any_key_skips() {
        use crate::state::{Msg, ProfileState, State};
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        let mut state = State {
            startup: Some(Startup::new(SplashMode::Animated, true, Instant::now())),
            ..State::default()
        };
        assert!(state.splash_visible());
        state.on_msg(Msg::Loaded(Vec::new()));
        state.on_msg(Msg::Profile(ProfileState::NotConfigured));
        let s = state.startup.as_ref().unwrap();
        assert_eq!(
            s.status(Step::Dashboard),
            &Status::Done("0 opportunities ranked".into())
        );
        assert!(matches!(s.status(Step::Profile), Status::Done(_)));
        assert!(s.status(Step::Sources).is_pending());
        let effects = state.on_key(KeyEvent::from(KeyCode::Char('j')));
        assert!(effects.is_empty(), "the key only skips the splash");
        assert!(!state.splash_visible());
        assert!(!state.quit);
    }
}
