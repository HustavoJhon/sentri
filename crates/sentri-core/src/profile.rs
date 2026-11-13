//! The user's interest profile. Loaded from configuration, never hard-coded.

use serde::{Deserialize, Serialize};

use crate::{Category, Modality};

/// How much an interest matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
}

impl Priority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    pub const fn multiplier(self) -> i32 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }
}

/// One area of interest, e.g. "Artificial intelligence".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Interest {
    pub name: String,
    /// Terms matched as whole words against title, summary, tags and location.
    pub keywords: Vec<String>,
    /// Categories that are relevant for this interest.
    pub categories: Vec<Category>,
    pub priority: Priority,
}

/// Personal profile used for relevance and eligibility.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// City where in-person attendance is easy (e.g. "Cusco").
    pub home_city: Option<String>,
    /// Country of participation (e.g. "Peru"), used for eligibility.
    pub country: Option<String>,
    /// Extra names that refer to the home city or country (e.g. "Perú").
    pub location_aliases: Vec<String>,
    pub preferred_modalities: Vec<Modality>,
    pub interests: Vec<Interest>,
    /// Terms that make an item less relevant (never hidden).
    pub avoid_keywords: Vec<String>,
}

impl Profile {
    /// All the names that identify the user's location, lowercased.
    pub fn location_terms(&self) -> Vec<String> {
        self.home_city
            .iter()
            .chain(self.country.iter())
            .chain(self.location_aliases.iter())
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Whether `term` appears in `haystack` as a whole word or phrase.
///
/// Both inputs are expected lowercased. "ai" matches "ai hackathon" but not
/// "said".
pub fn contains_term(haystack: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    haystack.match_indices(term).any(|(start, _)| {
        let end = start + term.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        before_ok && after_ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_whole_words_only() {
        assert!(contains_term("ai hackathon in cusco", "ai"));
        assert!(contains_term("learn rust.", "rust"));
        assert!(contains_term(
            "machine learning bootcamp",
            "machine learning"
        ));
        assert!(!contains_term("he said hello", "ai"));
        assert!(!contains_term("trust", "rust"));
        assert!(!contains_term("anything", ""));
    }

    #[test]
    fn profile_deserializes_with_defaults() {
        let p: Profile = serde_json::from_str(r#"{"home_city":"Cusco"}"#).unwrap();
        assert_eq!(p.location_terms(), vec!["cusco"]);
        assert_eq!(p.interests, Vec::<Interest>::new());
    }
}
