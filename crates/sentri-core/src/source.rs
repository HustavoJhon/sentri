use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// Platform or mechanism an opportunity was obtained from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Manual,
    Rss,
    ICal,
    GitHub,
    HackerNews,
    Reddit,
    LinkedIn,
    X,
    Luma,
    Gdg,
    Devpost,
    Meetup,
    Eventbrite,
    Kaggle,
    MicrosoftLearn,
    Web,
}

impl SourceKind {
    pub const ALL: [Self; 16] = [
        Self::Manual,
        Self::Rss,
        Self::ICal,
        Self::GitHub,
        Self::HackerNews,
        Self::Reddit,
        Self::LinkedIn,
        Self::X,
        Self::Luma,
        Self::Gdg,
        Self::Devpost,
        Self::Meetup,
        Self::Eventbrite,
        Self::Kaggle,
        Self::MicrosoftLearn,
        Self::Web,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Rss => "rss",
            Self::ICal => "ical",
            Self::GitHub => "github",
            Self::HackerNews => "hackernews",
            Self::Reddit => "reddit",
            Self::LinkedIn => "linkedin",
            Self::X => "x",
            Self::Luma => "luma",
            Self::Gdg => "gdg",
            Self::Devpost => "devpost",
            Self::Meetup => "meetup",
            Self::Eventbrite => "eventbrite",
            Self::Kaggle => "kaggle",
            Self::MicrosoftLearn => "mslearn",
            Self::Web => "web",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Rss => "RSS",
            Self::ICal => "iCal",
            Self::GitHub => "GitHub",
            Self::HackerNews => "Hacker News",
            Self::Reddit => "Reddit",
            Self::LinkedIn => "LinkedIn",
            Self::X => "X",
            Self::Luma => "Luma",
            Self::Gdg => "GDG",
            Self::Devpost => "Devpost",
            Self::Meetup => "Meetup",
            Self::Eventbrite => "Eventbrite",
            Self::Kaggle => "Kaggle",
            Self::MicrosoftLearn => "Microsoft Learn",
            Self::Web => "Web",
        }
    }

    /// Short ASCII badge that identifies the platform in any terminal (no Nerd Fonts).
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Manual => "++",
            Self::Rss => "RS",
            Self::ICal => "IC",
            Self::GitHub => "GH",
            Self::HackerNews => "HN",
            Self::Reddit => "r/",
            Self::LinkedIn => "in",
            Self::X => "X",
            Self::Luma => "LU",
            Self::Gdg => "GDG",
            Self::Devpost => "DP",
            Self::Meetup => "MU",
            Self::Eventbrite => "EB",
            Self::Kaggle => "KG",
            Self::MicrosoftLearn => "MS",
            Self::Web => "WW",
        }
    }

    /// Best-effort detection of the platform from a URL host.
    pub fn from_host(host: &str) -> Option<Self> {
        let host = host.trim_start_matches("www.").to_ascii_lowercase();
        let matches = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
        let kind = if matches("reddit.com") || matches("redd.it") {
            Self::Reddit
        } else if matches("linkedin.com") || matches("lnkd.in") {
            Self::LinkedIn
        } else if matches("x.com") || matches("twitter.com") {
            Self::X
        } else if matches("github.com") {
            Self::GitHub
        } else if matches("news.ycombinator.com") {
            Self::HackerNews
        } else if matches("lu.ma") || matches("luma.com") {
            Self::Luma
        } else if matches("gdg.community.dev") {
            Self::Gdg
        } else if matches("devpost.com") {
            Self::Devpost
        } else if matches("meetup.com") {
            Self::Meetup
        } else if matches("eventbrite.com") || host.starts_with("eventbrite.") {
            Self::Eventbrite
        } else if matches("kaggle.com") {
            Self::Kaggle
        } else if matches("learn.microsoft.com") {
            Self::MicrosoftLearn
        } else {
            return None;
        };
        Some(kind)
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for SourceKind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let needle = s.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|k| k.as_str() == needle)
            .ok_or_else(|| ParseError::new("source kind", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_platform_from_host() {
        assert_eq!(
            SourceKind::from_host("www.reddit.com"),
            Some(SourceKind::Reddit)
        );
        assert_eq!(
            SourceKind::from_host("old.reddit.com"),
            Some(SourceKind::Reddit)
        );
        assert_eq!(
            SourceKind::from_host("pe.linkedin.com"),
            Some(SourceKind::LinkedIn)
        );
        assert_eq!(SourceKind::from_host("lu.ma"), Some(SourceKind::Luma));
        assert_eq!(SourceKind::from_host("notgithub.com"), None);
    }

    #[test]
    fn round_trips_and_badges_are_short() {
        for kind in SourceKind::ALL {
            assert_eq!(kind.as_str().parse(), Ok(kind));
            assert!(kind.badge().len() <= 3);
        }
    }
}
