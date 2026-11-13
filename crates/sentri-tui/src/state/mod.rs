//! UI state and pure event handling.
//!
//! `State::on_key` and friends mutate the state and return [`Effect`]s; the
//! runtime executes effects (database, network, browser) and feeds results
//! back through `State::on_msg`. This keeps all behavior testable without a
//! terminal.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use sentri_app::{AddOutcome, CollectionRun, Insights, Ranked, SourceConfig, Stats};
use sentri_core::{
    Action, FeedQuery, FeedView, Flag, InteractionState, OpportunityId, Profile, SortOrder,
};

use crate::input::LineInput;

/// A navigable page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Dashboard,
    Feed(FeedView),
    Calendar,
    Sources,
    Settings,
}

impl Section {
    pub const ALL: [Self; 18] = [
        Self::Dashboard,
        Self::Feed(FeedView::Discover),
        Self::Feed(FeedView::News),
        Self::Feed(FeedView::Jobs),
        Self::Feed(FeedView::Courses),
        Self::Feed(FeedView::Events),
        Self::Feed(FeedView::Hackathons),
        Self::Feed(FeedView::Scholarships),
        Self::Feed(FeedView::Volunteering),
        Self::Feed(FeedView::OpenSource),
        Self::Feed(FeedView::Travel),
        Self::Feed(FeedView::Saved),
        Self::Feed(FeedView::Liked),
        Self::Feed(FeedView::Applied),
        Self::Calendar,
        Self::Feed(FeedView::History),
        Self::Sources,
        Self::Settings,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Feed(view) => view.label(),
            Self::Calendar => "Calendar",
            Self::Sources => "Sources",
            Self::Settings => "Settings",
        }
    }

    /// Single-width Unicode icon (no Nerd Fonts required).
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "◆",
            Self::Feed(FeedView::Discover) => "✦",
            Self::Feed(FeedView::News) => "▤",
            Self::Feed(FeedView::Jobs) => "⚒",
            Self::Feed(FeedView::Courses) => "✎",
            Self::Feed(FeedView::Events) => "◷",
            Self::Feed(FeedView::Hackathons) => "⚡",
            Self::Feed(FeedView::Scholarships) => "✪",
            Self::Feed(FeedView::Volunteering) => "♣",
            Self::Feed(FeedView::OpenSource) => "⌥",
            Self::Feed(FeedView::Travel) => "✈",
            Self::Feed(FeedView::Saved) => "★",
            Self::Feed(FeedView::Liked) => "♥",
            Self::Feed(FeedView::Applied) => "✓",
            Self::Calendar => "▦",
            Self::Feed(_) => "•",
            Self::Sources => "⇅",
            Self::Settings => "⚙",
        }
    }

    pub const fn view(self) -> Option<FeedView> {
        match self {
            Self::Feed(view) => Some(view),
            _ => None,
        }
    }
}

/// How much each card shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Detailed,
}

impl Density {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Detailed => "detailed",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Compact => Self::Comfortable,
            Self::Comfortable => Self::Detailed,
            Self::Detailed => Self::Compact,
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "compact" => Some(Self::Compact),
            "comfortable" => Some(Self::Comfortable),
            "detailed" => Some(Self::Detailed),
            _ => None,
        }
    }
}

/// Which pane receives navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    List,
    Detail,
}

/// Quick filters toggled from the keyboard or the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFilter {
    Free,
    Travel,
    Upcoming,
    Eligible,
    Certificate,
}

impl QuickFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Travel => "travel",
            Self::Upcoming => "upcoming",
            Self::Eligible => "open to my country",
            Self::Certificate => "certificate",
        }
    }
}

/// Commands offered by the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    GoTo(Section),
    Sort(SortOrder),
    Toggle(QuickFilter),
    Search,
    AddLink,
    Collect,
    Reload,
    Help,
    Quit,
}

