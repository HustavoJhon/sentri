//! Presentation preferences mirrored from `[cards]` and `[dashboard]` in the
//! configuration. Applied live when Settings are saved.

/// A piece of information a card can show under its title. Which ones
/// appear, and in which order, is configurable (`[cards] facts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fact {
    /// The main date: event start, publication, last update.
    When,
    Deadline,
    Pay,
    /// Organizer, company, provider, author or project owner.
    Organizer,
    Place,
    Modality,
    Cost,
    /// Certificate, prizes, travel and lodging.
    Perks,
    Topic,
    Skills,
    Links,
}

impl Fact {
    pub const ALL: [Self; 11] = [
        Self::When,
        Self::Deadline,
        Self::Pay,
        Self::Organizer,
        Self::Place,
        Self::Modality,
        Self::Cost,
        Self::Perks,
        Self::Topic,
        Self::Skills,
        Self::Links,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::When => "when",
            Self::Deadline => "deadline",
            Self::Pay => "pay",
            Self::Organizer => "organizer",
            Self::Place => "place",
            Self::Modality => "modality",
            Self::Cost => "cost",
            Self::Perks => "perks",
            Self::Topic => "topic",
            Self::Skills => "skills",
            Self::Links => "links",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// How dates read on cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateStyle {
    /// `Fri 20 Nov · in 3d`
    #[default]
    Both,
    /// `in 3d`
    Relative,
    /// `Fri 20 Nov`
    Absolute,
}

/// Card presentation preferences (`[cards]`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CardPrefs {
    pub show_source: bool,
    pub show_category: bool,
    pub show_meter: bool,
    /// Visible facts; also their display order when `custom_order`.
    pub facts: Vec<Fact>,
    pub custom_order: bool,
    pub dates: DateStyle,
    /// Shade the selected card as well as marking its rail.
    pub highlight: bool,
}

impl Default for CardPrefs {
    fn default() -> Self {
        Self::from_config(&sentri_app::CardsConfig::default())
    }
}

impl CardPrefs {
    pub fn from_config(c: &sentri_app::CardsConfig) -> Self {
        Self {
            show_source: c.show_source,
            show_category: c.show_category,
            show_meter: c.show_meter,
            facts: c.facts.iter().filter_map(|f| Fact::from_name(f)).collect(),
            custom_order: c.fact_order == "custom",
            dates: match c.dates.as_str() {
                "relative" => DateStyle::Relative,
                "absolute" => DateStyle::Absolute,
                _ => DateStyle::Both,
            },
            highlight: c.selection == "highlight",
        }
    }

    /// Visible facts, in the card's own order or the configured one.
    pub fn select<T>(&self, mut facts: Vec<(Fact, T)>) -> Vec<T> {
        facts.retain(|(f, _)| self.facts.contains(f));
        if self.custom_order {
            facts.sort_by_key(|(f, _)| self.facts.iter().position(|x| x == f));
        }
        facts.into_iter().map(|(_, value)| value).collect()
    }
}

/// A dashboard widget (`[dashboard] widgets`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widget {
    Metrics,
    Activity,
    Interactions,
    Distribution,
    Recommendations,
    Upcoming,
    News,
    Jobs,
}

impl Widget {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "metrics" => Self::Metrics,
            "activity" => Self::Activity,
            "interactions" => Self::Interactions,
            "distribution" => Self::Distribution,
            "recommendations" => Self::Recommendations,
            "upcoming" => Self::Upcoming,
            "news" => Self::News,
            "jobs" => Self::Jobs,
            _ => return None,
        })
    }

    /// Charts share a row; lists share another.
    pub const fn is_chart(self) -> bool {
        matches!(
            self,
            Self::Activity | Self::Interactions | Self::Distribution
        )
    }
}

/// Dashboard preferences (`[dashboard]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardPrefs {
    pub compact: bool,
    pub widgets: Vec<Widget>,
    /// Metric names, in order (see `sentri_app::config::DASHBOARD_METRICS`).
    pub metrics: Vec<String>,
    pub recommendations: usize,
}

impl Default for DashboardPrefs {
    fn default() -> Self {
        Self::from_config(&sentri_app::DashboardConfig::default())
    }
}

impl DashboardPrefs {
    pub fn from_config(c: &sentri_app::DashboardConfig) -> Self {
        Self {
            compact: c.layout == "compact",
            widgets: c
                .widgets
                .iter()
                .filter_map(|w| Widget::from_name(w))
                .collect(),
            metrics: c.metrics.clone(),
            recommendations: usize::from(c.recommendations),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_facts_are_dropped_and_custom_order_applies() {
        let mut prefs = CardPrefs {
            facts: vec![Fact::Place, Fact::Pay],
            ..CardPrefs::default()
        };
        let facts = vec![
            (Fact::Pay, "pay"),
            (Fact::When, "when"),
            (Fact::Place, "place"),
        ];
        assert_eq!(prefs.select(facts.clone()), ["pay", "place"], "kind order");
        prefs.custom_order = true;
        assert_eq!(prefs.select(facts), ["place", "pay"], "configured order");
    }

    #[test]
    fn defaults_show_everything() {
        let cards = CardPrefs::default();
        assert_eq!(cards.facts, Fact::ALL);
        assert!(cards.show_meter && cards.highlight && !cards.custom_order);
        let dash = DashboardPrefs::default();
        assert_eq!(dash.widgets.len(), 8);
        assert_eq!(dash.recommendations, 10);
    }
}
