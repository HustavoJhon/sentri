use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// Well-known Mastodon instances. Others are detected from the
/// `/@user/<id>` path shape by [`SourceKind::from_url_parts`].
const MASTODON_HOSTS: &[&str] = &[
    "mastodon.social",
    "fosstodon.org",
    "hachyderm.io",
    "mas.to",
    "infosec.exchange",
    "mstdn.social",
];

/// Platform or mechanism an opportunity was obtained from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Manual,
    Rss,
    #[serde(rename = "ical")]
    ICal,
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "hackernews")]
    HackerNews,
    Reddit,
    #[serde(rename = "linkedin")]
    LinkedIn,
    X,
    Luma,
    Gdg,
    Devpost,
    Meetup,
    Eventbrite,
    Kaggle,
    #[serde(rename = "mslearn")]
    MicrosoftLearn,
    Instagram,
    Threads,
    Facebook,
    #[serde(rename = "tiktok")]
    TikTok,
    #[serde(rename = "youtube")]
    YouTube,
    Bluesky,
    Mastodon,
    #[serde(rename = "googlenews")]
    GoogleNews,
    Web,
    Peruanos,
    Hack0,
}

impl SourceKind {
    pub const ALL: [Self; 26] = [
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
        Self::Instagram,
        Self::Threads,
        Self::Facebook,
        Self::TikTok,
        Self::YouTube,
        Self::Bluesky,
        Self::Mastodon,
        Self::GoogleNews,
        Self::Web,
        Self::Peruanos,
        Self::Hack0,
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
            Self::Instagram => "instagram",
            Self::Threads => "threads",
            Self::Facebook => "facebook",
            Self::TikTok => "tiktok",
            Self::YouTube => "youtube",
            Self::Bluesky => "bluesky",
            Self::Mastodon => "mastodon",
            Self::GoogleNews => "googlenews",
            Self::Web => "web",
            Self::Peruanos => "peruanos",
            Self::Hack0 => "hack0",
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
            Self::Instagram => "Instagram",
            Self::Threads => "Threads",
            Self::Facebook => "Facebook",
            Self::TikTok => "TikTok",
            Self::YouTube => "YouTube",
            Self::Bluesky => "Bluesky",
            Self::Mastodon => "Mastodon",
            Self::GoogleNews => "Google News",
            Self::Web => "Web",
            Self::Peruanos => "Peruanos.dev",
            Self::Hack0 => "hack0.dev",
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
            Self::Instagram => "IG",
            Self::Threads => "TH",
            Self::Facebook => "FB",
            Self::TikTok => "TT",
            Self::YouTube => "YT",
            Self::Bluesky => "BS",
            Self::Mastodon => "MA",
            Self::GoogleNews => "GN",
            Self::Web => "WW",
            Self::Peruanos => "PE",
            Self::Hack0 => "H0",
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
        } else if matches("instagram.com") || matches("instagr.am") {
            Self::Instagram
        } else if matches("threads.net") || matches("threads.com") {
            Self::Threads
        } else if matches("facebook.com") || matches("fb.com") || matches("fb.me") {
            Self::Facebook
        } else if matches("tiktok.com") {
            Self::TikTok
        } else if matches("youtube.com") || matches("youtu.be") {
            Self::YouTube
        } else if matches("bsky.app") || matches("bsky.social") {
            Self::Bluesky
        } else if MASTODON_HOSTS.iter().any(|h| matches(h)) {
            Self::Mastodon
        } else if matches("news.google.com") {
            Self::GoogleNews
        } else if matches("peruanos.dev") {
            Self::Peruanos
        } else if matches("hack0.dev") {
            Self::Hack0
        } else {
            return None;
        };
        Some(kind)
    }
}

impl SourceKind {
    /// Detection from host and path; recognizes Mastodon posts on any
    /// instance (`/@user/123…`).
    pub fn from_url_parts(host: &str, path: &str) -> Option<Self> {
        Self::from_host(host).or_else(|| {
            let mut parts = path.trim_matches('/').split('/');
            let user = parts.next()?;
            let id = parts.next()?;
            (user.starts_with('@') && !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
                .then_some(Self::Mastodon)
        })
    }

    /// Social networks whose posts can only be added manually (no permitted
    /// automatic access for personal use, see docs/sources.md).
    pub const fn is_manual_only(self) -> bool {
        matches!(
            self,
            Self::Reddit
                | Self::LinkedIn
                | Self::X
                | Self::Instagram
                | Self::Threads
                | Self::Facebook
                | Self::TikTok
        )
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
        assert_eq!(
            SourceKind::from_host("www.instagram.com"),
            Some(SourceKind::Instagram)
        );
        assert_eq!(SourceKind::from_host("youtu.be"), Some(SourceKind::YouTube));
        assert_eq!(
            SourceKind::from_url_parts("social.example", "/@ana/1129384"),
            Some(SourceKind::Mastodon)
        );
        assert_eq!(
            SourceKind::from_url_parts("blog.example", "/@ana/post"),
            None
        );
        assert_eq!(SourceKind::from_host("notgithub.com"), None);
    }

    #[test]
    fn round_trips_and_badges_are_short() {
        for kind in SourceKind::ALL {
            assert_eq!(kind.as_str().parse(), Ok(kind));
            assert!(kind.badge().len() <= 3);
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
        }
    }
}