impl Command {
    pub fn label(self) -> String {
        match self {
            Self::GoTo(section) => format!("Go to {}", section.label()),
            Self::Sort(SortOrder::Relevance) => "Sort by relevance".into(),
            Self::Sort(SortOrder::Newest) => "Sort by newest".into(),
            Self::Sort(SortOrder::Deadline) => "Sort by deadline".into(),
            Self::Toggle(filter) => format!("Toggle filter: {}", filter.label()),
            Self::Search => "Search…".into(),
            Self::AddLink => "Add link…".into(),
            Self::Collect => "Collect from sources now".into(),
            Self::Reload => "Reload".into(),
            Self::Help => "Keyboard shortcuts".into(),
            Self::Quit => "Quit".into(),
        }
    }

    pub fn all() -> Vec<Self> {
        let mut commands: Vec<Self> = vec![Self::AddLink, Self::Search, Self::Collect];
        commands.extend(Section::ALL.map(Self::GoTo));
        commands.extend([
            Self::Sort(SortOrder::Relevance),
            Self::Sort(SortOrder::Newest),
            Self::Sort(SortOrder::Deadline),
            Self::Toggle(QuickFilter::Free),
            Self::Toggle(QuickFilter::Travel),
            Self::Toggle(QuickFilter::Upcoming),
            Self::Toggle(QuickFilter::Eligible),
            Self::Toggle(QuickFilter::Certificate),
            Self::Reload,
            Self::Help,
            Self::Quit,
        ]);
        commands
    }
}

/// Case-insensitive subsequence match ("gtsv" matches "Go to Saved").
pub fn fuzzy_match(needle: &str, haystack: &str) -> bool {
    let mut hay = haystack.chars().flat_map(char::to_lowercase);
    needle
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !c.is_whitespace())
        .all(|n| hay.any(|h| h == n))
}

/// Modal overlays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search(LineInput),
    Palette { input: LineInput, selected: usize },
    AddLink(LineInput),
    Help,
}

/// Severity of a status-bar message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Error,
}

/// Work the runtime must perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Reload data for the current section with the current filters.
    Load,
    Act(OpportunityId, Action),
    AddLink(String),
    OpenUrl(String),
    Collect,
}

/// Read-only context shown by the dashboard, sources and settings pages.
#[derive(Debug, Clone, Default)]
pub struct Overview {
    pub stats: Stats,
    pub insights: Insights,
    /// Latest news for the dashboard.
    pub news: Vec<Ranked>,
    pub runs: Vec<CollectionRun>,
    pub sources: Vec<SourceConfig>,
    pub profile: Profile,
    pub config_path: String,
    pub database_path: String,
}

/// Results of effects.
#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Vec<Ranked>),
    Overview(Box<Overview>),
    Acted(OpportunityId, InteractionState),
    Added(Box<AddOutcome>),
    Info(String),
    /// Collection finished; the text summarizes it. Triggers a reload.
    Collected(String),
    Failed(String),
}

/// Screen areas from the last frame, used for mouse hit-testing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HitAreas {
    pub sidebar: Rect,
    pub list: Rect,
    /// Rows each list item occupies.
    pub item_height: u16,
    pub list_offset: usize,
}

/// Complete UI state.
#[derive(Debug, Clone)]
pub struct State {
    pub section: usize,
    pub focus: Focus,
    pub mode: Mode,
    pub items: Vec<Ranked>,
    pub selected: usize,
    pub query: FeedQuery,
    pub message: Option<(String, Level)>,
    pub loading: bool,
    pub detail_open: bool,
    pub detail_scroll: u16,
    pub hit: HitAreas,
    pub density: Density,
    pub overview: Overview,
    pub quit: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            section: 1,
            focus: Focus::List,
            mode: Mode::Normal,
            items: Vec::new(),
            selected: 0,
            query: FeedQuery::default(),
            message: None,
            loading: false,
            detail_open: false,
            detail_scroll: 0,
            hit: HitAreas::default(),
            density: Density::default(),
            overview: Overview::default(),
            quit: false,
        }
    }
}

