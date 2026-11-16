//! History section: activity heatmap, statistics and day summaries.
//!
//! Every number is derived from persisted, deduplicated activity
//! (`Sentri::activity`); nothing is estimated.

use std::collections::BTreeMap;

use chrono::{Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use sentri_app::{DayActivity, DayEntry};

/// Days shown in the heatmap (a year, aligned to whole weeks when drawn).
pub const DAYS: i64 = 371;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryStats {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub most_active_day: Option<(NaiveDate, i64)>,
    /// Monday of the most active week and its total.
    pub most_active_week: Option<(NaiveDate, i64)>,
    pub active_days: u32,
    pub reviewed: i64,
    pub saved: i64,
    pub liked: i64,
    pub applied: i64,
    pub enrolled: i64,
    pub edited: i64,
    pub scheduled: i64,
    pub added: i64,
}

fn week_start(d: NaiveDate) -> NaiveDate {
    use chrono::Datelike;
    d - Duration::days(i64::from(d.weekday().num_days_from_monday()))
}

/// Computes statistics from activity per day.
pub fn stats(days: &BTreeMap<NaiveDate, DayActivity>, today: NaiveDate) -> HistoryStats {
    let mut s = HistoryStats::default();
    let action = |a: &DayActivity, name: &str| a.actions.get(name).copied().unwrap_or(0);
    let mut weeks: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for (date, a) in days {
        let total = a.total();
        if total > 0 {
            s.active_days += 1;
        }
        if total > s.most_active_day.map_or(0, |(_, n)| n) {
            s.most_active_day = Some((*date, total));
        }
        *weeks.entry(week_start(*date)).or_default() += total;
        s.reviewed += action(a, "viewed");
        s.saved += action(a, "saved");
        s.liked += action(a, "liked");
        s.applied += action(a, "applied");
        s.enrolled += action(a, "enrolled");
        s.edited += action(a, "edited");
        s.scheduled += action(a, "scheduled");
        s.added += a.added_manually;
    }
    s.most_active_week = weeks
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(week, n)| (*n, std::cmp::Reverse(*week)));

    let active = |d: NaiveDate| days.get(&d).is_some_and(|a| a.total() > 0);
    // The current streak may end yesterday if today has no activity yet.
    let mut cursor = if active(today) {
        today
    } else {
        today - Duration::days(1)
    };
    while active(cursor) {
        s.current_streak += 1;
        cursor -= Duration::days(1);
    }
    let mut run = 0;
    let mut previous: Option<NaiveDate> = None;
    for (date, a) in days {
        if a.total() == 0 {
            continue;
        }
        run = match previous {
            Some(p) if *date - p == Duration::days(1) => run + 1,
            _ => 1,
        };
        s.longest_streak = s.longest_streak.max(run);
        previous = Some(*date);
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryState {
    pub today: NaiveDate,
    pub cursor: NaiveDate,
    pub days: BTreeMap<NaiveDate, DayActivity>,
    pub stats: HistoryStats,
    pub entries: Option<(NaiveDate, Vec<DayEntry>)>,
    pub loaded: bool,
}

impl Default for HistoryState {
    fn default() -> Self {
        let today = Local::now().date_naive();
        Self {
            today,
            cursor: today,
            days: BTreeMap::new(),
            stats: HistoryStats::default(),
            entries: None,
            loaded: false,
        }
    }
}

/// Requests from the History section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryEffect {
    LoadActivity,
    LoadDay(NaiveDate),
}

impl HistoryState {
    pub fn set_activity(&mut self, days: BTreeMap<NaiveDate, DayActivity>) {
        self.stats = stats(&days, self.today);
        self.days = days;
        self.loaded = true;
    }

    pub fn day(&self, date: NaiveDate) -> i64 {
        self.days.get(&date).map_or(0, DayActivity::total)
    }

    /// Highest day total, for color scaling.
    pub fn max(&self) -> i64 {
        self.days
            .values()
            .map(DayActivity::total)
            .max()
            .unwrap_or(0)
    }

    pub fn entries_for_cursor(&self) -> Option<&[DayEntry]> {
        self.entries
            .as_ref()
            .filter(|(d, _)| *d == self.cursor)
            .map(|(_, e)| e.as_slice())
    }

    /// Effects needed when the section opens.
    pub fn open(&mut self) -> Vec<HistoryEffect> {
        vec![
            HistoryEffect::LoadActivity,
            HistoryEffect::LoadDay(self.cursor),
        ]
    }

    /// Heatmap keys: `h`/`l` day, `H`/`L` week, `t` today.
    pub fn on_key(&mut self, key: KeyEvent) -> Option<Vec<HistoryEffect>> {
        let step = match key.code {
            KeyCode::Char('h') => -1,
            KeyCode::Char('l') => 1,
            KeyCode::Char('H') => -7,
            KeyCode::Char('L') => 7,
            KeyCode::Char('t') => {
                self.cursor = self.today;
                return Some(vec![HistoryEffect::LoadDay(self.cursor)]);
            }
            _ => return None,
        };
        let earliest = self.today - Duration::days(DAYS - 1);
        let next = (self.cursor + Duration::days(step)).clamp(earliest, self.today);
        if next == self.cursor {
            return Some(Vec::new());
        }
        self.cursor = next;
        Some(vec![HistoryEffect::LoadDay(next)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    fn activity(saved: i64) -> DayActivity {
        DayActivity {
            discovered: 0,
            added_manually: 0,
            actions: [("saved".to_owned(), saved), ("viewed".to_owned(), 5)]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn computes_streaks_and_peaks_from_data() {
        let mut days = BTreeMap::new();
        for day in [1, 2, 3, 10, 11, 12, 13] {
            days.insert(d(day), activity(1));
        }
        days.insert(d(12), activity(4));
        // A day with only views is not activity.
        days.insert(
            d(5),
            DayActivity {
                actions: [("viewed".to_owned(), 9)].into_iter().collect(),
                ..DayActivity::default()
            },
        );
        let s = stats(&days, d(14));
        assert_eq!(s.current_streak, 4, "ends yesterday (13th)");
        assert_eq!(s.longest_streak, 4);
        assert_eq!(s.most_active_day, Some((d(12), 4)));
        assert_eq!(s.most_active_week, Some((d(9), 7)));
        assert_eq!(s.active_days, 7);
        assert_eq!(s.saved, 10);
        assert_eq!(s.reviewed, 44);
    }

    #[test]
    fn empty_history_has_no_invented_numbers() {
        assert_eq!(stats(&BTreeMap::new(), d(14)), HistoryStats::default());
    }

    #[test]
    fn navigation_is_bounded_to_the_year() {
        let mut h = HistoryState {
            today: d(14),
            cursor: d(14),
            ..HistoryState::default()
        };
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(h.on_key(key('l')), Some(vec![]), "cannot go past today");
        assert_eq!(h.on_key(key('H')), Some(vec![HistoryEffect::LoadDay(d(7))]));
        assert_eq!(h.on_key(key('z')), None);
    }
}
