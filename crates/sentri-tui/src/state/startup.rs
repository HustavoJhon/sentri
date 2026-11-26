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
    Configuration,
    Profile,
    Sources,
    Dashboard,
}

impl Step {
    pub const ALL: [Self; 6] = [
        Self::Observer,
        Self::Environment,
        Self::Configuration,
        Self::Profile,
        Self::Sources,
        Self::Dashboard,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Observer => "Initializing Observer",
            Self::Environment => "Detecting terminal",
            Self::Configuration => "Loading configuration",
            Self::Profile => "Loading user profile",
            Self::Sources => "Preparing sources",
            Self::Dashboard => "Preparing dashboard",
        }
    }

    /// Short name used in the event log.
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Observer => "observer",
            Self::Environment => "terminal",
            Self::Configuration => "config",
            Self::Profile => "profile",
            Self::Sources => "sources",
            Self::Dashboard => "dashboard",
        }
    }
}

/// How an event-log entry is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Done,
    Background,
    Failed,
}

/// One line of the event log: something that really happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Time since the splash appeared.
    pub at: Duration,
    pub level: Level,
    pub tag: &'static str,
    pub text: String,
}

/// How long the awakening takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pace {
    /// A paced awakening of about seven seconds.
    #[default]
    Cinematic,
    /// About two seconds.
    Brief,
}

impl Pace {
    pub fn from_name(name: &str) -> Self {
        if name == "brief" {
            Self::Brief
        } else {
            Self::Cinematic
        }
    }

    /// The animated splash shows at least this long (any key skips it).
    pub const fn minimum(self) -> Duration {
        match self {
            Self::Cinematic => Duration::from_millis(7000),
            Self::Brief => Duration::from_millis(1500),
        }
    }