impl State {
    pub const fn current(&self) -> Section {
        Section::ALL[self.section]
    }

    pub fn selected_item(&self) -> Option<&Ranked> {
        self.items.get(self.selected)
    }

    fn selected_id(&self) -> Option<OpportunityId> {
        self.selected_item().map(|r| r.item.id)
    }

    /// Query for the current section, including filters.
    pub fn effective_query(&self) -> FeedQuery {
        let mut query = self.query.clone();
        query.view = match self.current() {
            Section::Feed(view) => view,
            Section::Calendar => {
                query.sort = SortOrder::Deadline;
                query.only_upcoming = true;
                FeedView::Discover
            }
            Section::Dashboard | Section::Sources | Section::Settings => FeedView::Discover,
        };
        query.limit = Some(500);
        query
    }

    fn info(&mut self, text: impl Into<String>, level: Level) {
        self.message = Some((text.into(), level));
    }

    fn go_to(&mut self, index: usize) -> Vec<Effect> {
        if index >= Section::ALL.len() || index == self.section {
            return Vec::new();
        }
        self.section = index;
        self.selected = 0;
        self.detail_open = false;
        self.detail_scroll = 0;
        self.loading = true;
        vec![Effect::Load]
    }

    fn move_by(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() - 1;
        self.selected = self.selected.saturating_add_signed(delta).min(last);
        self.detail_scroll = 0;
    }

    fn reload(&mut self) -> Vec<Effect> {
        self.loading = true;
        vec![Effect::Load]
    }

    fn toggle_flag(&mut self, flag: Flag) -> Vec<Effect> {
        match self.selected_item() {
            Some(r) => vec![Effect::Act(r.item.id, r.item.state.toggle(flag))],
            None => Vec::new(),
        }
    }

    fn toggle_filter(&mut self, filter: QuickFilter) -> Vec<Effect> {
        let q = &mut self.query;
        let slot = match filter {
            QuickFilter::Free => {
                q.price = match q.price {
                    Some(sentri_core::PriceFilter::Free) => None,
                    _ => Some(sentri_core::PriceFilter::Free),
                };
                q.price.is_some()
            }
            QuickFilter::Travel => {
                q.with_travel = !q.with_travel;
                q.with_travel
            }
            QuickFilter::Upcoming => {
                q.only_upcoming = !q.only_upcoming;
                q.only_upcoming
            }
            QuickFilter::Eligible => {
                q.open_to_country = !q.open_to_country;
                q.open_to_country
            }
            QuickFilter::Certificate => {
                q.with_certificate = !q.with_certificate;
                q.with_certificate
            }
        };
        let verb = if slot { "on" } else { "off" };
        self.info(format!("Filter {}: {verb}", filter.label()), Level::Info);
        self.selected = 0;
        self.reload()
    }

    fn cycle_sort(&mut self) -> Vec<Effect> {
        self.query.sort = match self.query.sort {
            SortOrder::Relevance => SortOrder::Newest,
            SortOrder::Newest => SortOrder::Deadline,
            SortOrder::Deadline => SortOrder::Relevance,
        };
        self.reload()
    }

    fn open_detail(&mut self) -> Vec<Effect> {
        let Some(id) = self.selected_id() else {
            return Vec::new();
        };
        self.detail_open = true;
        self.focus = Focus::Detail;
        self.detail_scroll = 0;
        vec![Effect::Act(id, Action::Viewed)]
    }

