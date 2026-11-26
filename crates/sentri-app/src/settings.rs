//! Settings editable from the TUI, written back to `config.toml`.
//!
//! Writing preserves the user's comments and layout (`toml_edit`), validates
//! the whole resulting document with the normal loader before touching the
//! file, and keeps a timestamped backup of the previous version.

use std::path::{Path, PathBuf};

use chrono::Local;
use thiserror::Error;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::{CARD_FACTS, DASHBOARD_METRICS, DASHBOARD_WIDGETS, DEFAULT_CONFIG};
use crate::{Config, ConfigError};

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("unknown setting {0:?}")]
    Unknown(String),
    #[error("{key}: {message}")]
    Invalid { key: String, message: String },
    #[error("configuration file is not valid TOML: {0}")]
    Parse(String),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot write configuration: {0}")]
    Io(#[from] std::io::Error),
}

/// How a setting is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    Bool,
    Choice(&'static [&'static str]),
    Int {
        min: i64,
        max: i64,
    },
    /// Free text; empty removes the key (back to the default).
    Text,
    /// Comma-separated subset of the options, in the chosen order; stored
    /// as a TOML array. Empty means none.
    List(&'static [&'static str]),
}

/// Parses a [`SettingKind::List`] value: trimmed, known, not repeated.
pub fn parse_list(text: &str, options: &[&str]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for item in text.split(',').map(str::trim).filter(|i| !i.is_empty()) {
        if !options.contains(&item) {
            return Err(format!("unknown {item:?}; use: {}", options.join(", ")));
        }
        if out.iter().any(|o| o == item) {
            return Err(format!("{item:?} is repeated"));
        }
        out.push(item.to_owned());
    }
    Ok(out)
}

/// A setting with a fixed key path such as `ui.density`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    pub key: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub kind: SettingKind,
    pub help: &'static str,
    /// Applied live by the TUI (the rest after restarting).
    pub live: bool,
}

const fn s(
    key: &'static str,
    group: &'static str,
    label: &'static str,
    kind: SettingKind,
    help: &'static str,
    live: bool,
) -> Setting {
    Setting {
        key,
        group,
        label,
        kind,
        help,
        live,
    }
}