    /// Frames between two stages of the eye opening.
    const fn eye_stage(self) -> u64 {
        match self {
            Self::Cinematic => 14,
            Self::Brief => 4,
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
/// Most event-log entries kept.
const LOG_LIMIT: usize = 64;
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
    /// The eye as drawn: it opens one stage at a time and never ahead of
    /// real progress, so even an instant startup shows the awakening.
    shown: Eye,
    pub context: Context,
    pub pace: Pace,
    /// What happened, in order.
    pub events: Vec<Event>,
    /// Time since the splash appeared, as of the last timer (so a frame is
    /// a pure function of the state).
    pub clock: Duration,
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
            shown: Eye::Closed,
            context: Context::default(),
            pace: Pace::default(),
            events: Vec::new(),
            clock: Duration::ZERO,
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

    /// Records the outcome of `step`. A final outcome (done or failed) is
    /// never overwritten; work that went to the background may still report
    /// how it ended.
    pub fn resolve(&mut self, step: Step, status: Status, now: Instant) {
        let Some((_, slot)) = self.steps.iter_mut().find(|(s, _)| *s == step) else {
            return;
        };
        let open = slot.is_pending()
            || matches!(slot, Status::Background(_)) && !matches!(status, Status::Background(_));
        if !open {
            return;
        }
        let (level, detail) = match &status {
            Status::Pending => return,
            Status::Done(d) => (Level::Done, d.clone()),
            Status::Background(d) => (Level::Background, d.clone()),
            Status::Failed(d) => (Level::Failed, d.clone()),
        };
        *slot = status;
        self.log(now, level, step.tag(), detail);
        if self.finished.is_none() && self.steps.iter().all(|(_, s)| !s.is_pending()) {
            self.finished = Some(now);
            self.log(now, Level::Info, "observer", "all systems ready".into());
        }
    }

    /// Adds a fact to the event log.
    pub fn log(&mut self, now: Instant, level: Level, tag: &'static str, text: String) {
        if self.events.len() >= LOG_LIMIT {
            self.events.remove(0);
        }
        self.events.push(Event {
            at: now.saturating_duration_since(self.started),
            level,
            tag,
            text,
        });
    }

    pub fn resolved(&self) -> usize {
        self.steps.iter().filter(|(_, s)| !s.is_pending()).count()
    }

    pub fn eye(&self) -> Eye {
        Eye::from_progress(self.resolved(), self.steps.len())
    }

    /// The eye to draw: the real one without animation, otherwise the
    /// stage the awakening has reached (never past the real one).
    pub fn display_eye(&self) -> Eye {
        if self.animated() {
            self.shown.min(self.eye())
        } else {
            self.eye()
        }
    }

    /// Advances the animation and applies the grace periods.
    pub fn on_timer(&mut self, now: Instant) {
        self.clock = self.elapsed(now);
        if self.mode == SplashMode::Animated {
            self.tick = self.tick.wrapping_add(1);
            if self.tick.is_multiple_of(self.pace.eye_stage()) && self.shown < self.eye() {
                self.shown = self.shown.next();
            }
            let target = crate::observer::glow_target(self.shown);
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
            && now.saturating_duration_since(self.started) < self.pace.minimum();
        !self.dismissed && (lingering || showing)
    }

    /// Time since the splash appeared.
    pub fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
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
        s.resolve(Step::Configuration, Status::Done("config.toml".into()), t0);
        s.resolve(Step::Dashboard, Status::Done("12 ranked".into()), t0);
        assert_eq!(s.eye(), Eye::Open);
        assert!(
            s.visible(t0 + Duration::from_millis(100)),
            "lingers briefly"
        );
        let gone = t0 + LINGER.max(Pace::Cinematic.minimum());
        let almost = t0
            + LINGER
                .max(Pace::Cinematic.minimum())
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
    fn background_work_may_still_report_how_it_ended() {
        let (mut s, t0) = startup(SplashMode::Animated);
        s.resolve(Step::Profile, Status::Background("GitHub later".into()), t0);
        s.resolve(Step::Profile, Status::Background("again".into()), t0);
        assert_eq!(
            s.status(Step::Profile),
            &Status::Background("GitHub later".into())
        );
        s.resolve(Step::Profile, Status::Done("@octocat".into()), t0);
        assert_eq!(s.status(Step::Profile), &Status::Done("@octocat".into()));
        let tags: Vec<_> = s.events.iter().map(|e| (e.tag, e.level)).collect();
        assert_eq!(
            tags,
            [("profile", Level::Background), ("profile", Level::Done)]
        );
    }

    #[test]
    fn the_log_records_only_what_happened_and_stays_bounded() {
        let (mut s, t0) = startup(SplashMode::Animated);
        assert!(s.events.is_empty(), "nothing before it happens");
        for step in Step::ALL {
            s.resolve(
                step,
                Status::Done("ok".into()),
                t0 + Duration::from_millis(300),
            );
        }
        assert_eq!(
            s.events.len(),
            Step::ALL.len() + 1,
            "one per step, then ready"
        );
        assert_eq!(s.events[0].at, Duration::from_millis(300));
        assert_eq!(s.events.last().unwrap().text, "all systems ready");
        for i in 0..200 {
            s.log(t0, Level::Info, "sources", format!("{i}"));
        }
        assert_eq!(s.events.len(), LOG_LIMIT);
        assert_eq!(s.events.last().unwrap().text, "199");
    }

    #[test]
    fn pace_sets_the_minimum_and_brief_is_short() {
        assert_eq!(Pace::from_name("brief"), Pace::Brief);
        assert_eq!(Pace::from_name("cinematic"), Pace::Cinematic);
        assert_eq!(Pace::from_name("anything"), Pace::Cinematic);
        let (mut s, t0) = startup(SplashMode::Animated);
        s.pace = Pace::Brief;
        for step in Step::ALL {
            s.resolve(step, Status::Done("ok".into()), t0);
        }
        assert!(!s.visible(t0 + Duration::from_secs(2)));
        s.pace = Pace::Cinematic;
        assert!(s.visible(t0 + Duration::from_secs(6)), "cinematic lingers");
        assert!(!s.visible(t0 + Duration::from_secs(7)));
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
        // Nothing waits on the work: only the paced minimum remains, and
        // any key skips even that.
        let end = t0 + Pace::Cinematic.minimum().max(MAX_WAIT + LINGER);
        assert!(!s.visible(end), "never blocks startup");
        s.pace = Pace::Brief;
        assert!(!s.visible(t0 + MAX_WAIT + LINGER));
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
    fn the_drawn_eye_opens_in_stages_but_never_ahead_of_progress() {
        let (mut s, t0) = startup(SplashMode::Animated);
        s.resolve(Step::Observer, Status::Done("ok".into()), t0);
        for _ in 0..40 {
            s.on_timer(t0);
        }
        assert_eq!(s.display_eye(), Eye::Stirring, "capped by real progress");
        for step in Step::ALL {
            s.resolve(step, Status::Done("ok".into()), t0);
        }
        assert_eq!(s.eye(), Eye::Open);
        let mut stages = vec![s.display_eye()];
        for _ in 0..60 {
            s.on_timer(t0);
            if stages.last() != Some(&s.display_eye()) {
                stages.push(s.display_eye());
            }
        }
        assert_eq!(stages, [Eye::Stirring, Eye::Half, Eye::Open]);
        let (mut still, t0) = startup(SplashMode::Static);
        still.resolve(Step::Observer, Status::Done("ok".into()), t0);
        assert_eq!(still.display_eye(), still.eye());
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
        for _ in 0..90 {
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
