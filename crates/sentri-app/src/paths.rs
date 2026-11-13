//! Default locations of configuration and data (XDG on Linux).

use std::path::PathBuf;

use directories::ProjectDirs;

/// Environment variable overriding the configuration file path.
pub const CONFIG_ENV: &str = "SENTRI_CONFIG";

fn dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "sentri")
}

/// `~/.config/sentri/config.toml` unless `SENTRI_CONFIG` is set.
pub fn config_file() -> PathBuf {
    std::env::var_os(CONFIG_ENV).map_or_else(
        || {
            dirs().map_or_else(
                || PathBuf::from("sentri.toml"),
                |d| d.config_dir().join("config.toml"),
            )
        },
        PathBuf::from,
    )
}

/// `~/.local/share/sentri/sentri.db`.
pub fn default_database() -> PathBuf {
    dirs().map_or_else(
        || PathBuf::from("sentri.db"),
        |d| d.data_dir().join("sentri.db"),
    )
}