/// Every fixed setting. Per-source settings are generated from the file.
pub const SETTINGS: &[Setting] = &[
    s(
        "ui.density",
        "Appearance",
        "Card density",
        SettingKind::Choice(&["compact", "comfortable", "detailed"]),
        "How much each card shows",
        true,
    ),
    s(
        "ui.images",
        "Appearance",
        "Images",
        SettingKind::Choice(&[
            "auto",
            "kitty",
            "sixel",
            "iterm2",
            "halfblock",
            "ascii",
            "off",
        ]),
        "auto detects Kitty/Sixel/iTerm2 graphics, else half blocks",
        true,
    ),
    s(
        "ui.colors",
        "Appearance",
        "Colors",
        SettingKind::Choice(&["auto", "truecolor", "indexed"]),
        "auto uses COLORTERM",
        false,
    ),
    s(
        "ui.ascii_icons",
        "Appearance",
        "ASCII icons",
        SettingKind::Bool,
        "Avoid Unicode symbols",
        true,
    ),
    s(
        "ui.show_sidebar",
        "Appearance",
        "Show sidebar",
        SettingKind::Bool,
        "Hidden sidebar: Tab switches sections",
        true,
    ),
    s(
        "ui.show_details",
        "Appearance",
        "Show details pane",
        SettingKind::Bool,
        "Enter still opens details",
        true,
    ),
    s(
        "ui.feed_width",
        "Appearance",
        "Feed width %",
        SettingKind::Int { min: 30, max: 80 },
        "Share of the screen used by the feed",
        true,
    ),
    s(
        "ui.collect_on_start",
        "Appearance",
        "Collect on start",
        SettingKind::Choice(&["auto", "always", "never"]),
        "auto: when empty or older than the daemon interval",
        true,
    ),
    s(
        "onboarding.startup_animation",
        "Appearance",
        "Startup animation",
        SettingKind::Bool,
        "The Observer awakening while SENTRI starts",
        false,
    ),
    s(
        "onboarding.startup_style",
        "Appearance",
        "Startup pace",
        SettingKind::Choice(&["cinematic", "brief"]),
        "cinematic: about seven seconds; brief: about two (any key skips)",
        false,
    ),
    s(
        "dashboard.layout",
        "Dashboard",
        "Layout",
        SettingKind::Choice(&["detailed", "compact"]),
        "compact: one-line metrics and short charts",
        true,
    ),
    s(
        "dashboard.widgets",
        "Dashboard",
        "Widgets (in order)",
        SettingKind::List(DASHBOARD_WIDGETS),
        "Comma-separated; leave one out to hide it, reorder to move it",
        true,
    ),
    s(
        "dashboard.metrics",
        "Dashboard",
        "Metrics (in order)",
        SettingKind::List(DASHBOARD_METRICS),
        "Counters in the metrics strip",
        true,
    ),
    s(
        "dashboard.recommendations",
        "Dashboard",
        "Recommendations",
        SettingKind::Int { min: 1, max: 30 },
        "Rows in the recommendations list",
        true,
    ),
    s(
        "cards.show_category",
        "Cards",
        "Show category",
        SettingKind::Bool,
        "JOB, MEETUP, OFFICIAL… label on each card",
        true,
    ),
    s(
        "cards.show_source",
        "Cards",
        "Show source",
        SettingKind::Bool,
        "Platform badge and publisher name",
        true,
    ),
    s(
        "cards.show_meter",
        "Cards",
        "Show match meter",
        SettingKind::Bool,
        "The ●●●○○ relevance scale",
        true,
    ),
    s(
        "cards.facts",
        "Cards",
        "Facts",
        SettingKind::List(CARD_FACTS),
        "Comma-separated; leave one out to hide it",
        true,
    ),
    s(
        "cards.fact_order",
        "Cards",
        "Fact order",
        SettingKind::Choice(&["kind", "custom"]),
        "kind: each card type's own order · custom: the Facts order",
        true,
    ),
    s(
        "cards.dates",
        "Cards",
        "Dates",
        SettingKind::Choice(&["both", "relative", "absolute"]),
        "Fri 20 Nov · in 3d, only in 3d, or only the date",
        true,
    ),
    s(
        "cards.selection",
        "Cards",
        "Selection style",
        SettingKind::Choice(&["highlight", "rail"]),
        "rail: only the colored bar marks the selected card",
        true,
    ),
    s(
        "profile.home_city",
        "Profile",
        "Home city",
        SettingKind::Text,
        "In-person events here rank higher",
        true,
    ),
    s(
        "profile.country",
        "Profile",
        "Country",
        SettingKind::Text,
        "Used for eligibility",
        true,
    ),
    s(
        "profile.country_code",
        "Profile",
        "Country code",
        SettingKind::Text,
        "ISO 3166-1 alpha-2 (PE, MX, ES…); adds its names to matching",
        true,
    ),
    s(
        "profile.region",
        "Profile",
        "Region / state",
        SettingKind::Text,
        "Optional",
        true,
    ),
    s(
        "profile.timezone",
        "Profile",
        "Time zone",
        SettingKind::Text,
        "IANA name such as America/Lima; empty uses the system zone",
        true,
    ),
    s(
        "github.username",
        "Profile",
        "GitHub username",
        SettingKind::Text,
        "Public profile, avatar and languages; no token",
        true,
    ),
    s(
        "profile.preferred_modalities",
        "Discovery",
        "Modalities",
        SettingKind::List(MODALITIES),
        "online (remote), hybrid, in_person; matching ones rank higher",
        true,
    ),
    s(
        "discovery.local",
        "Discovery",
        "Local",
        SettingKind::Bool,
        "Boost opportunities in your city",
        true,
    ),
    s(
        "discovery.national",
        "Discovery",
        "National",
        SettingKind::Bool,
        "Boost opportunities in your country",
        true,
    ),
    s(
        "discovery.international",
        "Discovery",
        "International",
        SettingKind::Bool,
        "Boost opportunities abroad that are open to you",
        true,
    ),
    s(
        "discovery.topics",
        "Discovery",
        "Interests",
        SettingKind::List(TOPICS),
        "Comma-separated; matching items rank higher, nothing is hidden",
        true,
    ),
    s(
        "daemon.interval_minutes",
        "Collection",
        "Daemon interval (min)",
        SettingKind::Int { min: 15, max: 1440 },
        "Minutes between collection cycles",
        false,
    ),
    s(
        "http.timeout_secs",
        "Collection",
        "Request timeout (s)",
        SettingKind::Int { min: 5, max: 120 },
        "Per request",
        false,
    ),
    s(
        "scoring.home_city",
        "Recommendations",
        "Weight: home city",
        SettingKind::Int {
            min: -100,
            max: 100,
        },
        "Points for events in your city",
        true,
    ),
    s(
        "scoring.travel",
        "Recommendations",
        "Weight: travel covered",
        SettingKind::Int {
            min: -100,
            max: 100,
        },
        "Points when travel or lodging is covered",
        true,
    ),
    s(
        "scoring.interest_keyword",
        "Recommendations",
        "Weight: interest keyword",
        SettingKind::Int {
            min: -100,
            max: 100,
        },
        "Per matching keyword, times priority",
        true,
    ),
    s(
        "scoring.deadline_soon_days",
        "Recommendations",
        "Deadline soon (days)",
        SettingKind::Int { min: 1, max: 90 },
        "Window for the 'deadline soon' bonus",
        true,
    ),
    s(
        "scoring.news_fresh",
        "Recommendations",
        "Weight: fresh news",
        SettingKind::Int {
            min: -100,
            max: 100,
        },
        "News from the last 24 hours",
        true,
    ),
];

