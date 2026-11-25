//! Public GitHub profile used for SENTRI's personal identity (avatar,
//! name, link) and for open-source recommendations (the languages and
//! topics of public repositories). Only public data from
//! `GET /users/{username}` and `GET /users/{username}/repos`; no token.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use url::Url;

use sentri_core::{Category, Interest, Priority};
use sentri_sources::FetchError;

use crate::{AppError, Sentri, Thumbnail};

const API: &str = "https://api.github.com/";
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
    /// Changes whenever the profile (avatar included) is edited.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Public profile text, as the user wrote it (sanitized, one line).
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
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

    /// Cache key for the avatar: a new profile version (for example a new
    /// picture) downloads it again instead of reusing the old file.
    pub fn avatar_cache_key(&self) -> String {
        format!(
            "github-avatar:{}:{}:{}",
            self.login.to_ascii_lowercase(),
            self.avatar_url,
            self.updated_at.as_deref().unwrap_or("")
        )
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
    let line = |v: Option<String>, max: usize| {
        v.map(|t| sentri_core::text::clean(&t, max))
            .filter(|t| !t.is_empty())
    };
    profile.bio = line(profile.bio.take(), 160).filter(|b| !mentions_ip(b));
    // An IP address in these fields is never shown: the card is
    // often on screen in demos, screenshots and screen shares.
    profile.location = line(profile.location.take(), 60).filter(|l| !mentions_ip(l));
    profile.company = line(profile.company.take(), 60).filter(|c| !mentions_ip(c));
    if !profile.avatar_url.starts_with("https://") || !profile.html_url.starts_with("https://") {
        return Err(AppError::Profile("GitHub returned non-https URLs".into()));
    }
    Ok(profile)
}

/// Whether `text` contains something that parses as an IP address
/// (`203.0.113.7`, `2001:db8::1`, `[::1]:80`, `10.0.0.1:8080`).
pub fn mentions_ip(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '/' | '(' | ')'))
        .map(|word| word.trim_matches(|c: char| matches!(c, '[' | ']' | '.' | '"' | '\'')))
        .any(|word| {
            word.parse::<std::net::IpAddr>().is_ok()
                || word.parse::<std::net::SocketAddr>().is_ok()
                || word
                    .rsplit_once(':')
                    .is_some_and(|(host, _)| host.parse::<std::net::Ipv4Addr>().is_ok())
        })
}

/// Result of checking a username typed during onboarding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Found(Box<GitHubProfile>),
    /// GitHub answered that the user does not exist.
    NotFound,
    /// Not a valid GitHub username (nothing was requested).
    Invalid,
    /// Offline, rate limited or another error; the name can still be saved.
    Unavailable(String),
}

/// What public repositories say about the user's stack.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitHubSignals {
    /// Most used languages first.
    pub languages: Vec<String>,
    /// Most repeated repository topics first.
    pub topics: Vec<String>,
}

#[derive(Deserialize)]
struct Repo {
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    fork: bool,
}

impl GitHubSignals {
    const LANGUAGES: usize = 5;
    const TOPICS: usize = 8;

