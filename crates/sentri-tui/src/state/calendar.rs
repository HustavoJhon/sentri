//! Calendar section state: views, date navigation and entry editing.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use sentri_app::{CalendarData, NewEntry};
use sentri_core::{Category, EntryKind, FeedItem, OpportunityId};

use crate::input::LineInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalView {
    #[default]
    Month,
    Week,
    Day,
    Agenda,
}

impl CalView {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Month => "Month",
            Self::Week => "Week",
            Self::Day => "Day",
            Self::Agenda => "Agenda",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Month => Self::Week,
            Self::Week => Self::Day,
            Self::Day => Self::Agenda,
            Self::Agenda => Self::Month,
        }
    }
}

/// One thing shown on the calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalItem {
    pub at: DateTime<Utc>,
    pub date: NaiveDate,
    /// Local `HH:MM`, or `None` for all-day items.
    pub time: Option<String>,
    pub title: String,
    pub kind: EntryKind,
    pub category: Option<Category>,
    /// SENTRI entry id when it is in the calendar; `None` for suggestions.
    pub entry_id: Option<i64>,
    pub opportunity_id: Option<OpportunityId>,
}

impl CalItem {
    pub const fn is_scheduled(&self) -> bool {
        self.entry_id.is_some()
    }
}

/// Text input currently open in the calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalInput {
    New(LineInput),
    Edit(i64, LineInput),
}

/// Work the calendar asks the runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalEffect {
    Load(DateTime<Utc>, DateTime<Utc>),
    Create(NewEntry),
    Update(i64, NewEntry),
    Delete(i64),
    Schedule(OpportunityId),
    Open(Box<FeedItem>),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarState {
    pub view: CalView,
    pub cursor: NaiveDate,
    pub today: NaiveDate,
    pub items: Vec<CalItem>,
    pub opportunities: HashMap<i64, FeedItem>,
    /// Index of the selected item within the cursor day (or agenda).
    pub selected: usize,
    pub input: Option<CalInput>,
    pub confirm_delete: Option<i64>,
    pub loaded: Option<(NaiveDate, NaiveDate)>,
}

impl Default for CalendarState {
    fn default() -> Self {
        let today = Local::now().date_naive();
        Self {
            view: CalView::Month,
            cursor: today,
            today,
            items: Vec::new(),
            opportunities: HashMap::new(),
            selected: 0,
            input: None,
            confirm_delete: None,
            loaded: None,
        }
    }
}

fn local(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    Local
        .from_local_datetime(&date.and_time(time))
        .earliest()
        .map_or_else(|| date.and_time(time).and_utc(), |d| d.with_timezone(&Utc))
}

pub fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

pub fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + i32::try_from(d.month0()).unwrap_or(0) + months;
    let year = total.div_euclid(12);
    let month = u32::try_from(total.rem_euclid(12)).unwrap_or(0) + 1;
    (0..4)
        .find_map(|back| NaiveDate::from_ymd_opt(year, month, d.day().saturating_sub(back)))
        .unwrap_or(d)
}

/// Monday of the week containing `d`.
pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(i64::from(d.weekday().num_days_from_monday()))
}

/// Parses `[HH:MM[-HH:MM]] title` (or `remind title`) on `date`.
pub fn parse_entry(text: &str, date: NaiveDate) -> Result<NewEntry, String> {
    let mut rest = text.trim();
    let mut kind = EntryKind::Personal;
    if let Some(r) = rest.strip_prefix("remind ") {
        kind = EntryKind::Reminder;
        rest = r.trim();
    }
    let (start, end, title) = match rest.split_once(' ') {
        Some((times, title)) if times.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
            let (a, b) = times
                .split_once('-')
                .map_or((times, None), |(a, b)| (a, Some(b)));
            let parse = |t: &str| {
                NaiveTime::parse_from_str(t, "%H:%M").map_err(|_| format!("invalid time {t:?}"))
            };
            (Some(parse(a)?), b.map(parse).transpose()?, title)
        }
        _ => (None, None, rest),
    };
    if title.trim().is_empty() {
        return Err("Write a title, e.g. \"18:00-20:00 Rust meetup\"".into());
    }
    let starts_at = start.map_or_else(|| sentri_core::dates::all_day(date), |t| local(date, t));
    let ends_at = end.map(|e| local(date, e));
    if ends_at.is_some_and(|e| e < starts_at) {
        return Err("The end time is before the start time".into());
    }
    Ok(NewEntry {
        kind,
        title: title.trim().to_owned(),
        starts_at,
        ends_at,
        all_day: start.is_none(),
        notes: None,
    })
}

