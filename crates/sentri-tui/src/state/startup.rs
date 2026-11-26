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
pub const LINGER: Duration = Duration::from_millis(600);
/// An animated splash shows at least this long (any key still skips it at
/// once). The work behind it never waits: the dashboard loads meanwhile.
pub const MIN_ANIMATED: Duration = Duration::from_millis(1500);
/// Frame interval while animated (12.5 frames per second).
pub const FRAME: Duration = Duration::from_millis(80);
/// Timer interval for the static splash (timeouts only).
pub const STATIC_TIMER: Duration = Duration::from_millis(250);

/// Facts from the configuration shown beside the steps. Read once at
/// startup; nothing here is guessed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    /// "City, Country" when the profile has a location.
    pub location: Option<String>,
    /// Discovery scopes that are enabled, in order (local, national,
    /// international).
    pub scopes: Vec<&'static str>,
    /// Topics chosen during onboarding.
    pub topics: usize,
    /// The onboarding wizard opens after the splash.
    pub first_run: bool,
}

impl Context {
    pub fn from_config(profile: &sentri_core::Profile, discovery: &sentri_core::Discovery) -> Self {
        let clean = |v: Option<&str>| {
            v.map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let country = clean(profile.country.as_deref()).or_else(|| {
            clean(profile.country_code.as_deref()).map(|code| {
                sentri_core::country::names(&code)
                    .first()
                    .map_or_else(|| code.to_uppercase(), |name| (*name).to_owned())
            })
        });
        let parts: Vec<String> = [clean(profile.home_city.as_deref()), country]
            .into_iter()
            .flatten()
            .collect();
        let scopes = [
            (discovery.local, "local"),
            (discovery.national, "national"),
            (discovery.international, "international"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        Self {
            location: (!parts.is_empty()).then(|| parts.join(", ")),
            scopes,
            topics: discovery.topics.len(),
            first_run: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Startup {
    pub mode: SplashMode,
    pub unicode: bool,
    pub steps: Vec<(Step, Status)>,
    pub tick: u64,
    /// Current illumination; eases towards the eye's target each frame.
    pub glow: f32,
    pub context: Context,
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
            glow: if mode == SplashMode::Animated {
                0.15
            } else {
                1.0
            },
            context: Context::default(),
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
            let target = crate::observer::glow_target(self.eye());
            self.glow += (target - self.glow) * 0.18;
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
        let lingering = self
            .finished
            .is_none_or(|at| now.saturating_duration_since(at) < LINGER);
        let showing = self.mode == SplashMode::Animated
            && now.saturating_duration_since(self.started) < MIN_ANIMATED;
        !self.dismissed && (lingering || showing)
    }

    /// Whether frames advance on a timer.
    pub fn animated(&self) -> bool {
        self.mode == SplashMode::Animated
    }

    /// How often the event loop should wake up, while the splash shows.
    pub fn timer(&self, now: Instant) -> Option<Duration> {
        if !self.visible(now) {
            return None;
        }
        Some(self.interval())
    }

    /// Time between wake-ups: a frame while animated, otherwise just often
    /// enough to apply the grace periods.
    pub fn interval(&self) -> Duration {
        match self.mode {
            SplashMode::Animated => FRAME,
            _ => STATIC_TIMER,
        }
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
        let gone = t0 + LINGER.max(MIN_ANIMATED);
        let almost = t0
            + LINGER
                .max(MIN_ANIMATED)
                .saturating_sub(Duration::from_millis(1));
        assert!(s.visible(almost), "brief minimum");
        assert!(!s.visible(gone), "then gets out of the way");
        assert_eq!(s.timer(gone), None);
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
    fn a_static_splash_leaves_as_soon_as_work_is_done() {
        let (mut s, t0) = startup(SplashMode::Static);
        for step in Step::ALL {
            s.resolve(step, Status::Done("ok".into()), t0);
        }
        assert!(!s.visible(t0 + LINGER), "no minimum without animation");
    }

    #[test]
    fn light_eases_up_with_progress() {
        let (mut s, t0) = startup(SplashMode::Animated);
        let dark = s.glow;
        s.on_timer(t0);
        assert!(s.glow > dark, "it warms up");
        assert!(s.glow < crate::observer::glow_target(Eye::Closed));
        for step in Step::ALL {
            s.resolve(step, Status::Done("ok".into()), t0);
        }
        for _ in 0..40 {
            s.on_timer(t0);
        }
        assert!((s.glow - 1.0).abs() < 0.01, "fully lit: {}", s.glow);
        let (still, _) = startup(SplashMode::Static);
        assert!((still.glow - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn context_reports_the_configured_location_and_scopes() {
        use sentri_core::{Discovery, Profile};
        let profile = Profile {
            home_city: Some("Cusco".into()),
            country_code: Some("pe".into()),
            ..Profile::default()
        };
        let discovery = Discovery {
            international: false,
            ..Discovery::default()
        };
        let c = Context::from_config(&profile, &discovery);
        assert_eq!(c.location.as_deref(), Some("Cusco, Peru"));
        assert_eq!(c.scopes, ["local", "national"]);
        let empty = Context::from_config(&Profile::default(), &Discovery::default());
        assert_eq!(empty.location, None, "nothing is invented");
        assert_eq!(empty.scopes.len(), 3);
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
