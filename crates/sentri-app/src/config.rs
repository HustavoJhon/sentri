//! `config.toml` model, loading and validation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sentri_core::{
    CanonicalUrl, Category, ContentKind, Discovery, Profile, SourceKind, Stance, Weights,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Template written by `sentri config init`; also used when no file exists.
pub const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid configuration in {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    #[error("invalid configuration: {0}")]
    Invalid(String),
    #[error("{} already exists", .0.display())]
    AlreadyExists(PathBuf),
    #[error("cannot write {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub database: Option<PathBuf>,
}

/// Limits for every outbound HTTP request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpConfig {
    pub timeout_secs: u64,
    pub max_body_bytes: u64,
    pub max_redirects: u8,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 20,
            max_body_bytes: 3_000_000,
            max_redirects: 5,
        }
    }
}

/// How images are drawn in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageMode {
    /// Asks the terminal at startup: Kitty, Sixel or iTerm2 graphics when
    /// supported, colored half blocks otherwise.
    #[default]
    Auto,
    /// Kitty graphics protocol (kitty, Ghostty, recent `WezTerm`).
    Kitty,
    /// Sixel graphics (foot, xterm `-ti vt340`, Windows Terminal, tmux 3.4+).
    Sixel,
    /// iTerm2 inline images (iTerm2, `WezTerm`, Konsole).
    Iterm2,
    /// Colored half blocks (`▀`): works in any truecolor or 256-color terminal.
    Halfblock,
    /// Grayscale ASCII ramp, for monochrome or very limited terminals.
    Ascii,
    Off,
}

/// Terminal color depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    /// Truecolor when `COLORTERM` says so, otherwise 256 colors.
    #[default]
    Auto,
    Truecolor,
    Indexed,
}

/// Appearance and interface preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// `compact`, `comfortable` or `detailed`.
    pub density: String,
    pub images: ImageMode,
    pub colors: ColorMode,
    /// Use ASCII instead of Unicode icons.
    pub ascii_icons: bool,
    pub show_sidebar: bool,
    pub show_details: bool,
    /// Percentage of the main area used by the feed when details are shown.
    pub feed_width: u16,
    /// Where downloaded images and avatars are cached.
    pub cache_dir: Option<PathBuf>,
    /// Collect when the interface opens: `auto` (empty database or data older
    /// than `daemon.interval_minutes`), `always` or `never`.
    pub collect_on_start: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            density: "comfortable".into(),
            images: ImageMode::default(),
            colors: ColorMode::default(),
            ascii_icons: false,
            show_sidebar: true,
            show_details: true,
            feed_width: 55,
            cache_dir: None,
            collect_on_start: "auto".into(),
        }
    }
}

/// Dashboard widgets, in the order they are drawn.
pub const DASHBOARD_WIDGETS: &[&str] = &[
    "metrics",
    "activity",
    "interactions",
    "distribution",
    "recommendations",
    "upcoming",
    "news",
    "jobs",
];

/// Counters the metrics strip can show.
pub const DASHBOARD_METRICS: &[&str] = &[
    "found", "new", "news", "events", "courses", "jobs", "saved", "liked", "applied", "calendar",
];

/// Facts a card can show under its title.
pub const CARD_FACTS: &[&str] = &[
    "when",
    "deadline",
    "pay",
    "organizer",
    "place",
    "modality",
    "cost",
    "perks",
    "topic",
    "skills",
    "links",
];

fn all(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

/// What the dashboard shows and how densely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DashboardConfig {
    /// `detailed` (charts with room to breathe) or `compact` (one-line
    /// metrics, short charts, more list rows).
    pub layout: String,
    /// Widgets to show, in order. Leaving one out hides it.
    pub widgets: Vec<String>,
    /// Counters in the metrics strip, in order.
    pub metrics: Vec<String>,
    /// Maximum rows in the recommendations list.
    pub recommendations: u16,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            layout: "detailed".into(),
            widgets: all(DASHBOARD_WIDGETS),
            metrics: all(DASHBOARD_METRICS),
            recommendations: 10,
        }
    }
}

