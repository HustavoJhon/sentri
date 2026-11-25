//! Discovery preferences chosen during onboarding: which areas interest the
//! user and how far from home opportunities may be.
//!
//! Topics are groups of existing [`Category`] values (plus news); they add
//! no new classification of their own.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Category, ContentKind, Opportunity, ParseError};

/// An area of interest offered by onboarding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Events,
    Courses,
    Hackathons,
    Scholarships,
    Competitions,
    Communities,
    OpenSource,
    Cfps,
    News,
    Jobs,
}

impl Topic {
    pub const ALL: [Self; 10] = [
        Self::Events,
        Self::Courses,
        Self::Hackathons,
        Self::Scholarships,
        Self::Competitions,
        Self::Communities,
        Self::OpenSource,
        Self::Cfps,
        Self::News,
        Self::Jobs,
    ];

    /// Stable identifier used in configuration.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Courses => "courses",
            Self::Hackathons => "hackathons",
            Self::Scholarships => "scholarships",
            Self::Competitions => "competitions",
            Self::Communities => "communities",
            Self::OpenSource => "open_source",
            Self::Cfps => "cfps",
            Self::News => "news",
            Self::Jobs => "jobs",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Events => "Events",
            Self::Courses => "Courses",
            Self::Hackathons => "Hackathons",
            Self::Scholarships => "Scholarships",
            Self::Competitions => "Competitions",
            Self::Communities => "Communities",
            Self::OpenSource => "Open Source",
            Self::Cfps => "CFPs",
            Self::News => "Tech News",
            Self::Jobs => "Jobs and internships",
        }
    }

    /// Existing categories this topic covers (news is a content kind).
    pub const fn categories(self) -> &'static [Category] {
        match self {
            Self::Events => &[
                Category::Event,
                Category::Conference,
                Category::Workshop,
                Category::Meetup,
            ],
            Self::Courses => &[Category::Course, Category::Training],
            Self::Hackathons => &[Category::Hackathon],
            Self::Scholarships => &[
                Category::Scholarship,
                Category::Exchange,
                Category::Fellowship,
            ],
            Self::Competitions => &[Category::Competition, Category::Call],
            Self::Communities => &[Category::Community, Category::Meetup],
            Self::OpenSource => &[Category::OpenSource],
            Self::Cfps => &[Category::CallForSpeakers],
            Self::News => &[],
            Self::Jobs => &[Category::Job, Category::Internship],
        }
    }

    pub fn matches(self, o: &Opportunity) -> bool {
        match self {
            Self::News => o.content == ContentKind::News,
            _ => o.content != ContentKind::News && self.categories().contains(&o.category),
        }
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Topic {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let needle = s.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == needle)
            .ok_or_else(|| ParseError::new("topic", s))
    }
}

/// `[discovery]`: how far opportunities may be and which topics matter.
///
/// Scope only adds or withholds bonus points; nothing is ever hidden. Remote,
/// hybrid and in-person preferences live in `profile.preferred_modalities`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Discovery {
    /// Boost opportunities in your city.
    pub local: bool,
    /// Boost opportunities elsewhere in your country.
    pub national: bool,
    /// Boost opportunities abroad that are open to you.
    pub international: bool,
    /// Topics chosen during onboarding. Empty means no preference.
    pub topics: Vec<Topic>,
}

impl Default for Discovery {
    fn default() -> Self {
        Self {
            local: true,
            national: true,
            international: true,
            topics: Vec::new(),
        }
    }
}

impl Discovery {
    /// The first chosen topic that covers `o`.
    pub fn topic_for(&self, o: &Opportunity) -> Option<Topic> {
        self.topics.iter().copied().find(|t| t.matches(o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalUrl, SourceKind};

    fn opp(category: Category, content: ContentKind) -> Opportunity {
        let url = CanonicalUrl::parse("https://example.com/x").unwrap();
        let mut o = Opportunity::new(
            SourceKind::Manual,
            "m",
            url.as_str(),
            &url,
            "x",
            chrono::Utc::now(),
        );
        o.category = category;
        o.content = content;
        o
    }

    #[test]
    fn topics_round_trip_and_cover_existing_categories() {
        for topic in Topic::ALL {
            assert_eq!(topic.as_str().parse::<Topic>().unwrap(), topic);
        }
        assert_eq!("Open Source".parse::<Topic>().unwrap(), Topic::OpenSource);
        assert!("nope".parse::<Topic>().is_err());
        let covered: Vec<Category> = Topic::ALL
            .iter()
            .flat_map(|t| t.categories().iter().copied())
            .collect();
        for c in Category::ALL {
            if !matches!(c, Category::Volunteering | Category::Other) {
                assert!(covered.contains(&c), "{c:?} has no topic");
            }
        }
    }

    #[test]
    fn news_is_matched_by_content_not_category() {
        let news = opp(Category::Course, ContentKind::News);
        assert!(Topic::News.matches(&news));
        assert!(!Topic::Courses.matches(&news));
        let course = opp(Category::Course, ContentKind::Opportunity);
        assert!(Topic::Courses.matches(&course));
        assert!(!Topic::News.matches(&course));
    }

    #[test]
    fn defaults_keep_every_scope_and_no_topics() {
        let d: Discovery = serde_json::from_str("{}").unwrap();
        assert!(d.local && d.national && d.international);
        let d = Discovery {
            topics: vec![Topic::Jobs, Topic::Hackathons],
            ..Discovery::default()
        };
        let hack = opp(Category::Hackathon, ContentKind::Opportunity);
        assert_eq!(d.topic_for(&hack), Some(Topic::Hackathons));
        assert_eq!(Discovery::default().topic_for(&hack), None);
    }
}