    /// Counts languages and topics of the user's own (non-fork) repos.
    pub fn from_repos(body: &[u8]) -> Result<Self, AppError> {
        let repos: Vec<Repo> = serde_json::from_slice(body)
            .map_err(|e| AppError::Profile(format!("unexpected GitHub response: {e}")))?;
        let mut languages: Vec<(String, usize)> = Vec::new();
        let mut topics: Vec<(String, usize)> = Vec::new();
        let bump = |list: &mut Vec<(String, usize)>, raw: &str| {
            let term = sentri_core::text::clean(raw, 40).to_lowercase();
            let safe = !term.is_empty()
                && term
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '+' | '#' | '.'));
            if !safe {
                return;
            }
            match list.iter_mut().find(|(t, _)| *t == term) {
                Some((_, n)) => *n += 1,
                None => list.push((term, 1)),
            }
        };
        for repo in repos.iter().filter(|r| !r.fork) {
            if let Some(language) = &repo.language {
                bump(&mut languages, language);
            }
            for topic in &repo.topics {
                bump(&mut topics, topic);
            }
        }
        let top = |mut list: Vec<(String, usize)>, n: usize| {
            // Stable: ties keep the most recently pushed first.
            list.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            list.into_iter().take(n).map(|(t, _)| t).collect()
        };
        Ok(Self {
            languages: top(languages, Self::LANGUAGES),
            topics: top(topics, Self::TOPICS),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.languages.is_empty() && self.topics.is_empty()
    }

    /// An interest used only for ranking (never written to the config):
    /// open-source items mentioning your languages or topics rank higher.
    pub fn interest(&self) -> Option<Interest> {
        if self.is_empty() {
            return None;
        }
        let shown: Vec<&str> = self.languages.iter().take(3).map(String::as_str).collect();
        let name = if shown.is_empty() {
            "your GitHub topics".to_owned()
        } else {
            format!("your GitHub languages: {}", shown.join(", "))
        };
        Some(Interest {
            name,
            keywords: self.languages.iter().chain(&self.topics).cloned().collect(),
            categories: vec![Category::OpenSource],
            priority: Priority::Medium,
        })
    }
}

fn is_fresh(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < TTL)
}