    pub fn run_command(&mut self, command: Command) -> Vec<Effect> {
        self.mode = Mode::Normal;
        match command {
            Command::GoTo(section) => {
                let index = Section::ALL.iter().position(|s| *s == section);
                index.map(|i| self.go_to(i)).unwrap_or_default()
            }
            Command::Sort(order) => {
                self.query.sort = order;
                self.reload()
            }
            Command::Toggle(filter) => self.toggle_filter(filter),
            Command::Search => {
                self.mode = Mode::Search(LineInput::with_value(
                    self.query.text.as_deref().unwrap_or_default(),
                ));
                Vec::new()
            }
            Command::AddLink => {
                self.mode = Mode::AddLink(LineInput::default());
                Vec::new()
            }
            Command::Collect => {
                self.loading = true;
                self.info("Collecting from enabled sources…", Level::Info);
                vec![Effect::Collect]
            }
            Command::Reload => self.reload(),
            Command::Help => {
                self.mode = Mode::Help;
                Vec::new()
            }
            Command::Quit => {
                self.quit = true;
                Vec::new()
            }
        }
    }

    /// Palette entries matching the current palette input.
    pub fn palette_matches(&self) -> Vec<Command> {
        let Mode::Palette { input, .. } = &self.mode else {
            return Vec::new();
        };
        let needle = input.value();
        Command::all()
            .into_iter()
            .filter(|c| fuzzy_match(&needle, &c.label()))
            .collect()
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        // Any key acknowledges the previous message.
        self.message = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Vec::new();
        }
        match self.mode.clone() {
            Mode::Normal => self.on_normal_key(key),
            Mode::Help => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            Mode::Search(input) => self.on_search_key(input, key),
            Mode::AddLink(input) => self.on_add_key(input, key),
            Mode::Palette { input, selected } => self.on_palette_key(input, selected, key),
        }
    }

    fn on_search_key(&mut self, mut input: LineInput, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            KeyCode::Enter => {
                let text = input.value().trim().to_owned();
                self.query.text = (!text.is_empty()).then_some(text);
                self.mode = Mode::Normal;
                self.selected = 0;
                if self.current().view().is_none() {
                    // Searching from a non-feed page jumps to Discover.
                    self.section = 1;
                }
                self.reload()
            }
            _ => {
                input.handle(key);
                self.mode = Mode::Search(input);
                Vec::new()
            }
        }
    }

    fn on_add_key(&mut self, mut input: LineInput, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            KeyCode::Enter if !input.value().trim().is_empty() => {
                self.mode = Mode::Normal;
                self.loading = true;
                self.info("Fetching link…", Level::Info);
                vec![Effect::AddLink(input.value().trim().to_owned())]
            }
            _ => {
                input.handle(key);
                self.mode = Mode::AddLink(input);
                Vec::new()
            }
        }
    }

    fn on_palette_key(
        &mut self,
        mut input: LineInput,
        selected: usize,
        key: KeyEvent,
    ) -> Vec<Effect> {
        let matches = self.palette_matches();
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            KeyCode::Enter => match matches.get(selected) {
                Some(command) => self.run_command(*command),
                None => Vec::new(),
            },
            KeyCode::Down | KeyCode::Tab => {
                let next = (selected + 1).min(matches.len().saturating_sub(1));
                self.mode = Mode::Palette {
                    input,
                    selected: next,
                };
                Vec::new()
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.mode = Mode::Palette {
                    input,
                    selected: selected.saturating_sub(1),
                };
                Vec::new()
            }
            _ => {
                input.handle(key);
                self.mode = Mode::Palette { input, selected: 0 };
                Vec::new()
            }
        }
    }

    fn on_normal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char(':') => {
                self.mode = Mode::Palette {
                    input: LineInput::default(),
                    selected: 0,
                }
            }
            KeyCode::Char('p') if ctrl => {
                self.mode = Mode::Palette {
                    input: LineInput::default(),
                    selected: 0,
                }
            }
            KeyCode::Char('/') => return self.run_command(Command::Search),
            KeyCode::Char('n') => return self.run_command(Command::AddLink),
            KeyCode::Tab | KeyCode::Char(']') => {
                return self.go_to((self.section + 1) % Section::ALL.len());
            }
            KeyCode::BackTab | KeyCode::Char('[') => {
                let len = Section::ALL.len();
                return self.go_to((self.section + len - 1) % len);
            }
            KeyCode::Char(c @ '1'..='9') => {
                return self.go_to(usize::from(c as u8 - b'1'));
            }
            KeyCode::Char('0') => return self.go_to(9),
            KeyCode::Left => {
                self.focus = match self.focus {
                    Focus::Detail => Focus::List,
                    _ => Focus::Sidebar,
                };
            }
            KeyCode::Right => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::List,
                    _ => Focus::Detail,
                };
            }
            KeyCode::Esc => {
                if self.detail_open {
                    self.detail_open = false;
                    self.focus = Focus::List;
                } else if self.query.text.take().is_some() {
                    return self.reload();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => return self.on_vertical(1),
            KeyCode::Char('k') | KeyCode::Up => return self.on_vertical(-1),
            KeyCode::PageDown => return self.on_vertical(10),
            KeyCode::PageUp => return self.on_vertical(-10),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = self.items.len().saturating_sub(1),
            KeyCode::Enter => {
                if self.focus == Focus::Sidebar {
                    self.focus = Focus::List;
                    return Vec::new();
                }
                return self.open_detail();
            }
            KeyCode::Char('l') => return self.toggle_flag(Flag::Liked),
            KeyCode::Char('s') => return self.toggle_flag(Flag::Saved),
            KeyCode::Char('d') => return self.toggle_flag(Flag::Dismissed),
            KeyCode::Char('i') => return self.toggle_flag(Flag::Interesting),
            KeyCode::Char('p') => return self.toggle_flag(Flag::Applied),
            KeyCode::Char('e') => return self.toggle_flag(Flag::Enrolled),
            KeyCode::Char('c') => return self.toggle_flag(Flag::Completed),
            KeyCode::Char('a') => self.info(
                "Google Calendar sync is not available yet (phase 7). Dated items are listed under Calendar.",
                Level::Info,
            ),
            KeyCode::Char('o') => {
                if let Some(r) = self.selected_item() {
                    return vec![Effect::OpenUrl(r.item.opportunity.original_url.clone())];
                }
            }
            KeyCode::Char('S') => return self.cycle_sort(),
            KeyCode::Char('v') => {
                self.density = self.density.next();
                self.info(format!("Density: {}", self.density.label()), Level::Info);
            }
            KeyCode::Char('F') => return self.toggle_filter(QuickFilter::Free),
            KeyCode::Char('T') => return self.toggle_filter(QuickFilter::Travel),
            KeyCode::Char('U') => return self.toggle_filter(QuickFilter::Upcoming),
            KeyCode::Char('E') => return self.toggle_filter(QuickFilter::Eligible),
            KeyCode::Char('C') => return self.toggle_filter(QuickFilter::Certificate),
            KeyCode::Char('r') => return self.reload(),
            KeyCode::Char('R') => return self.run_command(Command::Collect),
            _ => {}
        }
        Vec::new()
    }

    fn on_vertical(&mut self, delta: isize) -> Vec<Effect> {
        match self.focus {
            Focus::Sidebar => {
                let len = Section::ALL.len();
                let target = self.section.saturating_add_signed(delta).min(len - 1);
                self.go_to(target)
            }
            Focus::Detail if self.detail_open => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add_signed(i16::try_from(delta).unwrap_or(0));
                Vec::new()
            }
            Focus::List | Focus::Detail => {
                self.move_by(delta);
                Vec::new()
            }
        }
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) -> Vec<Effect> {
        let at = Position::new(mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::ScrollDown => self.move_by(1),
            MouseEventKind::ScrollUp => self.move_by(-1),
            MouseEventKind::Down(_) if self.hit.sidebar.contains(at) => {
                let row = usize::from(at.y.saturating_sub(self.hit.sidebar.y + 1));
                self.focus = Focus::Sidebar;
                return self.go_to(row);
            }
            MouseEventKind::Down(_) if self.hit.list.contains(at) => {
                let height = self.hit.item_height.max(1);
                let row = usize::from((at.y.saturating_sub(self.hit.list.y + 1)) / height);
                let index = self.hit.list_offset + row;
                self.focus = Focus::List;
                if index == self.selected {
                    return self.open_detail();
                }
                if index < self.items.len() {
                    self.selected = index;
                    self.detail_scroll = 0;
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn on_added(&mut self, outcome: AddOutcome) -> Vec<Effect> {
        match outcome {
            AddOutcome::Added { item, warnings } => {
                let mut text = format!("Added #{}: {}", item.id, item.opportunity.title);
                if let Some(first) = warnings.first() {
                    text.push_str(" — ");
                    text.push_str(first);
                }
                let level = if warnings.is_empty() {
                    Level::Success
                } else {
                    Level::Info
                };
                self.info(text, level);
                self.reload()
            }
            AddOutcome::Duplicate(item) => {
                self.loading = false;
                self.info(
                    format!(
                        "Already in SENTRI as #{}: {}",
                        item.id, item.opportunity.title
                    ),
                    Level::Info,
                );
                Vec::new()
            }
        }
    }

    pub fn on_msg(&mut self, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Loaded(items) => {
                self.loading = false;
                self.items = items;
                self.selected = self.selected.min(self.items.len().saturating_sub(1));
            }
            Msg::Overview(overview) => self.overview = *overview,
            Msg::Acted(id, state) => {
                if let Some(r) = self.items.iter_mut().find(|r| r.item.id == id) {
                    r.item.state = state;
                }
            }
            Msg::Added(outcome) => return self.on_added(*outcome),
            Msg::Info(text) => self.info(text, Level::Info),
            Msg::Collected(text) => {
                self.info(text, Level::Success);
                return self.reload();
            }
            Msg::Failed(text) => {
                self.loading = false;
                self.info(text, Level::Error);
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use sentri_core::{CanonicalUrl, FeedItem, Opportunity, Relevance, SourceKind};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ranked(id: i64) -> Ranked {
        let url = CanonicalUrl::parse(&format!("https://x.example/{id}")).unwrap();
        Ranked {
            item: FeedItem {
                id: OpportunityId(id),
                opportunity: Opportunity::new(
                    SourceKind::Rss,
                    "f",
                    url.as_str(),
                    &url,
                    &format!("Item {id}"),
                    Utc::now(),
                ),
                state: InteractionState::default(),
            },
            relevance: Relevance::default(),
        }
    }

    fn loaded(n: i64) -> State {
        let mut state = State::default();
        state.on_msg(Msg::Loaded((1..=n).map(ranked).collect()));
        state
    }

    #[test]
    fn navigates_list_and_sections() {
        let mut s = loaded(3);
        s.on_key(key(KeyCode::Char('j')));
        s.on_key(key(KeyCode::Char('j')));
        s.on_key(key(KeyCode::Char('j')));
        assert_eq!(s.selected, 2);
        s.on_key(key(KeyCode::Char('g')));
        assert_eq!(s.selected, 0);
        assert_eq!(s.on_key(key(KeyCode::Tab)), vec![Effect::Load]);
        assert_eq!(s.current(), Section::Feed(FeedView::News));
        assert_eq!(s.on_key(key(KeyCode::Char('1'))), vec![Effect::Load]);
        assert_eq!(s.current(), Section::Dashboard);
        assert_eq!(
            s.on_key(key(KeyCode::BackTab)),
            vec![Effect::Load],
            "wraps to Settings"
        );
        assert_eq!(s.current(), Section::Settings);
    }

    #[test]
    fn interaction_keys_toggle_flags_of_selected_item() {
        let mut s = loaded(2);
        s.on_key(key(KeyCode::Down));
        let id = OpportunityId(2);
        assert_eq!(
            s.on_key(key(KeyCode::Char('l'))),
            vec![Effect::Act(id, Action::Set(Flag::Liked))]
        );
        s.on_msg(Msg::Acted(
            id,
            InteractionState {
                liked: true,
                ..InteractionState::default()
            },
        ));
        assert_eq!(
            s.on_key(key(KeyCode::Char('l'))),
            vec![Effect::Act(id, Action::Clear(Flag::Liked))]
        );
        assert_eq!(
            s.on_key(key(KeyCode::Char('s'))),
            vec![Effect::Act(id, Action::Set(Flag::Saved))]
        );
    }

    #[test]
    fn enter_opens_detail_and_records_view() {
        let mut s = loaded(1);
        assert_eq!(
            s.on_key(key(KeyCode::Enter)),
            vec![Effect::Act(OpportunityId(1), Action::Viewed)]
        );
        assert!(s.detail_open);
        s.on_key(key(KeyCode::Esc));
        assert!(!s.detail_open);
    }

    #[test]
    fn search_applies_text_and_escape_clears_it() {
        let mut s = loaded(1);
        s.on_key(key(KeyCode::Char('/')));
        for c in "rust".chars() {
            s.on_key(key(KeyCode::Char(c)));
        }
        assert_eq!(s.on_key(key(KeyCode::Enter)), vec![Effect::Load]);
        assert_eq!(s.query.text.as_deref(), Some("rust"));
        assert_eq!(s.on_key(key(KeyCode::Esc)), vec![Effect::Load]);
        assert_eq!(s.query.text, None);
    }

    #[test]
    fn add_link_emits_effect() {
        let mut s = loaded(0);
        s.on_key(key(KeyCode::Char('n')));
        for c in "https://a.b".chars() {
            s.on_key(key(KeyCode::Char(c)));
        }
        assert_eq!(
            s.on_key(key(KeyCode::Enter)),
            vec![Effect::AddLink("https://a.b".into())]
        );
        assert_eq!(s.mode, Mode::Normal);
    }

    #[test]
    fn palette_filters_and_runs_commands() {
        let mut s = loaded(0);
        s.on_key(key(KeyCode::Char(':')));
        for c in "gosaved".chars() {
            s.on_key(key(KeyCode::Char(c)));
        }
        assert_eq!(
            s.palette_matches().first(),
            Some(&Command::GoTo(Section::Feed(FeedView::Saved)))
        );
        assert_eq!(s.on_key(key(KeyCode::Enter)), vec![Effect::Load]);
        assert_eq!(s.current(), Section::Feed(FeedView::Saved));
        assert_eq!(s.mode, Mode::Normal);
    }

    #[test]
    fn quick_filters_and_sort_reload() {
        let mut s = loaded(0);
        assert_eq!(s.on_key(key(KeyCode::Char('T'))), vec![Effect::Load]);
        assert!(s.query.with_travel);
        s.on_key(key(KeyCode::Char('S')));
        assert_eq!(s.query.sort, SortOrder::Newest);
        s.on_key(key(KeyCode::Char('F')));
        assert_eq!(s.query.price, Some(sentri_core::PriceFilter::Free));
    }

    #[test]
    fn calendar_section_lists_upcoming_by_deadline() {
        let mut s = loaded(0);
        s.section = Section::ALL
            .iter()
            .position(|x| *x == Section::Calendar)
            .unwrap();
        let q = s.effective_query();
        assert_eq!(q.sort, SortOrder::Deadline);
        assert!(q.only_upcoming);
    }

    #[test]
    fn fuzzy_matching() {
        assert!(fuzzy_match("gtsv", "Go to Saved"));
        assert!(fuzzy_match("", "anything"));
        assert!(!fuzzy_match("xyz", "Go to Saved"));
    }

    #[test]
    fn collect_key_emits_effect() {
        let mut s = loaded(0);
        assert_eq!(s.on_key(key(KeyCode::Char('R'))), vec![Effect::Collect]);
        assert!(s.loading);
    }

    #[test]
    fn quits() {
        let mut s = loaded(0);
        s.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(s.quit);
    }
}
