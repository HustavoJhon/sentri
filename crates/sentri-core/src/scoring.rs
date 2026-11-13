//! Transparent, rule-based relevance scoring (see ADR-0003).
//!
//! The score is the sum of rule contributions. Every contribution carries a
//! human readable reason so the UI can explain recommendations. Scores only
//! affect ordering: nothing is ever hidden for being irrelevant.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::profile::contains_term;
use crate::{Category, Eligibility, InteractionState, Modality, Opportunity, Profile};

/// Points awarded by each rule. Configurable from `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weights {
    /// Per matching interest keyword, multiplied by the interest priority.
    pub interest_keyword: i32,
    /// When the category belongs to an interest, multiplied by priority.
    pub interest_category: i32,
    pub home_city: i32,
    pub home_country: i32,
    pub preferred_modality: i32,
    pub eligible: i32,
    pub not_eligible: i32,
    pub free: i32,
    pub certificate: i32,
    pub prizes: i32,
    pub travel: i32,
    pub deadline_soon: i32,
    pub deadline_passed: i32,
    pub avoid_keyword: i32,
    /// Points per unit of learned affinity.
    pub affinity_unit: i32,
    /// Maximum absolute contribution of learned affinity.
    pub affinity_cap: i32,
    /// Days before a deadline considered "soon".
    pub deadline_soon_days: i64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            interest_keyword: 6,
            interest_category: 4,
            home_city: 15,
            home_country: 8,
            preferred_modality: 5,
            eligible: 8,
            not_eligible: -30,
            free: 4,
            certificate: 4,
            prizes: 5,
            travel: 12,
            deadline_soon: 10,
            deadline_passed: -40,
            avoid_keyword: -20,
            affinity_unit: 3,
            affinity_cap: 24,
            deadline_soon_days: 14,
        }
    }
}

/// Preferences learned from past interactions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Affinity {
    categories: HashMap<Category, i32>,
    tags: HashMap<String, i32>,
    travel: i32,
}

impl Affinity {
    /// Signal strength of an interaction state (positive = wants more like it).
    fn signal(state: &InteractionState) -> i32 {
        let mut s = 0;
        if state.liked {
            s += 2;
        }
        if state.saved {
            s += 3;
        }
        if state.interesting {
            s += 1;
        }
        if state.applied || state.enrolled {
            s += 3;
        }
        if state.dismissed {
            s -= 3;
        }
        s
    }

    /// Builds affinity from opportunities the user interacted with.
    pub fn learn<'a>(
        items: impl IntoIterator<Item = (&'a Opportunity, &'a InteractionState)>,
    ) -> Self {
        let mut affinity = Self::default();
        for (opportunity, state) in items {
            let signal = Self::signal(state);
            if signal == 0 {
                continue;
            }
            *affinity.categories.entry(opportunity.category).or_default() += signal;
            for tag in &opportunity.tags {
                *affinity.tags.entry(tag.clone()).or_default() += signal;
            }
            if opportunity.perks.offers_travel() {
                affinity.travel += signal;
            }
        }
        affinity
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty() && self.tags.is_empty() && self.travel == 0
    }
}

/// One rule contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub points: i32,
    pub text: String,
}

/// Final score with its explanation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Relevance {
    pub score: i32,
    pub reasons: Vec<Reason>,
}

impl Relevance {
    fn add(&mut self, points: i32, text: impl Into<String>) {
        if points != 0 {
            self.score += points;
            self.reasons.push(Reason {
                points,
                text: text.into(),
            });
        }
    }
}

/// Everything the scorer needs besides the opportunity itself.
#[derive(Debug, Clone)]
pub struct Scorer<'a> {
    pub profile: &'a Profile,
    pub weights: &'a Weights,
    pub affinity: &'a Affinity,
    pub now: DateTime<Utc>,
}