fn to_item(
    at: DateTime<Utc>,
    all_day: bool,
    title: String,
    kind: EntryKind,
    category: Option<Category>,
    entry_id: Option<i64>,
    opportunity_id: Option<OpportunityId>,
) -> CalItem {
    let local = at.with_timezone(&Local);
    let whole_day = all_day || sentri_core::dates::is_all_day(at);
    CalItem {
        at,
        date: sentri_core::dates::calendar_date(at, |d| d.with_timezone(&Local).date_naive()),
        time: (!whole_day).then(|| local.format("%H:%M").to_string()),
        title,
        kind,
        category,
        entry_id,
        opportunity_id,
    }
}

impl CalendarState {
    /// Range to load for the current cursor and view (generous, so moving
    /// around rarely needs a new query).
    fn range(&self) -> (NaiveDate, NaiveDate) {
        match self.view {
            CalView::Agenda => (self.cursor, self.cursor + Duration::days(90)),
            _ => (
                week_start(first_of_month(add_months(self.cursor, -1))),
                first_of_month(add_months(self.cursor, 2)) + Duration::days(7),
            ),
        }
    }

    /// Requests data when the cursor left the loaded range.
    pub fn ensure_loaded(&mut self) -> Option<CalEffect> {
        let (from, to) = self.range();
        let covered = self.loaded.is_some_and(|(a, b)| {
            a <= self.cursor && self.cursor < b && (self.view != CalView::Agenda || a == from)
        });
        if covered {
            return None;
        }
        self.loaded = Some((from, to));
        Some(CalEffect::Load(
            local(from, NaiveTime::MIN),
            local(to, NaiveTime::MIN),
        ))
    }

    pub fn reload(&mut self) -> Option<CalEffect> {
        self.loaded = None;
        self.ensure_loaded()
    }

    pub fn set_data(&mut self, data: CalendarData) {
        let mut items = Vec::new();
        let mut opportunities = HashMap::new();
        for e in data.entries {
            let category = e
                .opportunity_id
                .and_then(|id| self.opportunities.get(&id))
                .map(|i| i.opportunity.category);
            items.push(to_item(
                e.starts_at,
                e.all_day,
                e.title,
                e.kind,
                category,
                Some(e.id),
                e.opportunity_id.map(OpportunityId),
            ));
        }
        for item in data.suggestions {
            for (kind, at) in sentri_core::calendar::schedulable_dates(&item.opportunity) {
                items.push(to_item(
                    at,
                    false,
                    item.opportunity.title.clone(),
                    kind,
                    Some(item.opportunity.category),
                    None,
                    Some(item.id),
                ));
            }
            opportunities.insert(item.id.0, item);
        }
        items.sort_by(|a, b| a.at.cmp(&b.at).then(a.title.cmp(&b.title)));
        self.items = items;
        self.opportunities.extend(opportunities);
        self.selected = 0;
    }

    pub fn items_on(&self, date: NaiveDate) -> Vec<&CalItem> {
        self.items.iter().filter(|i| i.date == date).collect()
    }

    /// Items in the agenda list (from the cursor on).
    pub fn agenda(&self) -> Vec<&CalItem> {
        self.items
            .iter()
            .filter(|i| i.date >= self.cursor)
            .collect()
    }

