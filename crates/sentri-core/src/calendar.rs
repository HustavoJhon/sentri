//! SENTRI's own calendar entries and the Google Calendar sync policy.
//!
//! Adding something to the calendar is not enrolling in it: entries only
//! record dates the user wants to see.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::profile::contains_term;
use crate::{Modality, Opportunity, Profile};

/// What a calendar entry represents. Each one is kept separate on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The event itself (conference, meetup, hackathon…).
    Event,
    /// Application or registration deadline.
    Deadline,
    CourseStart,
    CourseEnd,
    Reminder,
    /// Created by hand, not linked to an opportunity.
    Personal,
}

impl EntryKind {
    pub const ALL: [Self; 6] = [
        Self::Event,
        Self::Deadline,
        Self::CourseStart,
        Self::CourseEnd,
        Self::Reminder,
        Self::Personal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Deadline => "deadline",
            Self::CourseStart => "course_start",
            Self::CourseEnd => "course_end",
            Self::Reminder => "reminder",
            Self::Personal => "personal",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "Event",
            Self::Deadline => "Deadline",
            Self::CourseStart => "Course starts",
            Self::CourseEnd => "Course ends",
            Self::Reminder => "Reminder",
            Self::Personal => "Personal",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Event => "◷",
            Self::Deadline => "⏳",
            Self::CourseStart => "▶",
            Self::CourseEnd => "■",
            Self::Reminder => "⏰",
            Self::Personal => "•",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// An entry in SENTRI's calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: i64,
    pub opportunity_id: Option<i64>,
    pub kind: EntryKind,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub notes: Option<String>,
    /// Set once synchronized with Google Calendar.
    pub google_event_id: Option<String>,
}

/// The dates of an opportunity that can become calendar entries.
pub fn schedulable_dates(o: &Opportunity) -> Vec<(EntryKind, DateTime<Utc>)> {
    let learning = o.category.is_learning();
    let mut out = Vec::new();
    if let Some(start) = o.dates.starts_at {
        out.push((
            if learning {
                EntryKind::CourseStart
            } else {
                EntryKind::Event
            },
            start,
        ));
    }
    if learning && let Some(end) = o.dates.ends_at {
        out.push((EntryKind::CourseEnd, end));
    }
    if let Some(deadline) = o.dates.deadline {
        out.push((EntryKind::Deadline, deadline));
    }
    out
}

/// How an opportunity's dates should reach Google Calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPolicy {
    Automatic,
    Manual,
    /// No concrete date: nothing to create.
    Never,
}

/// Sync rules: in-person events in the home city and online events sync
/// automatically; international in-person events need manual confirmation;
/// items without dates never create events.
pub fn sync_policy(o: &Opportunity, profile: &Profile) -> SyncPolicy {
    if schedulable_dates(o).is_empty() {
        return SyncPolicy::Never;
    }
    let place = [o.location.as_deref(), o.country.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let in_home_city = profile
        .home_city
        .as_deref()
        .is_some_and(|c| contains_term(&place, &c.to_lowercase()));
    match o.modality {
        Modality::Online => SyncPolicy::Automatic,
        Modality::InPerson | Modality::Hybrid if in_home_city => SyncPolicy::Automatic,
        _ => SyncPolicy::Manual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalUrl, Category, SourceKind};
    use chrono::Duration;

    fn opp(category: Category) -> Opportunity {
        let url = CanonicalUrl::parse("https://x.example/").unwrap();
        let mut o = Opportunity::new(SourceKind::Manual, "m", url.as_str(), &url, "X", Utc::now());
        o.category = category;
        o
    }

    fn profile() -> Profile {
        Profile {
            home_city: Some("Cusco".into()),
            ..Profile::default()
        }
    }

    #[test]
    fn distinguishes_event_course_and_deadline_dates() {
        let now = Utc::now();
        let mut course = opp(Category::Course);
        course.dates.starts_at = Some(now);
        course.dates.ends_at = Some(now + Duration::days(30));
        course.dates.deadline = Some(now - Duration::days(1));
        let kinds: Vec<EntryKind> = schedulable_dates(&course)
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(
            kinds,
            [
                EntryKind::CourseStart,
                EntryKind::CourseEnd,
                EntryKind::Deadline
            ]
        );

        let mut meetup = opp(Category::Meetup);
        meetup.dates.starts_at = Some(now);
        assert_eq!(schedulable_dates(&meetup)[0].0, EntryKind::Event);
    }

    #[test]
    fn applies_sync_rules() {
        let mut o = opp(Category::Meetup);
        assert_eq!(sync_policy(&o, &profile()), SyncPolicy::Never, "no date");
        o.dates.starts_at = Some(Utc::now());
        o.modality = Modality::Online;
        assert_eq!(sync_policy(&o, &profile()), SyncPolicy::Automatic);
        o.modality = Modality::InPerson;
        o.location = Some("Cusco, Perú".into());
        assert_eq!(sync_policy(&o, &profile()), SyncPolicy::Automatic);
        o.location = Some("Berlin".into());
        assert_eq!(sync_policy(&o, &profile()), SyncPolicy::Manual);
    }
}