impl Scorer<'_> {
    pub fn score(&self, o: &Opportunity) -> Relevance {
        let mut r = Relevance::default();
        let haystack = o.haystack();
        self.interests(o, &haystack, &mut r);
        self.location(o, &mut r);
        self.conditions(o, &mut r);
        self.deadline(o, &mut r);
        self.avoid(&haystack, &mut r);
        self.learned(o, &mut r);
        r.reasons.sort_by_key(|reason| -reason.points.abs());
        r
    }

    fn interests(&self, o: &Opportunity, haystack: &str, r: &mut Relevance) {
        let w = self.weights;
        for interest in &self.profile.interests {
            let mult = interest.priority.multiplier();
            let hits: Vec<&str> = interest
                .keywords
                .iter()
                .map(String::as_str)
                .filter(|k| contains_term(haystack, &k.to_lowercase()))
                .collect();
            if !hits.is_empty() {
                let n = i32::try_from(hits.len().min(3)).unwrap_or(3);
                r.add(
                    w.interest_keyword * mult * n,
                    format!("Matches interest '{}' ({})", interest.name, hits.join(", ")),
                );
            }
            if interest.categories.contains(&o.category) {
                r.add(
                    w.interest_category * mult,
                    format!("{} is relevant to '{}'", o.category, interest.name),
                );
            }
        }
    }

    fn location(&self, o: &Opportunity, r: &mut Relevance) {
        let w = self.weights;
        let place = [o.location.as_deref(), o.country.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let in_city = self
            .profile
            .home_city
            .as_deref()
            .is_some_and(|c| contains_term(&place, &c.to_lowercase()));
        if in_city {
            r.add(w.home_city, "Takes place in your city");
        } else if self
            .profile
            .location_terms()
            .iter()
            .any(|t| contains_term(&place, t))
        {
            r.add(w.home_country, "Takes place in your country");
        }
        if o.modality != Modality::Unknown
            && self.profile.preferred_modalities.contains(&o.modality)
        {
            r.add(
                w.preferred_modality,
                format!("{} matches your preferred modality", o.modality),
            );
        }
    }

    fn conditions(&self, o: &Opportunity, r: &mut Relevance) {
        let w = self.weights;
        match o.eligibility {
            Eligibility::Open => r.add(w.eligible, "Open to participants from your country"),
            Eligibility::NotEligible => r.add(w.not_eligible, "Not open to your country"),
            Eligibility::Unknown => {}
        }
        if o.cost.is_free() {
            r.add(w.free, "Free");
        }
        if o.perks.certificate == Some(true) {
            r.add(w.certificate, "Includes a certificate");
        }
        if o.perks.prizes.is_some() {
            r.add(w.prizes, "Offers prizes");
        }
        if o.perks.offers_travel() {
            r.add(w.travel, "Covers travel or accommodation");
        }
    }

    fn deadline(&self, o: &Opportunity, r: &mut Relevance) {
        let w = self.weights;
        let Some(deadline) = o.dates.deadline else {
            return;
        };
        if deadline < self.now {
            r.add(w.deadline_passed, "Deadline has passed");
        } else if deadline - self.now <= Duration::days(w.deadline_soon_days) {
            r.add(w.deadline_soon, "Deadline is coming up soon");
        }
    }

    fn avoid(&self, haystack: &str, r: &mut Relevance) {
        let hits: Vec<&str> = self
            .profile
            .avoid_keywords
            .iter()
            .map(String::as_str)
            .filter(|k| contains_term(haystack, &k.to_lowercase()))
            .collect();
        if !hits.is_empty() {
            r.add(
                self.weights.avoid_keyword,
                format!("Contains avoided terms ({})", hits.join(", ")),
            );
        }
    }

    fn learned(&self, o: &Opportunity, r: &mut Relevance) {
        let w = self.weights;
        let a = self.affinity;
        let mut units = a.categories.get(&o.category).copied().unwrap_or(0);
        let mut tag_units = 0;
        for tag in &o.tags {
            tag_units += a.tags.get(tag).copied().unwrap_or(0);
        }
        units += tag_units.clamp(-6, 6);
        if o.perks.offers_travel() {
            units += a.travel.clamp(-3, 3);
        }
        let points = (units * w.affinity_unit).clamp(-w.affinity_cap, w.affinity_cap);
        if points > 0 {
            r.add(points, "Similar to items you liked or saved");
        } else if points < 0 {
            r.add(points, "Similar to items you dismissed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Interest, Priority};
    use crate::{Action, CanonicalUrl, Flag, SourceKind};

    fn opp(title: &str, category: Category) -> Opportunity {
        let url = CanonicalUrl::parse(&format!("https://example.com/{}", title.len())).unwrap();
        let mut o = Opportunity::new(SourceKind::Manual, "m", url.as_str(), &url, title, now());
        o.category = category;
        o
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-13T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn profile() -> Profile {
        Profile {
            home_city: Some("Cusco".into()),
            country: Some("Peru".into()),
            location_aliases: vec!["Perú".into()],
            preferred_modalities: vec![Modality::Online],
            interests: vec![Interest {
                name: "AI".into(),
                keywords: vec!["ai".into(), "machine learning".into()],
                categories: vec![Category::Hackathon],
                priority: Priority::High,
            }],
            avoid_keywords: vec!["senior".into()],
        }
    }

    fn score(o: &Opportunity, affinity: &Affinity) -> Relevance {
        let p = profile();
        let w = Weights::default();
        Scorer {
            profile: &p,
            weights: &w,
            affinity,
            now: now(),
        }
        .score(o)
    }

    #[test]
    fn interest_match_is_explained() {
        let o = opp("AI Hackathon", Category::Hackathon);
        let r = score(&o, &Affinity::default());
        assert_eq!(r.score, 6 * 3 + 4 * 3);
        assert!(r.reasons.iter().any(|x| x.text.contains("'AI' (ai)")));
    }

    #[test]
    fn local_and_travel_rank_higher() {
        let mut local = opp("Meetup", Category::Meetup);
        local.location = Some("Cusco, Perú".into());
        let mut national = opp("Meetup", Category::Meetup);
        national.location = Some("Lima, Perú".into());
        let base = Affinity::default();
        assert_eq!(score(&local, &base).score, 15);
        assert_eq!(score(&national, &base).score, 8);

        let mut travel = opp("Fellowship", Category::Fellowship);
        travel.perks.travel_covered = Some(true);
        assert_eq!(score(&travel, &base).score, 12);
    }

    #[test]
    fn deadlines_and_avoided_terms() {
        let mut soon = opp("Call", Category::Call);
        soon.dates.deadline = Some(now() + Duration::days(3));
        assert_eq!(score(&soon, &Affinity::default()).score, 10);

        let mut passed = opp("Call", Category::Call);
        passed.dates.deadline = Some(now() - Duration::days(1));
        assert_eq!(score(&passed, &Affinity::default()).score, -40);

        let senior = opp("Senior Engineer", Category::Job);
        assert_eq!(score(&senior, &Affinity::default()).score, -20);
    }

    #[test]
    fn learns_from_interactions() {
        let mut liked = opp("Rust course", Category::Course);
        liked.tags = vec!["rust".into()];
        let saved = InteractionState::default().apply(Action::Set(Flag::Saved), now());
        let mut dismissed_job = opp("Job", Category::Job);
        dismissed_job.tags = vec!["php".into()];
        let dismissed = InteractionState::default().apply(Action::Set(Flag::Dismissed), now());

        let affinity = Affinity::learn([(&liked, &saved), (&dismissed_job, &dismissed)]);

        let mut candidate = opp("Another Rust course", Category::Course);
        candidate.tags = vec!["rust".into()];
        assert!(score(&candidate, &affinity).score > 0);

        let job = opp("PHP job", Category::Job);
        assert!(score(&job, &affinity).score < 0);
    }

    #[test]
    fn affinity_is_capped() {
        let mut o = opp("Course", Category::Course);
        o.tags = vec!["rust".into()];
        let saved = InteractionState::default()
            .apply(Action::Set(Flag::Saved), now())
            .apply(Action::Set(Flag::Liked), now());
        let items: Vec<_> = (0..50).map(|_| (o.clone(), saved)).collect();
        let affinity = Affinity::learn(items.iter().map(|(o, s)| (o, s)));
        assert_eq!(score(&o, &affinity).score, Weights::default().affinity_cap);
    }
}
