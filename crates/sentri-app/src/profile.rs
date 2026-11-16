//! Public GitHub profile used for SENTRI's personal identity (avatar,
//! name, link). Only public data from `GET /users/{username}`; no token.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{AppError, Sentri, Thumbnail};

const API: &str = "https://api.github.com/users/";
const TTL: Duration = Duration::from_secs(24 * 3600);

/// The public fields SENTRI shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubProfile {
    pub login: String,
    pub name: Option<String>,
    pub html_url: String,
    pub avatar_url: String,
    #[serde(default)]
    pub public_repos: u64,
    #[serde(default)]
    pub followers: u64,
}

impl GitHubProfile {
    /// Up to two initials for the fallback avatar.
    pub fn initials(&self) -> String {
        let source = self.name.as_deref().unwrap_or(&self.login);
        let mut initials: String = source
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect();
        if initials.is_empty() {
            initials = self.login.chars().take(2).collect();
        }
        initials.to_uppercase()
    }
}

/// GitHub usernames: 1–39 alphanumerics or single hyphens.
pub fn valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 39
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn parse(body: &[u8]) -> Result<GitHubProfile, AppError> {
    let mut profile: GitHubProfile = serde_json::from_slice(body)
        .map_err(|e| AppError::Profile(format!("unexpected GitHub response: {e}")))?;
    profile.name = profile
        .name
        .map(|n| sentri_core::text::clean(&n, 100))
        .filter(|n| !n.is_empty());
    profile.login = sentri_core::text::clean(&profile.login, 39);
    if !profile.avatar_url.starts_with("https://") || !profile.html_url.starts_with("https://") {
        return Err(AppError::Profile("GitHub returned non-https URLs".into()));
    }
    Ok(profile)
}

impl Sentri {
    fn profile_cache(&self, username: &str) -> PathBuf {
        self.cache_dir()
            .with_file_name(format!("github-{}.json", username.to_ascii_lowercase()))
    }

    /// The configured GitHub profile, from a 24-hour cache or the API.
    /// Returns `Ok(None)` when no username is configured.
    pub async fn github_profile(&self) -> Result<Option<GitHubProfile>, AppError> {
        let config = self.config();
        let Some(username) = config.github.username.as_deref().filter(|u| !u.is_empty()) else {
            return Ok(None);
        };
        if !valid_username(username) {
            return Err(AppError::Profile(format!(
                "{username:?} is not a valid GitHub username"
            )));
        }
        let cache = self.profile_cache(username);
        let fresh = std::fs::metadata(&cache)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < TTL);
        if fresh
            && let Ok(bytes) = std::fs::read(&cache)
            && let Ok(profile) = parse(&bytes)
        {
            return Ok(Some(profile));
        }
        let url = Url::parse(&format!("{API}{username}"))
            .map_err(|e| AppError::Profile(e.to_string()))?;
        let headers = [("accept", "application/vnd.github+json".to_owned())];
        let page = self.http().get_with(&url, &headers).await?;
        let profile = parse(&page.body)?;
        if let Some(parent) = cache.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(error) = std::fs::write(&cache, &page.body) {
            tracing::debug!(%error, "could not cache GitHub profile");
        }
        Ok(Some(profile))
    }

    /// The avatar as a small thumbnail (GitHub serves resized avatars).
    pub async fn github_avatar(&self, profile: &GitHubProfile) -> Result<Thumbnail, AppError> {
        let sep = if profile.avatar_url.contains('?') {
            '&'
        } else {
            '?'
        };
        let url = format!("{}{sep}s=96", profile.avatar_url);
        Ok(self.image_thumbnail(&url, 48, 48).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_usernames() {
        assert!(valid_username("octocat"));
        assert!(valid_username("Hustavo-Jhon"));
        assert!(!valid_username("-bad"));
        assert!(!valid_username("a--b"));
        assert!(!valid_username("../etc"));
        assert!(!valid_username(""));
    }

    #[test]
    fn parses_public_profile_and_initials() {
        let body = br#"{"login":"octocat","name":"The Octocat","html_url":"https://github.com/octocat",
            "avatar_url":"https://avatars.githubusercontent.com/u/583231?v=4","public_repos":8,"followers":10}"#;
        let p = parse(body).unwrap();
        assert_eq!(p.initials(), "TO");
        assert_eq!(p.public_repos, 8);
        let unsafe_urls =
            br#"{"login":"x","name":null,"html_url":"http://x","avatar_url":"javascript:1"}"#;
        assert!(parse(unsafe_urls).is_err());
    }
}