/// Values of `profile.preferred_modalities` offered in Settings.
pub const MODALITIES: &[&str] = &["online", "hybrid", "in_person"];

/// Values of `discovery.topics`.
pub const TOPICS: &[&str] = &[
    "events",
    "courses",
    "hackathons",
    "scholarships",
    "competitions",
    "communities",
    "open_source",
    "cfps",
    "news",
    "jobs",
];

/// Keys written by onboarding that Settings does not list.
const INTERNAL: &[(&str, SettingKind)] = &[
    ("onboarding.completed", SettingKind::Bool),
    (
        "onboarding.step",
        SettingKind::Int {
            min: 0,
            max: crate::config::ONBOARDING_LAST_STEP as i64,
        },
    ),
    ("profile.country", SettingKind::Text),
];

/// One setting per remappable shortcut (`keys.<action>`).
pub fn key_settings() -> Vec<(String, String, SettingKind)> {
    crate::keys::KeyAction::ALL
        .iter()
        .map(|a| {
            (
                format!("keys.{}", a.name()),
                a.label().to_owned(),
                SettingKind::Text,
            )
        })
        .collect()
}

/// A setting for one configured source (`sources.<name>.enabled|max_items`).
pub fn source_settings(config: &Config) -> Vec<(String, String, SettingKind)> {
    config
        .sources
        .iter()
        .flat_map(|src| {
            [
                (
                    format!("sources.{}.enabled", src.name),
                    format!("{} · enabled", src.name),
                    SettingKind::Bool,
                ),
                (
                    format!("sources.{}.max_items", src.name),
                    format!("{} · max items", src.name),
                    SettingKind::Int { min: 1, max: 500 },
                ),
            ]
        })
        .collect()
}

