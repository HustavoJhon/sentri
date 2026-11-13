//! `config.toml` model, loading and validation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sentri_core::{CanonicalUrl, Category, Profile, SourceKind, Weights};
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
    pub profile: Profile,
    pub scoring: Weights,
    pub http: HttpConfig,
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
                CanonicalUrl::parse(url).map_err(|e| {
                    ConfigError::Invalid(format!("source {name:?} has an invalid url: {e}"))
                })?;
            }
            if source.max_items == 0 || source.max_items > 500 {
                return Err(ConfigError::Invalid(format!(
                    "source {name:?}: max_items must be between 1 and 500"
                )));
            }
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
            "[[sources]]\nname = \"a\"\nkind = \"nope\"",
            "[[sources]]\nname = \"a\"\nkind = \"rss\"\nmax_items = 0",
        ];
        for text in bad {
            assert!(
                Config::parse(text, Path::new("x")).is_err(),
                "accepted: {text}"
            );
        }
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