/// How feed cards present an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CardsConfig {
    pub show_source: bool,
    pub show_category: bool,
    pub show_meter: bool,
    /// Facts to show under the title. Leaving one out hides it.
    pub facts: Vec<String>,
    /// `kind`: each kind of card keeps its own arrangement; `custom`: facts
    /// follow the order of `facts`.
    pub fact_order: String,
    /// `both` (`Fri 20 Nov · in 3d`), `relative` or `absolute`.
    pub dates: String,
    /// `highlight` (colored left bar plus a shaded card) or `rail` (bar only).
    pub selection: String,
}

impl Default for CardsConfig {
    fn default() -> Self {
        Self {
            show_source: true,
            show_category: true,
            show_meter: true,
            facts: all(CARD_FACTS),
            fact_order: "kind".into(),
            dates: "both".into(),
            selection: "highlight".into(),
        }
    }
}

/// Local calendar preferences and Google Calendar setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarConfig {
    /// `monday` or `sunday`.
    pub week_start: String,
    /// `month`, `week`, `day` or `agenda`.
    pub default_view: String,
    /// OAuth client file for the future Google Calendar sync.
    pub google_credentials: Option<PathBuf>,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            week_start: "monday".into(),
            default_view: "month".into(),
            google_credentials: None,
        }
    }
}

impl CalendarConfig {
    pub fn credentials_path(&self) -> PathBuf {
        self.google_credentials
            .clone()
            .unwrap_or_else(crate::google::default_credentials_path)
    }
}

/// GitHub identity (public profile only; no token needed).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHubConfig {
    /// Your GitHub username, used for the avatar and profile link.
    pub username: Option<String>,
}

/// First-run setup and the startup animation.
///
/// A file without this section belongs to someone who used SENTRI before
/// onboarding existed, so `completed` defaults to `true` and they are never
/// sent through it. The template written for new users sets it to `false`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnboardingConfig {
    pub completed: bool,
    /// Show the Observer awakening while SENTRI starts.
    pub startup_animation: bool,
    /// Step to resume an interrupted onboarding from (absent when done).
    pub step: Option<u8>,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self {
            completed: true,
            startup_animation: true,
            step: None,
        }
    }
}

/// Last onboarding step index (the review screen).
pub const ONBOARDING_LAST_STEP: u8 = 5;

/// Background collection settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonConfig {
    /// Minutes between collection cycles (minimum 15).
    pub interval_minutes: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_minutes: 120,
        }
    }
}

/// One configured source instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    /// Unique, stable name (used in storage).
    pub name: String,
    pub kind: SourceKind,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Feed or endpoint URL, for kinds that need one.
    #[serde(default)]
    pub url: Option<String>,
    /// Search query, for kinds that support one.
    #[serde(default)]
    pub query: Option<String>,
    /// Connector-specific target (GitHub: `repositories` or `issues`).
    #[serde(default)]
    pub target: Option<String>,
    /// Category assigned when the item does not reveal one.
    #[serde(default)]
    pub category: Option<Category>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    /// Name of the environment variable holding a token, never the token.
    #[serde(default)]
    pub token_env: Option<String>,
    /// `opportunity` (default) or `news`.
    #[serde(default)]
    pub content: ContentKind,
    /// Default stance of a news source (`official` for company blogs).
    #[serde(default)]
    pub stance: Option<Stance>,
}

const fn enabled_by_default() -> bool {
    true
}

const fn default_max_items() -> usize {
    50
}

/// The whole configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub storage: StorageConfig,
    pub onboarding: OnboardingConfig,
    pub profile: Profile,
    pub discovery: Discovery,
    pub scoring: Weights,
    pub http: HttpConfig,
    pub daemon: DaemonConfig,
    pub ui: UiConfig,
    pub github: GitHubConfig,
    pub calendar: CalendarConfig,
    pub dashboard: DashboardConfig,
    pub cards: CardsConfig,
    /// Keyboard shortcuts: action name → key (see `sentri_app::keys`).
    pub keys: std::collections::BTreeMap<String, String>,
    pub sources: Vec<SourceConfig>,
}