/// Current value of a setting, as text.
pub fn current(config: &Config, key: &str) -> Option<String> {
    let ui = &config.ui;
    let text = |v: &Option<String>| v.clone().unwrap_or_default();
    Some(match key {
        "ui.density" => ui.density.clone(),
        "ui.images" => format!("{:?}", ui.images).to_lowercase(),
        "ui.colors" => format!("{:?}", ui.colors).to_lowercase(),
        "ui.ascii_icons" => ui.ascii_icons.to_string(),
        "ui.show_sidebar" => ui.show_sidebar.to_string(),
        "ui.show_details" => ui.show_details.to_string(),
        "ui.feed_width" => ui.feed_width.to_string(),
        "ui.collect_on_start" => ui.collect_on_start.clone(),
        "dashboard.layout" => config.dashboard.layout.clone(),
        "dashboard.widgets" => config.dashboard.widgets.join(", "),
        "dashboard.metrics" => config.dashboard.metrics.join(", "),
        "dashboard.recommendations" => config.dashboard.recommendations.to_string(),
        "cards.show_category" => config.cards.show_category.to_string(),
        "cards.show_source" => config.cards.show_source.to_string(),
        "cards.show_meter" => config.cards.show_meter.to_string(),
        "cards.facts" => config.cards.facts.join(", "),
        "cards.fact_order" => config.cards.fact_order.clone(),
        "cards.dates" => config.cards.dates.clone(),
        "cards.selection" => config.cards.selection.clone(),
        "profile.home_city" => text(&config.profile.home_city),
        "profile.country" => text(&config.profile.country),
        "profile.country_code" => text(&config.profile.country_code),
        "profile.region" => text(&config.profile.region),
        "profile.timezone" => text(&config.profile.timezone),
        "profile.preferred_modalities" => config
            .profile
            .preferred_modalities
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        "discovery.local" => config.discovery.local.to_string(),
        "discovery.national" => config.discovery.national.to_string(),
        "discovery.international" => config.discovery.international.to_string(),
        "discovery.topics" => config
            .discovery
            .topics
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        "onboarding.startup_animation" => config.onboarding.startup_animation.to_string(),
        "onboarding.startup_style" => config.onboarding.startup_style.clone(),
        "onboarding.completed" => config.onboarding.completed.to_string(),
        "onboarding.step" => config.onboarding.step.unwrap_or(0).to_string(),
        "github.username" => text(&config.github.username),
        "daemon.interval_minutes" => config.daemon.interval_minutes.to_string(),
        "calendar.week_start" => config.calendar.week_start.clone(),
        "calendar.default_view" => config.calendar.default_view.clone(),
        "http.timeout_secs" => config.http.timeout_secs.to_string(),
        "scoring.home_city" => config.scoring.home_city.to_string(),
        "scoring.travel" => config.scoring.travel.to_string(),
        "scoring.interest_keyword" => config.scoring.interest_keyword.to_string(),
        "scoring.deadline_soon_days" => config.scoring.deadline_soon_days.to_string(),
        "scoring.news_fresh" => config.scoring.news_fresh.to_string(),
        other if other.starts_with("keys.") => {
            let action = crate::keys::KeyAction::from_name(&other[5..])?;
            config
                .keys
                .get(action.name())
                .cloned()
                .unwrap_or_else(|| action.default_key().to_owned())
        }
        other => {
            let rest = other.strip_prefix("sources.")?;
            let (name, field) = rest.rsplit_once('.')?;
            let src = config.source(name)?;
            match field {
                "enabled" => src.enabled.to_string(),
                "max_items" => src.max_items.to_string(),
                _ => return None,
            }
        }
    })
}

fn kind_of(config: &Config, key: &str) -> Option<SettingKind> {
    SETTINGS
        .iter()
        .find(|s| s.key == key)
        .map(|s| s.kind)
        .or_else(|| {
            INTERNAL
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, kind)| *kind)
        })
        .or_else(|| {
            source_settings(config)
                .into_iter()
                .chain(key_settings())
                .find(|(k, _, _)| k == key)
                .map(|(_, _, kind)| kind)
        })
}

fn to_item(key: &str, kind: SettingKind, text: &str) -> Result<Option<Item>, SettingsError> {
    let invalid = |message: String| SettingsError::Invalid {
        key: key.to_owned(),
        message,
    };
    let text = text.trim();
    if key == "onboarding.step" && text.is_empty() {
        // Onboarding finished: nothing left to resume.
        return Ok(None);
    }
    Ok(Some(match kind {
        SettingKind::Bool => match text {
            "true" | "yes" | "on" => value(true),
            "false" | "no" | "off" => value(false),
            _ => return Err(invalid("use true or false".into())),
        },
        SettingKind::Choice(options) => {
            if !options.contains(&text) {
                return Err(invalid(format!("one of: {}", options.join(", "))));
            }
            value(text)
        }
        SettingKind::Int { min, max } => {
            let n: i64 = text
                .parse()
                .map_err(|_| invalid("must be a whole number".into()))?;
            if !(min..=max).contains(&n) {
                return Err(invalid(format!("between {min} and {max}")));
            }
            value(n)
        }
        SettingKind::List(options) => {
            let items = parse_list(text, options).map_err(invalid)?;
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(item);
            }
            value(array)
        }
        SettingKind::Text if text.is_empty() => return Ok(None),
        SettingKind::Text if key == "profile.country_code" => value(text.to_ascii_uppercase()),
        SettingKind::Text => value(text),
    }))
}