fn write_cache(path: &std::path::Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = std::fs::write(path, bytes) {
        tracing::debug!(%error, "could not cache GitHub data");
    }
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
        self.github_profile_for(username).await.map(Some)
    }

    /// The public profile of any username (onboarding checks one before it
    /// is saved), sharing the same cache.
    pub async fn github_profile_for(&self, username: &str) -> Result<GitHubProfile, AppError> {
        self.github_profile_at(API, username).await
    }

    async fn github_profile_at(
        &self,
        api: &str,
        username: &str,
    ) -> Result<GitHubProfile, AppError> {
        if !valid_username(username) {
            return Err(AppError::Profile(format!(
                "{username:?} is not a valid GitHub username"
            )));
        }
        let cache = self.profile_cache(username);
        if is_fresh(&cache)
            && let Ok(bytes) = std::fs::read(&cache)
            && let Ok(profile) = parse(&bytes)
        {
            return Ok(profile);
        }
        let body = self.github_get(api, &format!("users/{username}")).await?;
        let profile = parse(&body)?;
        write_cache(&cache, &body);
        Ok(profile)
    }

    /// Checks a username typed during onboarding. Never fails: a missing
    /// profile and an unreachable API are both answers the wizard shows.
    pub async fn lookup_github(&self, username: &str) -> Lookup {
        self.lookup_github_at(API, username).await
    }

    async fn lookup_github_at(&self, api: &str, username: &str) -> Lookup {
        if !valid_username(username) {
            return Lookup::Invalid;
        }
        match self.github_profile_at(api, username).await {
            Ok(profile) => Lookup::Found(Box::new(profile)),
            Err(AppError::Http(FetchError::Status(404))) => Lookup::NotFound,
            Err(e) => Lookup::Unavailable(e.to_string()),
        }
    }

    fn signals_cache(&self, username: &str) -> PathBuf {
        self.cache_dir().with_file_name(format!(
            "github-{}-signals.json",
            username.to_ascii_lowercase()
        ))
    }

    /// Languages and topics from the last download, whatever its age.
    /// Reads only the local cache, so ranking never waits for the network.
    pub fn cached_github_signals(&self) -> Option<GitHubSignals> {
        let config = self.config();
        let username = config
            .github
            .username
            .as_deref()
            .filter(|u| valid_username(u))?;
        let bytes = std::fs::read(self.signals_cache(username)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Downloads repository languages and topics when the cache is older
    /// than a day. Returns `Ok(None)` when no username is configured.
    pub async fn refresh_github_signals(&self) -> Result<Option<GitHubSignals>, AppError> {
        self.refresh_github_signals_at(API).await
    }

    async fn refresh_github_signals_at(
        &self,
        api: &str,
    ) -> Result<Option<GitHubSignals>, AppError> {
        let config = self.config();
        let Some(username) = config.github.username.as_deref().filter(|u| !u.is_empty()) else {
            return Ok(None);
        };
        if !valid_username(username) {
            return Err(AppError::Profile(format!(
                "{username:?} is not a valid GitHub username"
            )));
        }
        let cache = self.signals_cache(username);
        if is_fresh(&cache)
            && let Some(signals) = self.cached_github_signals()
        {
            return Ok(Some(signals));
        }
        let body = self
            .github_get(
                api,
                &format!("users/{username}/repos?type=owner&sort=pushed&per_page=30"),
            )
            .await?;
        let signals = GitHubSignals::from_repos(&body)?;
        if let Ok(json) = serde_json::to_vec(&signals) {
            write_cache(&cache, &json);
        }
        Ok(Some(signals))
    }

    async fn github_get(&self, api: &str, path: &str) -> Result<Vec<u8>, AppError> {
        let url = Url::parse(api)
            .and_then(|base| base.join(path))
            .map_err(|e| AppError::Profile(e.to_string()))?;
        let headers = [("accept", "application/vnd.github+json".to_owned())];
        Ok(self.http().get_with(&url, &headers).await?.body)
    }

    /// The avatar, requested at [`crate::image_source::REQUESTED_EDGE`]
    /// pixels (GitHub scales it on demand) and cached as downloaded. The
    /// interface resamples it to whatever size it draws.
    pub async fn github_avatar(&self, profile: &GitHubProfile) -> Result<Thumbnail, AppError> {
        Ok(self
            .image_cached(
                &profile.avatar_url,
                &profile.avatar_cache_key(),
                crate::images::SOURCE_EDGE,
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn app(dir: &std::path::Path, username: Option<&str>) -> Sentri {
        let mut config = crate::Config::parse("", std::path::Path::new("test")).unwrap();
        config.ui.cache_dir = Some(dir.join("images"));
        config.github.username = username.map(str::to_owned);
        let store = sentri_storage::Store::in_memory().await.unwrap();
        Sentri::with_store(config, store, true).unwrap()
    }

    const PROFILE: &str = r#"{"login":"octocat","name":"The Octocat","html_url":"https://github.com/octocat",
        "avatar_url":"https://avatars.githubusercontent.com/u/583231?v=4","public_repos":8}"#;

    #[tokio::test]
    async fn onboarding_lookup_handles_found_missing_invalid_and_offline() {
        let server = MockServer::start().await;
        Mock::given(path("/users/octocat"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(PROFILE, "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/users/ghost-user"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(path("/users/busy"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path(), None).await;
        let api = format!("{}/", server.uri());
        let Lookup::Found(p) = app.lookup_github_at(&api, "octocat").await else {
            panic!("expected a profile");
        };
        assert_eq!(p.public_repos, 8);
        // The second lookup is served from the shared cache (expect(1)).
        assert!(matches!(
            app.lookup_github_at(&api, "OctoCat").await,
            Lookup::Found(_)
        ));
        assert_eq!(
            app.lookup_github_at(&api, "ghost-user").await,
            Lookup::NotFound
        );
        assert_eq!(
            app.lookup_github_at(&api, "bad name").await,
            Lookup::Invalid
        );
        assert!(matches!(
            app.lookup_github_at(&api, "busy").await,
            Lookup::Unavailable(_)
        ));
    }

    #[tokio::test]
    async fn repo_signals_are_downloaded_once_and_read_from_cache() {
        let server = MockServer::start().await;
        let repos = r#"[
            {"language":"Rust","topics":["cli","tui"],"fork":false},
            {"language":"Rust","topics":["cli"],"fork":false},
            {"language":"Go","topics":[],"fork":false},
            {"language":"PHP","topics":["wordpress"],"fork":true},
            {"language":null}
        ]"#;
        Mock::given(path("/users/octocat/repos"))
            .and(query_param("per_page", "30"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(repos, "application/json"))
            .expect(1)
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path(), Some("octocat")).await;
        assert_eq!(app.cached_github_signals(), None);
        let api = format!("{}/", server.uri());
        let signals = app.refresh_github_signals_at(&api).await.unwrap().unwrap();
        assert_eq!(signals.languages, ["rust", "go"], "forks are ignored");
        assert_eq!(signals.topics, ["cli", "tui"]);
        assert_eq!(app.cached_github_signals(), Some(signals.clone()));
        let again = app.refresh_github_signals_at(&api).await.unwrap();
        assert_eq!(again, Some(signals));

        let nobody = self::app(dir.path(), None).await;
        assert_eq!(nobody.refresh_github_signals_at(&api).await.unwrap(), None);
        assert_eq!(nobody.cached_github_signals(), None);
    }

    #[test]
    fn signals_become_an_open_source_interest() {
        let empty = GitHubSignals::default();
        assert_eq!(empty.interest(), None);
        let signals = GitHubSignals::from_repos(
            br#"[{"language":"Rust","topics":["<script>","machine-learning"]}]"#,
        )
        .unwrap();
        assert_eq!(signals.topics, ["machine-learning"], "unsafe terms dropped");
        let interest = signals.interest().unwrap();
        assert_eq!(interest.name, "your GitHub languages: rust");
        assert_eq!(interest.categories, [Category::OpenSource]);
        assert!(interest.keywords.contains(&"machine-learning".to_owned()));
        assert!(GitHubSignals::from_repos(b"{}").is_err());
    }

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

    #[test]
    fn ip_addresses_are_not_shown_as_a_location() {
        for text in [
            "203.0.113.7",
            "Lima 10.0.0.1:8080",
            "[2001:db8::1]",
            "at 2001:db8::1",
        ] {
            assert!(mentions_ip(text), "{text}");
        }
        for text in ["Lima, Peru", "Cusco 2026", "v1.2.3", "Room 4:30"] {
            assert!(!mentions_ip(text), "{text}");
        }
        let body = br#"{"login":"octocat","html_url":"https://github.com/octocat",
            "avatar_url":"https://avatars.githubusercontent.com/u/1","location":"203.0.113.7"}"#;
        assert_eq!(parse(body).unwrap().location, None);
    }

    #[test]
    fn keeps_public_bio_location_and_company_as_safe_single_lines() {
        let body = br#"{"login":"octocat","html_url":"https://github.com/octocat",
            "avatar_url":"https://avatars.githubusercontent.com/u/1","bio":"Rust \u001b[31m dev\n from Cusco",
            "location":"  Lima, Peru ","company":"","blog":"https://x.example"}"#;
        let p = parse(body).unwrap();
        assert_eq!(p.bio.as_deref(), Some("Rust [31m dev from Cusco"));
        assert_eq!(p.location.as_deref(), Some("Lima, Peru"));
        assert_eq!(p.company, None, "empty fields are absent");
    }

    #[test]
    fn avatar_cache_follows_profile_changes() {
        let body = |updated: &str| {
            format!(
                r#"{{"login":"Octocat","name":null,"html_url":"https://github.com/octocat",
                "avatar_url":"https://avatars.githubusercontent.com/u/583231?v=4","updated_at":"{updated}"}}"#
            )
        };
        let old = parse(body("2026-01-01T00:00:00Z").as_bytes()).unwrap();
        let same = parse(body("2026-01-01T00:00:00Z").as_bytes()).unwrap();
        let new = parse(body("2026-11-19T00:00:00Z").as_bytes()).unwrap();
        assert_eq!(old.avatar_cache_key(), same.avatar_cache_key());
        assert_ne!(old.avatar_cache_key(), new.avatar_cache_key());
        assert!(old.avatar_cache_key().contains("octocat"));
        let first = crate::image_source::candidates(&old.avatar_url);
        assert_eq!(
            first[0].query(),
            Some("v=4&s=460"),
            "avatar requested at 460 px, not GitHub's default thumbnail"
        );
    }
}
