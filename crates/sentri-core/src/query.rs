//! Feed views and filters, independent of how they are executed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Category, Flag, Modality, SourceKind};

/// A named slice of the feed. Category views include every non-dismissed item
/// of those categories; flag views include items with that flag set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedView {
    #[default]
    Discover,
    News,
    /// Opportunities and news together (global search).
    All,
    Jobs,
    Courses,
    Events,
    Hackathons,
    Scholarships,
    Volunteering,
    OpenSource,
    Communities,
    CallForSpeakers,
    Travel,
    Liked,
    Saved,
    Interesting,
    Applied,
    Enrolled,
    Completed,
    Dismissed,
    History,
}

impl FeedView {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Discover => "Discover",
            Self::News => "Tech News",
            Self::All => "Everything",
            Self::Jobs => "Jobs",
            Self::Courses => "Courses",
            Self::Events => "Events",
            Self::Hackathons => "Hackathons",
            Self::Scholarships => "Scholarships",
            Self::Volunteering => "Volunteering",
            Self::OpenSource => "Open Source",
            Self::Communities => "Communities",
            Self::CallForSpeakers => "Call for Speakers",
            Self::Travel => "Travel",
            Self::Liked => "Likes",
            Self::Saved => "Saved",
            Self::Interesting => "Interesting",
            Self::Applied => "Applications",
            Self::Enrolled => "Enrollments",
            Self::Completed => "Completed",
            Self::Dismissed => "Dismissed",
            Self::History => "History",
        }
    }

    /// Categories shown by this view, or `None` for "any category".
    pub const fn categories(self) -> Option<&'static [Category]> {
        Some(match self {
            Self::Jobs => &[Category::Job, Category::Internship],
            Self::Courses => &[Category::Course, Category::Training],
            Self::Events => &[
                Category::Event,
                Category::Conference,
                Category::Workshop,
                Category::Meetup,
            ],
            Self::Hackathons => &[Category::Hackathon, Category::Competition],
            Self::Scholarships => &[
                Category::Scholarship,
                Category::Fellowship,
                Category::Exchange,
            ],
            Self::Volunteering => &[Category::Volunteering],
            Self::OpenSource => &[Category::OpenSource],
            Self::Communities => &[Category::Community],
            Self::CallForSpeakers => &[Category::CallForSpeakers],
            _ => return None,
        })
    }

    /// Flag that must be set for an item to appear, if any.
    pub const fn required_flag(self) -> Option<Flag> {
        match self {
            Self::Liked => Some(Flag::Liked),
            Self::Saved => Some(Flag::Saved),
            Self::Interesting => Some(Flag::Interesting),
            Self::Applied => Some(Flag::Applied),
            Self::Enrolled => Some(Flag::Enrolled),
            Self::Completed => Some(Flag::Completed),
            Self::Dismissed => Some(Flag::Dismissed),
            _ => None,
        }
    }

    /// Content shown: `Some(News)` for Tech News, `Some(Opportunity)` for
    /// discovery views, `None` (both) for personal lists.
    pub const fn content(self) -> Option<crate::ContentKind> {
        match self {
            Self::News => Some(crate::ContentKind::News),
            Self::Discover
            | Self::Jobs
            | Self::Courses
            | Self::Events
            | Self::Hackathons
            | Self::Scholarships
            | Self::Volunteering
            | Self::OpenSource
            | Self::Communities
            | Self::CallForSpeakers
            | Self::Travel => Some(crate::ContentKind::Opportunity),
            _ => None,
        }
    }

    /// Whether dismissed items are hidden. Explicit flag views show everything
    /// the user marked, even if it was later dismissed.
    pub const fn hides_dismissed(self) -> bool {
        self.required_flag().is_none() && !matches!(self, Self::History)
    }
}

/// Price filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceFilter {
    Free,
    Paid,
}

/// Sort order of the feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Highest relevance first.
    #[default]
    Relevance,
    /// Most recently discovered first.
    Newest,
    /// Closest upcoming deadline first.
    Deadline,
}

/// Every filter the feed supports. `None`/empty means "no restriction".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
// Boolean filters are independent toggles exposed directly in the UI.
#[allow(clippy::struct_excessive_bools)]
pub struct FeedQuery {
    pub view: FeedView,
    /// Free text; every word must appear in title, summary, tags or location.
    pub text: Option<String>,
    pub sources: Vec<SourceKind>,
    pub categories: Vec<Category>,
    pub modalities: Vec<Modality>,
    pub location: Option<String>,
    pub price: Option<PriceFilter>,
    pub with_certificate: bool,
    pub with_prizes: bool,
    pub with_travel: bool,
    pub open_to_country: bool,
    pub deadline_before: Option<DateTime<Utc>>,
    /// Hide items whose deadline already passed.
    pub only_upcoming: bool,
    /// Hide items whose date has not passed yet.
    pub only_past: bool,
    pub min_score: Option<i32>,
    pub sort: SortOrder,
    pub limit: Option<u32>,
}

impl FeedQuery {
    pub fn view(view: FeedView) -> Self {
        Self {
            view,
            ..Self::default()
        }
    }

    /// Lowercased search words.
    pub fn words(&self) -> Vec<String> {
        self.text
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_map_to_filters() {
        assert_eq!(FeedView::Saved.required_flag(), Some(Flag::Saved));
        assert!(!FeedView::Saved.hides_dismissed());
        assert!(FeedView::Discover.hides_dismissed());
        assert!(
            FeedView::Jobs
                .categories()
                .unwrap()
                .contains(&Category::Internship)
        );
        assert_eq!(
            FeedView::Communities.categories(),
            Some(&[Category::Community][..])
        );
        assert_eq!(
            FeedView::CallForSpeakers.categories(),
            Some(&[Category::CallForSpeakers][..])
        );
        assert_eq!(FeedView::Discover.categories(), None);
    }

    #[test]
    fn splits_search_words() {
        let q = FeedQuery {
            text: Some("  Rust  AI ".into()),
            ..FeedQuery::default()
        };
        assert_eq!(q.words(), vec!["rust", "ai"]);
    }
}
