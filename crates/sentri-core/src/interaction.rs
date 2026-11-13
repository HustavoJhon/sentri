//! User interactions with an opportunity. Every flag is independent: an item
//! can be liked and saved at the same time.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ParseError;

/// An independent boolean state the user can set on an opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    Liked,
    Saved,
    Dismissed,
    Interesting,
    Applied,
    Enrolled,
    Completed,
}

impl Flag {
    pub const ALL: [Self; 7] = [
        Self::Liked,
        Self::Saved,
        Self::Dismissed,
        Self::Interesting,
        Self::Applied,
        Self::Enrolled,
        Self::Completed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Liked => "liked",
            Self::Saved => "saved",
            Self::Dismissed => "dismissed",
            Self::Interesting => "interesting",
            Self::Applied => "applied",
            Self::Enrolled => "enrolled",
            Self::Completed => "completed",
        }
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Flag {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|f| f.as_str() == s.trim().to_ascii_lowercase())
            .ok_or_else(|| ParseError::new("flag", s))
    }
}

/// A user action. Recorded in the history log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action", content = "flag")]
pub enum Action {
    Set(Flag),
    Clear(Flag),
    /// Opened the detail view or the original link.
    Viewed,
    /// Corrected one or more fields manually.
    Edited,
    /// Added to SENTRI's calendar.
    Scheduled,
}

impl Action {
    /// Stable textual form used in the history table (`"set:liked"`, `"viewed"`).
    pub fn as_db(self) -> String {
        match self {
            Self::Set(f) => format!("set:{f}"),
            Self::Clear(f) => format!("clear:{f}"),
            Self::Viewed => "viewed".into(),
            Self::Edited => "edited".into(),
            Self::Scheduled => "scheduled".into(),
        }
    }

    pub fn from_db(s: &str) -> Result<Self, ParseError> {
        match s.split_once(':') {
            Some(("set", f)) => Ok(Self::Set(f.parse()?)),
            Some(("clear", f)) => Ok(Self::Clear(f.parse()?)),
            None if s == "viewed" => Ok(Self::Viewed),
            None if s == "edited" => Ok(Self::Edited),
            None if s == "scheduled" => Ok(Self::Scheduled),
            _ => Err(ParseError::new("action", s)),
        }
    }
}

/// Current interaction state of one opportunity.
// Each flag is an independent user choice, so plain bools are the clearest model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct InteractionState {
    pub liked: bool,
    pub saved: bool,
    pub dismissed: bool,
    pub interesting: bool,
    pub applied: bool,
    pub enrolled: bool,
    pub completed: bool,
    pub last_viewed_at: Option<DateTime<Utc>>,
}

impl InteractionState {
    pub const fn get(&self, flag: Flag) -> bool {
        match flag {
            Flag::Liked => self.liked,
            Flag::Saved => self.saved,
            Flag::Dismissed => self.dismissed,
            Flag::Interesting => self.interesting,
            Flag::Applied => self.applied,
            Flag::Enrolled => self.enrolled,
            Flag::Completed => self.completed,
        }
    }

    pub fn set(&mut self, flag: Flag, value: bool) {
        let slot = match flag {
            Flag::Liked => &mut self.liked,
            Flag::Saved => &mut self.saved,
            Flag::Dismissed => &mut self.dismissed,
            Flag::Interesting => &mut self.interesting,
            Flag::Applied => &mut self.applied,
            Flag::Enrolled => &mut self.enrolled,
            Flag::Completed => &mut self.completed,
        };
        *slot = value;
    }

    /// Applies an action and returns the resulting state.
    #[must_use]
    pub fn apply(mut self, action: Action, at: DateTime<Utc>) -> Self {
        match action {
            Action::Set(flag) => self.set(flag, true),
            Action::Clear(flag) => self.set(flag, false),
            Action::Viewed => self.last_viewed_at = Some(at),
            Action::Edited | Action::Scheduled => {}
        }
        self
    }

    /// The action that toggles `flag` from the current state.
    pub const fn toggle(&self, flag: Flag) -> Action {
        if self.get(flag) {
            Action::Clear(flag)
        } else {
            Action::Set(flag)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_independent() {
        let now = Utc::now();
        let s = InteractionState::default()
            .apply(Action::Set(Flag::Liked), now)
            .apply(Action::Set(Flag::Saved), now);
        assert!(s.liked && s.saved && !s.dismissed);
        let s = s.apply(Action::Clear(Flag::Liked), now);
        assert!(!s.liked && s.saved);
    }

    #[test]
    fn toggle_inverts() {
        let s = InteractionState::default();
        assert_eq!(s.toggle(Flag::Saved), Action::Set(Flag::Saved));
        let s = s.apply(Action::Set(Flag::Saved), Utc::now());
        assert_eq!(s.toggle(Flag::Saved), Action::Clear(Flag::Saved));
    }

    #[test]
    fn action_db_round_trip() {
        for flag in Flag::ALL {
            for a in [Action::Set(flag), Action::Clear(flag)] {
                assert_eq!(Action::from_db(&a.as_db()), Ok(a));
            }
        }
        for a in [Action::Viewed, Action::Edited, Action::Scheduled] {
            assert_eq!(Action::from_db(&a.as_db()), Ok(a));
        }
        assert!(Action::from_db("set:bogus").is_err());
    }
}