fn set_in(doc: &mut DocumentMut, key: &str, item: Option<Item>) -> Result<(), SettingsError> {
    if let Some(rest) = key.strip_prefix("sources.") {
        let (name, field) = rest
            .rsplit_once('.')
            .ok_or_else(|| SettingsError::Unknown(key.to_owned()))?;
        let sources = doc
            .get_mut("sources")
            .and_then(Item::as_array_of_tables_mut)
            .ok_or_else(|| SettingsError::Unknown(key.to_owned()))?;
        let table = sources
            .iter_mut()
            .find(|t| t.get("name").and_then(Item::as_str) == Some(name))
            .ok_or_else(|| SettingsError::Unknown(key.to_owned()))?;
        match item {
            Some(item) => {
                table.insert(field, item);
            }
            None => {
                table.remove(field);
            }
        }
        return Ok(());
    }
    let (section, field) = key
        .split_once('.')
        .ok_or_else(|| SettingsError::Unknown(key.to_owned()))?;
    let table = doc
        .entry(section)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| SettingsError::Unknown(key.to_owned()))?;
    match item {
        Some(item) => {
            table.insert(field, item);
        }
        None => {
            table.remove(field);
        }
    }
    Ok(())
}

fn backup(path: &Path) -> Result<Option<PathBuf>, SettingsError> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let target = path.with_extension(format!("toml.bak-{stamp}"));
    std::fs::copy(path, &target)?;
    Ok(Some(target))
}

/// Writes through a temporary file and a rename, so an interrupted write
/// never leaves a half-written configuration behind.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Outcome of a save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub config: Config,
    pub backup: Option<PathBuf>,
}

/// Applies `changes` (key, text) to the file at `path`. Nothing is written
/// unless every value is valid and the resulting configuration loads.
pub fn save(path: &Path, changes: &[(String, String)]) -> Result<Saved, SettingsError> {
    save_with(path, changes, true)
}

/// [`save`], optionally without a backup (used between onboarding steps,
/// which back up once before the first change).
pub fn save_with(
    path: &Path,
    changes: &[(String, String)],
    keep_backup: bool,
) -> Result<Saved, SettingsError> {
    let original = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
        Err(e) => return Err(e.into()),
    };
    let current = Config::parse(&original, path)?;
    let mut doc: DocumentMut = original
        .parse()
        .map_err(|e: toml_edit::TomlError| SettingsError::Parse(e.to_string()))?;
    for (key, text) in changes {
        let kind = kind_of(&current, key).ok_or_else(|| SettingsError::Unknown(key.clone()))?;
        set_in(&mut doc, key, to_item(key, kind, text)?)?;
    }
    let updated = doc.to_string();
    let config = Config::parse(&updated, path)?;
    let backup = if keep_backup { backup(path)? } else { None };
    write_atomic(path, &updated)?;
    Ok(Saved { config, backup })
}

/// Marks onboarding as not done, so the wizard opens on the next start
/// (or right away in the TUI). Every answer already saved is kept.
pub fn restart_onboarding(path: &Path) -> Result<Saved, SettingsError> {
    save(
        path,
        &[
            ("onboarding.completed".into(), "false".into()),
            ("onboarding.step".into(), "0".into()),
        ],
    )
}

/// Replaces the file with the commented default template (after a backup).
pub fn restore_defaults(path: &Path) -> Result<Saved, SettingsError> {
    let backup = backup(path)?;
    write_atomic(path, DEFAULT_CONFIG)?;
    Ok(Saved {
        config: Config::parse(DEFAULT_CONFIG, path)?,
        backup,
    })
}

