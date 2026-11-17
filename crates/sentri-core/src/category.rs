use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// What kind of opportunity an item represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Event,
    Conference,
    Workshop,
    Meetup,
    Course,
    Training,
    Hackathon,
    Competition,
    Scholarship,
    Exchange,
    Fellowship,
    Volunteering,
    Call,
    Job,
    Internship,
    OpenSource,
    Community,
    CallForSpeakers,
    Other,
}

impl Category {
    pub const ALL: [Self; 19] = [
        Self::Event,
        Self::Conference,
        Self::Workshop,
        Self::Meetup,
        Self::Course,
        Self::Training,
        Self::Hackathon,
        Self::Competition,
        Self::Scholarship,
        Self::Exchange,
        Self::Fellowship,
        Self::Volunteering,
        Self::Call,
        Self::Job,
        Self::Internship,
        Self::OpenSource,
        Self::Community,
        Self::CallForSpeakers,
        Self::Other,
    ];

    /// Stable identifier used in storage and configuration.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Conference => "conference",
            Self::Workshop => "workshop",
            Self::Meetup => "meetup",
            Self::Course => "course",
            Self::Training => "training",
            Self::Hackathon => "hackathon",
            Self::Competition => "competition",
            Self::Scholarship => "scholarship",
            Self::Exchange => "exchange",
            Self::Fellowship => "fellowship",
            Self::Volunteering => "volunteering",
            Self::Call => "call",
            Self::Job => "job",
            Self::Internship => "internship",
            Self::OpenSource => "open_source",
            Self::Community => "community",
            Self::CallForSpeakers => "call_for_speakers",
            Self::Other => "other",
        }
    }

    /// Human readable label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Event => "Event",
            Self::Conference => "Conference",
            Self::Workshop => "Workshop",
            Self::Meetup => "Meetup",
            Self::Course => "Course",
            Self::Training => "Training",
            Self::Hackathon => "Hackathon",
            Self::Competition => "Competition",
            Self::Scholarship => "Scholarship",
            Self::Exchange => "Exchange",
            Self::Fellowship => "Fellowship",
            Self::Volunteering => "Volunteering",
            Self::Call => "Call",
            Self::Job => "Job",
            Self::Internship => "Internship",
            Self::OpenSource => "Open Source",
            Self::Community => "Community",
            Self::CallForSpeakers => "Call for Speakers",
            Self::Other => "Other",
        }
    }

    /// Whether items of this category usually happen at a concrete date.
    pub const fn is_dated_event(self) -> bool {
        matches!(
            self,
            Self::Event | Self::Conference | Self::Workshop | Self::Meetup | Self::Hackathon
        )
    }

    /// Whether this category belongs to the jobs section.
    pub const fn is_job(self) -> bool {
        matches!(self, Self::Job | Self::Internship)
    }

    /// Whether this category belongs to the learning section.
    pub const fn is_learning(self) -> bool {
        matches!(self, Self::Course | Self::Training)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for Category {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let needle = s.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|c| c.as_str() == needle)
            .ok_or_else(|| ParseError::new("category", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_form_matches_as_str() {
        for v in Category::ALL {
            let json = serde_json::to_string(&v).unwrap();
            assert_eq!(json, format!("\"{}\"", v.as_str()));
        }
    }

    #[test]
    fn round_trips_every_category() {
        for category in Category::ALL {
            assert_eq!(category.as_str().parse::<Category>(), Ok(category));
        }
    }

    #[test]
    fn parses_human_spellings() {
        assert_eq!("Open Source".parse(), Ok(Category::OpenSource));
        assert_eq!("open-source".parse(), Ok(Category::OpenSource));
        assert!("unknown".parse::<Category>().is_err());
    }
}