    pub fn selected_item(&self) -> Option<&CalItem> {
        let list = if self.view == CalView::Agenda {
            self.agenda()
        } else {
            self.items_on(self.cursor)
        };
        list.get(self.selected).copied()
    }

    fn move_days(&mut self, days: i64) {
        self.cursor += Duration::days(days);
        self.selected = 0;
    }

    fn move_period(&mut self, forward: bool) {
        let sign = if forward { 1 } else { -1 };
        match self.view {
            CalView::Month => {
                self.cursor = add_months(self.cursor, sign);
                self.selected = 0;
            }
            CalView::Week => self.move_days(7 * i64::from(sign)),
            CalView::Day | CalView::Agenda => self.move_days(i64::from(sign)),
        }
    }

    fn on_input_key(&mut self, mut input: CalInput, key: KeyEvent) -> Vec<CalEffect> {
        let (CalInput::New(line) | CalInput::Edit(_, line)) = &mut input;
        match key.code {
            KeyCode::Esc => Vec::new(),
            KeyCode::Enter => match parse_entry(&line.value(), self.cursor) {
                Ok(entry) => match input {
                    CalInput::New(_) => vec![CalEffect::Create(entry)],
                    CalInput::Edit(id, _) => vec![CalEffect::Update(id, entry)],
                },
                Err(e) => {
                    self.input = Some(input);
                    vec![CalEffect::Message(e)]
                }
            },
            _ => {
                line.handle(key);
                self.input = Some(input);
                Vec::new()
            }
        }
    }