impl Config {
    /// Parses and validates a TOML document.
    pub fn parse(text: &str, origin: &Path) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(|e| ConfigError::Parse {
            path: origin.to_path_buf(),
            source: Box::new(e),
        })?;
        config.validate()?;
        Ok(config)
    }

    /// Loads `path`, or the built-in defaults when the file does not exist.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(path = %path.display(), "no configuration file; using defaults");
                Self::parse(DEFAULT_CONFIG, Path::new("<built-in defaults>"))
            }
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Writes the commented default template to `path` without overwriting.
    pub fn write_default(path: &Path) -> Result<(), ConfigError> {
        if path.exists() {
            return Err(ConfigError::AlreadyExists(path.to_path_buf()));
        }
        let write = || -> std::io::Result<()> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, DEFAULT_CONFIG)
        };
        write().map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Whether the first-run wizard still has to run.
    pub const fn needs_onboarding(&self) -> bool {
        !self.onboarding.completed
    }

    /// Database path from the configuration or the default location.
    pub fn database_path(&self) -> PathBuf {
        self.storage
            .database
            .clone()
            .unwrap_or_else(crate::paths::default_database)
    }

    pub fn source(&self, name: &str) -> Option<&SourceConfig> {
        self.sources.iter().find(|s| s.name == name)
    }

    fn validate_presentation(&self) -> Result<(), ConfigError> {
        fn list(key: &str, values: &[String], allowed: &[&str]) -> Result<(), ConfigError> {
            let mut seen = HashSet::new();
            for v in values {
                if !allowed.contains(&v.as_str()) {
                    return Err(ConfigError::Invalid(format!(
                        "{key}: unknown {v:?} (use {})",
                        allowed.join(", ")
                    )));
                }
                if !seen.insert(v) {
                    return Err(ConfigError::Invalid(format!("{key}: {v:?} is repeated")));
                }
            }
            Ok(())
        }
        fn choice(key: &str, value: &str, allowed: &[&str]) -> Result<(), ConfigError> {
            if allowed.contains(&value) {
                Ok(())
            } else {
                Err(ConfigError::Invalid(format!(
                    "{key} must be one of: {}",
                    allowed.join(", ")
                )))
            }
        }
        let d = &self.dashboard;
        choice("dashboard.layout", &d.layout, &["detailed", "compact"])?;
        list("dashboard.widgets", &d.widgets, DASHBOARD_WIDGETS)?;
        list("dashboard.metrics", &d.metrics, DASHBOARD_METRICS)?;
        if !(1..=30).contains(&d.recommendations) {
            return Err(ConfigError::Invalid(
                "dashboard.recommendations must be between 1 and 30".into(),
            ));
        }
        let c = &self.cards;
        list("cards.facts", &c.facts, CARD_FACTS)?;
        choice("cards.fact_order", &c.fact_order, &["kind", "custom"])?;
        choice("cards.dates", &c.dates, &["both", "relative", "absolute"])?;
        choice("cards.selection", &c.selection, &["highlight", "rail"])?;
        Ok(())
    }

    fn validate_profile(&self) -> Result<(), ConfigError> {
        let p = &self.profile;
        if let Some(code) = p.country_code.as_deref().filter(|c| !c.is_empty())
            && sentri_core::country::normalize_code(code).is_none()
        {
            return Err(ConfigError::Invalid(format!(
                "profile.country_code {code:?} is not an ISO 3166-1 alpha-2 code (e.g. PE, MX, ES)"
            )));
        }
        if let Some(zone) = p.timezone.as_deref().filter(|z| !z.is_empty())
            && !crate::timezone::valid(zone)
        {
            return Err(ConfigError::Invalid(format!(
                "profile.timezone {zone:?} is not a time zone name like America/Lima"
            )));
        }
        for (key, value) in [("home_city", &p.home_city), ("region", &p.region)] {
            if value.as_deref().is_some_and(|v| v.chars().count() > 80) {
                return Err(ConfigError::Invalid(format!(
                    "profile.{key} is longer than 80 characters"
                )));
            }
        }
        if self
            .onboarding
            .step
            .is_some_and(|s| s > ONBOARDING_LAST_STEP)
        {
            return Err(ConfigError::Invalid(format!(
                "onboarding.step must be between 0 and {ONBOARDING_LAST_STEP}"
            )));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut names = HashSet::new();
        for source in &self.sources {
            let name = source.name.trim();
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            {
                return Err(ConfigError::Invalid(format!(
                    "source name {:?} must be non-empty and use only letters, digits, '-', '_' or '.'",
                    source.name
                )));
            }
            if !names.insert(name) {
                return Err(ConfigError::Invalid(format!(
                    "duplicate source name {name:?}"
                )));
            }
            if let Some(url) = &source.url {
                // `webcal://` is how calendars offer subscriptions; it is https.
                let url = url
                    .strip_prefix("webcal://")
                    .map_or_else(|| url.clone(), |rest| format!("https://{rest}"));
                CanonicalUrl::parse(&url).map_err(|e| {
                    ConfigError::Invalid(format!("source {name:?} has an invalid url: {e}"))
                })?;
            }
            if source.max_items == 0 || source.max_items > 500 {
                return Err(ConfigError::Invalid(format!(
                    "source {name:?}: max_items must be between 1 and 500"
                )));
            }
        }
        if !["compact", "comfortable", "detailed"].contains(&self.ui.density.as_str()) {
            return Err(ConfigError::Invalid(
                "ui.density must be compact, comfortable or detailed".into(),
            ));
        }
        if !(30..=80).contains(&self.ui.feed_width) {
            return Err(ConfigError::Invalid(
                "ui.feed_width must be between 30 and 80".into(),
            ));
        }
        if let Some(user) = self.github.username.as_deref().filter(|u| !u.is_empty())
            && !crate::profile::valid_username(user)
        {
            return Err(ConfigError::Invalid(format!(
                "github.username {user:?} is not a valid GitHub username"
            )));
        }
        self.validate_profile()?;
        if let Err(errors) = crate::keys::effective(&self.keys) {
            return Err(ConfigError::Invalid(format!("keys: {}", errors.join("; "))));
        }
        if !["monday", "sunday"].contains(&self.calendar.week_start.as_str()) {
            return Err(ConfigError::Invalid(
                "calendar.week_start must be monday or sunday".into(),
            ));
        }
        if !["month", "week", "day", "agenda"].contains(&self.calendar.default_view.as_str()) {
            return Err(ConfigError::Invalid(
                "calendar.default_view must be month, week, day or agenda".into(),
            ));
        }
        if !["auto", "always", "never"].contains(&self.ui.collect_on_start.as_str()) {
            return Err(ConfigError::Invalid(
                "ui.collect_on_start must be auto, always or never".into(),
            ));
        }
        self.validate_presentation()?;
        if self.daemon.interval_minutes < 15 {
            return Err(ConfigError::Invalid(
                "daemon.interval_minutes must be at least 15 (be polite with sources)".into(),
            ));
        }
        if self.http.max_body_bytes == 0 || self.http.timeout_secs == 0 {
            return Err(ConfigError::Invalid(
                "http.max_body_bytes and http.timeout_secs must be positive".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentri_core::{Modality, Priority};

    #[test]
    fn default_template_is_valid_and_reflects_initial_profile() {
        let config = Config::parse(DEFAULT_CONFIG, Path::new("default")).unwrap();
        assert_eq!(config.profile.home_city.as_deref(), Some("Cusco"));
        assert_eq!(config.profile.country.as_deref(), Some("Peru"));
        assert!(
            config
                .profile
                .preferred_modalities
                .contains(&Modality::Online)
        );
        assert!(
            config
                .profile
                .interests
                .iter()
                .any(|i| i.priority == Priority::High)
        );
        assert_eq!(config.scoring, Weights::default());
        assert!(config.source("freecodecamp-news").unwrap().enabled);
        assert!(!config.source("github-good-first-issues").unwrap().enabled);
        let events = config.source("peruanos-events").unwrap();
        assert!(events.enabled);
        assert_eq!(events.target.as_deref(), Some("events"));
        assert_eq!(events.kind, SourceKind::Peruanos);
        assert_eq!(
            config.source("peruanos-cfs").unwrap().target.as_deref(),
            Some("cfs")
        );
        assert_eq!(config.source("hack0").unwrap().kind, SourceKind::Hack0);
        assert!(config.source("gdg-lima").is_some());
        assert!(config.source("gdg-cloud-lima").is_some());
    }

    #[test]
    fn partial_files_use_defaults() {
        let config = Config::parse("[scoring]\ntravel = 30\n", Path::new("x")).unwrap();
        assert_eq!(config.scoring.travel, 30);
        assert_eq!(config.scoring.home_city, Weights::default().home_city);
        assert_eq!(config.http, HttpConfig::default());
    }

    #[test]
    fn rejects_mistakes() {
        let bad = [
            "unknown_key = 1",
            "[[sources]]\nname = \"a\"\nkind = \"rss\"\n[[sources]]\nname = \"a\"\nkind = \"rss\"",
            "[[sources]]\nname = \"a b\"\nkind = \"rss\"",
            "[[sources]]\nname = \"a\"\nkind = \"rss\"\nurl = \"file:///etc/passwd\"",
            "[[sources]]\nname = \"a\"\nkind = \"ical\"\nurl = \"webcal:/nohost\"",
            "[[sources]]\nname = \"a\"\nkind = \"nope\"",
            "[[sources]]\nname = \"a\"\nkind = \"rss\"\nmax_items = 0",
            "[daemon]\ninterval_minutes = 1",
            "[keys]\nlike = \"s\"",
            "[keys]\nsave = \"j\"",
            "[profile]\ncountry_code = \"XX\"",
            "[profile]\ncountry_code = \"PER\"",
            "[profile]\ntimezone = \"Lima\"",
            "[onboarding]\nstep = 9",
            "[onboarding]\nnope = true",
            "[discovery]\ntopics = [\"cooking\"]",
        ];
        for text in bad {
            assert!(
                Config::parse(text, Path::new("x")).is_err(),
                "accepted: {text}"
            );
        }
    }

    #[test]
    fn new_users_onboard_and_existing_files_do_not() {
        let fresh = Config::parse(DEFAULT_CONFIG, Path::new("default")).unwrap();
        assert!(fresh.needs_onboarding(), "the template starts onboarding");
        assert!(fresh.onboarding.startup_animation);
        let missing = tempfile::tempdir().unwrap();
        let first_run = Config::load(&missing.path().join("config.toml")).unwrap();
        assert!(
            first_run.needs_onboarding(),
            "no file at all is a first run"
        );

        // A configuration written before onboarding existed.
        let existing = "[profile]\nhome_city = \"Arequipa\"\n[ui]\ndensity = \"compact\"\n";
        let config = Config::parse(existing, Path::new("x")).unwrap();
        assert!(!config.needs_onboarding());
        assert!(config.onboarding.startup_animation);
        assert_eq!(config.discovery, Discovery::default());
        assert_eq!(config.profile.country_code, None);
        assert_eq!(config.profile.home_city.as_deref(), Some("Arequipa"));
    }

    #[test]
    fn profile_location_and_discovery_parse() {
        let text = "[onboarding]\ncompleted = false\nstartup_animation = false\nstep = 2\n\
            [profile]\ncountry_code = \"mx\"\nregion = \"\"\ntimezone = \"America/Mexico_City\"\n\
            [discovery]\ninternational = false\ntopics = [\"hackathons\", \"open_source\"]\n";
        let config = Config::parse(text, Path::new("x")).unwrap();
        assert_eq!(config.onboarding.step, Some(2));
        assert!(!config.onboarding.startup_animation);
        assert!(!config.discovery.international && config.discovery.local);
        assert_eq!(
            config.discovery.topics,
            [
                sentri_core::Topic::Hackathons,
                sentri_core::Topic::OpenSource
            ]
        );
        assert_eq!(
            config.profile.region.as_deref(),
            Some(""),
            "optional fields may stay empty"
        );
        assert!(
            config
                .profile
                .location_terms()
                .contains(&"méxico".to_owned())
        );
    }

    #[test]
    fn accepts_webcal_subscriptions() {
        let text =
            "[[sources]]\nname = \"l\"\nkind = \"luma\"\nurl = \"webcal://api.lu.ma/ics/get?id=1\"";
        assert!(Config::parse(text, Path::new("x")).is_ok());
    }

    #[test]
    fn writes_template_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sentri/config.toml");
        Config::write_default(&path).unwrap();
        assert!(Config::load(&path).is_ok());
        assert!(matches!(
            Config::write_default(&path),
            Err(ConfigError::AlreadyExists(_))
        ));
    }
}