/// Source kinds that can be added from the TUI (feed and calendar URLs).
pub const CUSTOM_KINDS: &[&str] = &[
    "rss",
    "ical",
    "luma",
    "meetup",
    "mastodon",
    "bluesky",
    "youtube",
    "googlenews",
];

/// A source as entered in the TUI form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceDraft {
    pub name: String,
    pub kind: String,
    pub url: String,
    /// `opportunity` or `news`.
    pub content: String,
    /// Optional default category (e.g. `event`).
    pub category: String,
    pub enabled: bool,
}

fn source_table(draft: &SourceDraft) -> Result<Table, SettingsError> {
    let invalid = |message: &str| SettingsError::Invalid {
        key: "source".into(),
        message: message.into(),
    };
    if !CUSTOM_KINDS.contains(&draft.kind.as_str()) {
        return Err(invalid(&format!(
            "kind must be one of: {}",
            CUSTOM_KINDS.join(", ")
        )));
    }
    if draft.url.trim().is_empty() {
        return Err(invalid("a feed or calendar URL is required"));
    }
    let mut t = Table::new();
    t.insert("name", value(draft.name.trim()));
    t.insert("kind", value(draft.kind.as_str()));
    t.insert("url", value(draft.url.trim()));
    if draft.content == "news" {
        t.insert("content", value("news"));
    }
    if !draft.category.trim().is_empty() {
        t.insert("category", value(draft.category.trim()));
    }
    if !draft.enabled {
        t.insert("enabled", value(false));
    }
    Ok(t)
}

/// Loads, edits and writes the `[[sources]]` list; the whole document is
/// validated before anything is written.
fn edit_sources(
    path: &Path,
    edit: impl FnOnce(&mut toml_edit::ArrayOfTables) -> Result<(), SettingsError>,
) -> Result<Saved, SettingsError> {
    let original = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
        Err(e) => return Err(e.into()),
    };
    let mut doc: DocumentMut = original
        .parse()
        .map_err(|e: toml_edit::TomlError| SettingsError::Parse(e.to_string()))?;
    let sources = doc
        .entry("sources")
        .or_insert_with(|| Item::ArrayOfTables(toml_edit::ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| SettingsError::Parse("`sources` is not a list of tables".into()))?;
    edit(sources)?;
    let updated = doc.to_string();
    let config = Config::parse(&updated, path)?;
    let backup = backup(path)?;
    write_atomic(path, &updated)?;
    Ok(Saved { config, backup })
}

fn position(sources: &toml_edit::ArrayOfTables, name: &str) -> Option<usize> {
    sources
        .iter()
        .position(|t| t.get("name").and_then(Item::as_str) == Some(name))
}

/// Adds a new source at the end of the list.
pub fn add_source(path: &Path, draft: &SourceDraft) -> Result<Saved, SettingsError> {
    let table = source_table(draft)?;
    edit_sources(path, |sources| {
        sources.push(table);
        Ok(())
    })
}

/// Replaces the editable keys of source `name`, keeping any other keys
/// (`tags`, `token_env`, comments…).
pub fn update_source(path: &Path, name: &str, draft: &SourceDraft) -> Result<Saved, SettingsError> {
    let table = source_table(draft)?;
    edit_sources(path, |sources| {
        let i = position(sources, name).ok_or_else(|| SettingsError::Unknown(name.into()))?;
        let target = sources
            .get_mut(i)
            .ok_or_else(|| SettingsError::Unknown(name.into()))?;
        for key in ["name", "kind", "url", "content", "category", "enabled"] {
            match table.get(key) {
                Some(item) => {
                    target.insert(key, item.clone());
                }
                None => {
                    target.remove(key);
                }
            }
        }
        Ok(())
    })
}

/// Deletes source `name`.
pub fn remove_source(path: &Path, name: &str) -> Result<Saved, SettingsError> {
    edit_sources(path, |sources| {
        let i = position(sources, name).ok_or_else(|| SettingsError::Unknown(name.into()))?;
        sources.remove(i);
        Ok(())
    })
}