    /// Handles a key in the Calendar section. Returns `None` when the key is
    /// not a calendar key (so global bindings apply).
    pub fn on_key(&mut self, key: KeyEvent) -> Option<Vec<CalEffect>> {
        if let Some(input) = self.input.take() {
            return Some(self.on_input_key(input, key));
        }
        if let Some(id) = self.confirm_delete.take() {
            let yes = matches!(key.code, KeyCode::Char('y' | 'Y'));
            return Some(if yes {
                vec![CalEffect::Delete(id)]
            } else {
                Vec::new()
            });
        }
        let agenda = self.view == CalView::Agenda;
        let count = if agenda {
            self.agenda().len()
        } else {
            self.items_on(self.cursor).len()
        };
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.move_days(-1),
            KeyCode::Right | KeyCode::Char('l') => self.move_days(1),
            KeyCode::Up | KeyCode::Char('k') if agenda => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') if agenda => {
                self.selected = (self.selected + 1).min(count.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_days(if self.view == CalView::Month { -7 } else { -1 });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_days(if self.view == CalView::Month { 7 } else { 1 });
            }
            KeyCode::Char('K') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('J') => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            KeyCode::Char('<' | ',') => self.move_period(false),
            KeyCode::Char('>' | '.') => self.move_period(true),
            KeyCode::Char('t') => {
                self.cursor = self.today;
                self.selected = 0;
            }
            KeyCode::Char('v') => {
                self.view = self.view.next();
                self.selected = 0;
            }
            KeyCode::Char('+') => self.input = Some(CalInput::New(LineInput::default())),
            KeyCode::Char('e') => {
                let item = self.selected_item()?.clone();
                let id = item.entry_id?;
                let text = match item.time {
                    Some(t) => format!("{t} {}", item.title),
                    None => item.title,
                };
                self.input = Some(CalInput::Edit(id, LineInput::with_value(&text)));
            }
            KeyCode::Char('x') => {
                if let Some(id) = self.selected_item().and_then(|i| i.entry_id) {
                    self.confirm_delete = Some(id);
                } else {
                    return Some(vec![CalEffect::Message(
                        "Only SENTRI calendar entries can be deleted; suggestions are just dates of opportunities.".into(),
                    )]);
                }
            }
            KeyCode::Char('a') => {
                let item = self.selected_item()?;
                if item.is_scheduled() {
                    return Some(vec![CalEffect::Message("Already in your calendar.".into())]);
                }
                return Some(vec![CalEffect::Schedule(item.opportunity_id?)]);
            }
            KeyCode::Enter => {
                let id = self.selected_item()?.opportunity_id?;
                let item = self.opportunities.get(&id.0)?.clone();
                return Some(vec![CalEffect::Open(Box::new(item))]);
            }
            KeyCode::Char('g') => {
                return Some(vec![CalEffect::Message(
                    "Google Calendar is not connected yet (needs OAuth credentials, see docs/calendar.md).".into(),
                )]);
            }
            _ => return None,
        }
        Some(self.ensure_loaded().into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn month_arithmetic_clamps_days() {
        assert_eq!(add_months(date(2026, 1, 31), 1), date(2026, 2, 28));
        assert_eq!(add_months(date(2026, 12, 15), 1), date(2027, 1, 15));
        assert_eq!(add_months(date(2026, 1, 15), -1), date(2025, 12, 15));
        assert_eq!(week_start(date(2026, 11, 13)), date(2026, 11, 9));
    }

    #[test]
    fn parses_quick_entries() {
        let d = date(2026, 11, 20);
        let e = parse_entry("18:00-20:00 Rust meetup", d).unwrap();
        assert_eq!(e.title, "Rust meetup");
        assert!(!e.all_day && e.ends_at.is_some());
        let r = parse_entry("remind Send CV", d).unwrap();
        assert_eq!(r.kind, EntryKind::Reminder);
        assert!(r.all_day);
        assert!(parse_entry("25:00 Bad", d).is_err());
        assert!(parse_entry("20:00-18:00 Backwards", d).is_err());
        assert!(parse_entry("   ", d).is_err());
    }

    #[test]
    fn navigates_and_creates_entries() {
        let mut c = CalendarState {
            cursor: date(2026, 11, 13),
            ..CalendarState::default()
        };
        assert!(matches!(c.ensure_loaded(), Some(CalEffect::Load(..))));
        assert_eq!(c.ensure_loaded(), None, "already loaded");
        c.on_key(key(KeyCode::Char('l')));
        assert_eq!(c.cursor, date(2026, 11, 14));
        c.on_key(key(KeyCode::Char('j')));
        assert_eq!(c.cursor, date(2026, 11, 21), "month view moves by weeks");
        c.on_key(key(KeyCode::Char('>')));
        assert_eq!(c.cursor, date(2026, 12, 21));
        c.on_key(key(KeyCode::Char('v')));
        assert_eq!(c.view, CalView::Week);

        c.on_key(key(KeyCode::Char('+')));
        for ch in "Study".chars() {
            c.on_key(key(KeyCode::Char(ch)));
        }
        let effects = c.on_key(key(KeyCode::Enter)).unwrap();
        assert!(matches!(&effects[0], CalEffect::Create(e) if e.title == "Study"));
        assert_eq!(
            c.on_key(key(KeyCode::Char('Z'))),
            None,
            "not a calendar key"
        );
    }

    #[test]
    fn deleting_requires_a_sentri_entry_and_confirmation() {
        let mut c = CalendarState::default();
        let today = c.cursor;
        c.items = vec![CalItem {
            at: Utc::now(),
            date: today,
            time: None,
            title: "Mine".into(),
            kind: EntryKind::Personal,
            category: None,
            entry_id: Some(4),
            opportunity_id: None,
        }];
        c.loaded = Some((today - Duration::days(40), today + Duration::days(40)));
        assert_eq!(c.on_key(key(KeyCode::Char('x'))), Some(vec![]));
        assert_eq!(c.confirm_delete, Some(4));
        assert_eq!(
            c.on_key(key(KeyCode::Char('y'))),
            Some(vec![CalEffect::Delete(4)])
        );
    }
}