/// Default value of a fixed setting.
pub fn default_value(key: &str) -> Option<String> {
    let defaults = Config::parse(DEFAULT_CONFIG, Path::new("<defaults>")).ok()?;
    current(&defaults, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "# my notes stay\n[ui]\ndensity = \"compact\" # inline comment\n\n[[sources]]\nname = \"feed\"\nkind = \"rss\"\nurl = \"https://x.example/feed\"\n";

    #[test]
    fn lists_are_validated_and_saved_as_arrays() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        let saved = save(
            &path,
            &[
                ("dashboard.widgets".into(), "news, metrics".into()),
                ("cards.facts".into(), "pay,  place".into()),
            ],
        )
        .unwrap();
        assert_eq!(saved.config.dashboard.widgets, ["news", "metrics"]);
        assert_eq!(saved.config.cards.facts, ["pay", "place"]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("widgets = [\"news\", \"metrics\"]"), "{text}");
        assert!(text.contains("# my notes stay"));
        assert_eq!(
            current(&saved.config, "dashboard.widgets").unwrap(),
            "news, metrics"
        );

        let err = save(&path, &[("dashboard.widgets".into(), "news, chart".into())]);
        assert!(err.is_err());
        let err = save(&path, &[("cards.facts".into(), "pay, pay".into())]);
        assert!(err.is_err());
        assert_eq!(parse_list(" ", CARD_FACTS).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn saves_preserving_comments_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        let saved = save(
            &path,
            &[
                ("ui.density".into(), "detailed".into()),
                ("ui.feed_width".into(), "60".into()),
                ("sources.feed.enabled".into(), "false".into()),
                ("github.username".into(), "octocat".into()),
            ],
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my notes stay"));
        assert!(text.contains("density = \"detailed\""));
        assert!(text.contains("feed_width = 60"));
        assert_eq!(saved.config.ui.density, "detailed");
        assert!(!saved.config.source("feed").unwrap().enabled);
        assert_eq!(saved.config.github.username.as_deref(), Some("octocat"));
        let backup = saved.backup.unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), USER);
    }

    #[test]
    fn rejects_invalid_values_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        for (key, text) in [
            ("ui.feed_width", "95"),
            ("ui.density", "huge"),
            ("ui.show_sidebar", "maybe"),
            ("github.username", "../x"),
            ("nope.key", "1"),
        ] {
            assert!(save(&path, &[(key.into(), text.into())]).is_err(), "{key}");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), USER);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no backups"
        );
    }

    #[test]
    fn empty_text_removes_the_key_and_defaults_can_be_restored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[profile]\nhome_city = \"Lima\"\n").unwrap();
        let saved = save(&path, &[("profile.home_city".into(), String::new())]).unwrap();
        assert_eq!(saved.config.profile.home_city, None);
        let restored = restore_defaults(&path).unwrap();
        assert_eq!(restored.config.profile.home_city.as_deref(), Some("Cusco"));
        assert!(restored.backup.is_some());
        assert_eq!(default_value("ui.density").as_deref(), Some("comfortable"));
    }

    #[test]
    fn edits_shortcuts_and_rejects_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        let saved = save(&path, &[("keys.like".into(), "f".into())]).unwrap();
        assert_eq!(saved.config.keys.get("like").map(String::as_str), Some("f"));
        assert_eq!(current(&saved.config, "keys.like").as_deref(), Some("f"));
        assert_eq!(
            current(&saved.config, "keys.save").as_deref(),
            Some("s"),
            "default"
        );
        let conflict = save(&path, &[("keys.save".into(), "f".into())]);
        assert!(conflict.unwrap_err().to_string().contains("both"));
        let reset = save(&path, &[("keys.like".into(), String::new())]).unwrap();
        assert_eq!(
            reset.config.keys.get("like"),
            None,
            "empty restores the default"
        );
        assert_eq!(default_value("keys.like").as_deref(), Some("l"));
    }

    #[test]
    fn manages_custom_sources() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        let draft = SourceDraft {
            name: "my-blog".into(),
            kind: "rss".into(),
            url: "https://blog.example/feed.xml".into(),
            content: "news".into(),
            category: String::new(),
            enabled: true,
        };
        let saved = add_source(&path, &draft).unwrap();
        let src = saved.config.source("my-blog").unwrap();
        assert_eq!(src.content, sentri_core::ContentKind::News);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("# my notes stay")
        );

        assert!(add_source(&path, &draft).is_err(), "duplicate name");
        let bad_url = SourceDraft {
            name: "x".into(),
            url: "ftp://x".into(),
            ..draft.clone()
        };
        assert!(add_source(&path, &bad_url).is_err());
        let bad_kind = SourceDraft {
            name: "y".into(),
            kind: "linkedin".into(),
            ..draft.clone()
        };
        assert!(add_source(&path, &bad_kind).is_err());

        let renamed = SourceDraft {
            name: "my-cal".into(),
            kind: "ical".into(),
            url: "webcal://cal.example/a.ics".into(),
            content: "opportunity".into(),
            enabled: false,
            ..draft.clone()
        };
        let saved = update_source(&path, "my-blog", &renamed).unwrap();
        let cal = saved.config.source("my-cal").unwrap();
        assert!(!cal.enabled);
        assert_eq!(cal.content, sentri_core::ContentKind::Opportunity);

        let saved = remove_source(&path, "my-cal").unwrap();
        assert!(saved.config.source("my-cal").is_none());
        assert!(saved.config.source("feed").is_some(), "others untouched");
        assert!(remove_source(&path, "nope").is_err());
    }

    #[test]
    fn profile_discovery_and_onboarding_keys_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, USER).unwrap();
        let saved = save_with(
            &path,
            &[
                ("profile.country_code".into(), "co".into()),
                ("profile.region".into(), "Antioquia".into()),
                ("profile.timezone".into(), "America/Bogota".into()),
                (
                    "profile.preferred_modalities".into(),
                    "online, in_person".into(),
                ),
                ("discovery.international".into(), "false".into()),
                ("discovery.topics".into(), "jobs, news".into()),
                ("onboarding.startup_animation".into(), "false".into()),
                ("onboarding.step".into(), "3".into()),
            ],
            false,
        )
        .unwrap();
        assert_eq!(saved.backup, None);
        let c = &saved.config;
        assert_eq!(
            c.profile.country_code.as_deref(),
            Some("CO"),
            "stored uppercase"
        );
        assert_eq!(
            current(c, "profile.preferred_modalities").unwrap(),
            "online, in_person"
        );
        assert_eq!(current(c, "discovery.topics").unwrap(), "jobs, news");
        assert_eq!(current(c, "discovery.international").unwrap(), "false");
        assert_eq!(c.onboarding.step, Some(3));
        assert!(!c.onboarding.startup_animation);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("# my notes stay")
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no backup, no temp file"
        );

        for (key, text) in [
            ("profile.country_code", "Peru"),
            ("profile.timezone", "Lima"),
            ("discovery.topics", "cooking"),
            ("onboarding.step", "9"),
        ] {
            assert!(save(&path, &[(key.into(), text.into())]).is_err(), "{key}");
        }
        let cleared = save(&path, &[("profile.region".into(), String::new())]).unwrap();
        assert_eq!(
            cleared.config.profile.region, None,
            "optional fields can be emptied"
        );
    }

    #[test]
    fn onboarding_can_be_restarted_without_losing_answers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[profile]\nhome_city = \"Lima\"\n").unwrap();
        let saved = restart_onboarding(&path).unwrap();
        assert!(saved.config.needs_onboarding());
        assert_eq!(saved.config.onboarding.step, Some(0));
        assert_eq!(saved.config.profile.home_city.as_deref(), Some("Lima"));
        assert!(saved.backup.is_some());
    }

    #[test]
    fn every_setting_has_a_current_value() {
        let config = Config::parse(DEFAULT_CONFIG, Path::new("d")).unwrap();
        for setting in SETTINGS {
            assert!(current(&config, setting.key).is_some(), "{}", setting.key);
        }
        assert_eq!(
            current(&config, "sources.hn-front-page.enabled").as_deref(),
            Some("true")
        );
    }
}
