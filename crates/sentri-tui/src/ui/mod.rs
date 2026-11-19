//! Frame composition and responsive layout.
//!
//! - ≥ 110 columns: sidebar · feed · details
//! - ≥ 80 columns: sidebar · feed (details replace the feed when opened)
//! - narrower: feed only (Tab switches sections)

mod ascii;
mod calendar;
mod card;
mod charts;
mod dashboard;
mod detail;
mod editor;
mod feed;
mod history;
pub mod image;
mod popups;
mod profile;
mod settings;
mod sources;
mod widgets;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use sentri_core::{PriceFilter, SortOrder};

use sentri_app::keys::KeyAction;

use crate::state::{Focus, Level, Section, State};
use crate::theme::Theme;
use widgets::panel;

const SIDEBAR_WIDTH: u16 = 24;
const WIDE: u16 = 110;
const MEDIUM: u16 = 80;

/// Draws the whole interface.
pub fn draw(frame: &mut Frame, state: &mut State, theme: &Theme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.base()), area);
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(frame, header, state, theme);
    draw_body(frame, body, state, theme);
    draw_status(frame, status, state, theme);
    popups::render(frame, state, theme);
    if let crate::state::Mode::Editor(editor) = &state.mode {
        editor::render(frame, editor, theme);
    }
    widgets::normalize_widths(frame.buffer_mut());
    if state.ui.ascii_icons {
        ascii::asciify(frame.buffer_mut());
    }
}

fn draw_header(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let s = &state.overview.stats;
    let mut spans = vec![
        Span::styled(" ◉ SENTRI ", theme.badge(theme.p.orange)),
        Span::styled(format!("  {}", state.current().label()), theme.title()),
    ];
    if let Some(text) = &state.query.text {
        spans.push(Span::styled(format!("  /{text}"), theme.accent()));
    }
    for chip in active_filters(state) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(format!("[{chip}]"), theme.warn()));
    }
    if let Some(progress) = &state.collecting {
        spans.push(Span::styled(
            format!("  ⟳ {}", progress.label()),
            theme.warn(),
        ));
    } else if state.loading {
        spans.push(Span::styled("  ⟳", theme.dim()));
    }
    let right = Line::from(vec![
        Span::styled(format!("✦ {} new ", s.new_since), theme.good()),
        Span::styled(format!("★ {} ", s.saved), theme.warn()),
        Span::styled(
            format!("⏳ {} ", s.upcoming_deadlines),
            Style::new().fg(theme.p.orange),
        ),
    ])
    .right_aligned();
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    if area.width >= MEDIUM {
        frame.render_widget(Paragraph::new(right), area);
    }
}

fn active_filters(state: &State) -> Vec<String> {
    let q = &state.query;
    let mut chips = Vec::new();
    match q.sort {
        SortOrder::Relevance => {}
        SortOrder::Newest => chips.push("newest".to_owned()),
        SortOrder::Deadline => chips.push("by deadline".to_owned()),
    }
    if q.price == Some(PriceFilter::Free) {
        chips.push("free".to_owned());
    }
    if q.with_travel {
        chips.push("travel".to_owned());
    }
    if q.only_upcoming {
        chips.push("upcoming".to_owned());
    }
    if q.only_past {
        chips.push("past".to_owned());
    }
    if q.open_to_country {
        chips.push("eligible".to_owned());
    }
    if q.with_certificate {
        chips.push("certificate".to_owned());
    }
    for modality in &q.modalities {
        chips.push(modality.label().to_lowercase());
    }
    if let Some(location) = q.location.as_deref().filter(|l| !l.trim().is_empty()) {
        chips.push(location.to_owned());
    }
    chips
}

fn draw_body(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme) {
    let main = if area.width >= MEDIUM && state.ui.show_sidebar {
        let [sidebar, main] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)]).areas(area);
        draw_sidebar(frame, sidebar, state, theme);
        main
    } else {
        state.hit.sidebar = Rect::default();
        area
    };
    match state.current() {
        Section::Dashboard => dashboard::render(frame, main, state, theme),
        Section::Sources => sources::render(frame, main, state, theme),
        Section::Settings => settings::render(frame, main, state, theme),
        Section::Calendar if state.detail_open => detail::render(frame, main, state, theme),
        Section::Calendar => calendar::render(frame, main, &state.calendar, theme),
        Section::Feed(sentri_core::FeedView::History) if !state.detail_open => {
            history::render(frame, main, state, theme);
        }
        Section::Feed(view) => draw_feed(frame, main, state, theme, view.label()),
    }
}

fn draw_feed(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme, title: &str) {
    let side_by_side = frame.area().width >= WIDE && state.ui.show_details;
    state.hit.detail_visible = side_by_side || state.detail_open;
    if side_by_side {
        let feed_width = state.ui.feed_width.clamp(30, 80);
        let [list, detail] = Layout::horizontal([
            Constraint::Percentage(feed_width),
            Constraint::Percentage(100 - feed_width),
        ])
        .areas(area);
        feed::render(frame, list, state, theme, title);
        detail::render(frame, detail, state, theme);
    } else if state.detail_open {
        detail::render(frame, area, state, theme);
    } else {
        feed::render(frame, area, state, theme, title);
    }
}

/// `text` cut to `width` characters with an ellipsis.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn draw_sidebar(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme) {
    let items: Vec<ListItem> = Section::ALL
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let key = match i {
                0..=8 => format!("{}", i + 1),
                9 => "0".into(),
                _ => " ".into(),
            };
            let count = match section {
                Section::Feed(view) => state
                    .overview
                    .counts
                    .iter()
                    .find(|(v, _)| v == view)
                    .map(|(_, n)| *n)
                    .filter(|n| *n > 0),
                _ => None,
            };
            let mut count = count.map(|n| {
                if n > 999 {
                    "999+".to_owned()
                } else {
                    n.to_string()
                }
            });
            // Unsaved settings stay marked wherever you are.
            let unsaved = *section == Section::Settings && state.settings.is_dirty();
            if unsaved {
                count = Some("●".to_owned());
            }
            // Room after "1 ◆ " inside the bordered, padded panel; some
            // icons (⚡) take two columns.
            let icon_width = Span::raw(section.icon()).width();
            let room = usize::from(SIDEBAR_WIDTH.saturating_sub(4 + 3)).saturating_sub(icon_width);
            let count_width = count.as_ref().map_or(0, |c| c.chars().count() + 1);
            let label = fit(section.label(), room.saturating_sub(count_width));
            let gap = room.saturating_sub(label.chars().count() + count_width);
            let mut spans = vec![
                Span::styled(format!("{key} "), theme.muted()),
                Span::styled(
                    format!("{} ", section.icon()),
                    Style::new().fg(theme.p.aqua),
                ),
                Span::raw(label),
            ];
            if let Some(count) = count {
                spans.push(Span::raw(" ".repeat(gap + 1)));
                spans.push(Span::styled(
                    count,
                    if unsaved { theme.warn() } else { theme.muted() },
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items)
        .block(panel(theme, "", state.focus == Focus::Sidebar))
        .highlight_style(
            Style::new()
                .fg(theme.p.yellow)
                .bg(theme.p.bg1)
                .add_modifier(Modifier::BOLD),
        );
    // Sections need 23 rows; the profile card takes what is left.
    let profile_height = match area.height {
        33.. => profile::FULL_HEIGHT,
        28.. => profile::COMPACT_HEIGHT,
        _ => 0,
    };
    let [nav, profile] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(profile_height)]).areas(area);
    let mut list_state = ListState::default().with_selected(Some(state.section));
    frame.render_stateful_widget(list, nav, &mut list_state);
    state.hit.sidebar = nav;
    if profile_height > 0 {
        profile::render(frame, profile, state, theme);
    }
}

/// Key currently bound to `action` (custom shortcuts included).
fn key_of(state: &State, action: KeyAction) -> String {
    state
        .keymap
        .iter()
        .find(|(_, a)| **a == action)
        .map_or_else(
            || action.default_key().to_owned(),
            |(spec, _)| spec.to_string(),
        )
}

/// Status bar hints for the current section.
fn hints(state: &State) -> Vec<(String, &'static str)> {
    let fixed = |pairs: &[(&str, &'static str)]| -> Vec<(String, &'static str)> {
        pairs.iter().map(|(k, w)| ((*k).to_owned(), *w)).collect()
    };
    let mut hints = match state.current() {
        Section::Sources => fixed(&[
            ("+", "add"),
            ("e", "edit"),
            ("space", "on/off"),
            ("u", "refresh"),
            ("x", "delete"),
        ]),
        Section::Calendar => fixed(&[
            ("h/l", "day"),
            ("</>", "period"),
            ("v", "view"),
            ("t", "today"),
            ("+", "new"),
        ]),
        Section::Settings => fixed(&[("Enter", "edit"), ("ctrl-s", "save")]),
        Section::Dashboard => [
            (KeyAction::Collect, "collect now"),
            (KeyAction::Search, "search"),
            (KeyAction::AddLink, "add link"),
        ]
        .into_iter()
        .map(|(a, w)| (key_of(state, a), w))
        .collect(),
        Section::Feed(_) => [
            (KeyAction::Search, "search"),
            (KeyAction::Like, "like"),
            (KeyAction::Save, "save"),
            (KeyAction::Dismiss, "dismiss"),
            (KeyAction::Open, "open"),
            (KeyAction::Density, "density"),
        ]
        .into_iter()
        .map(|(a, w)| (key_of(state, a), w))
        .collect(),
    };
    hints.push((key_of(state, KeyAction::Palette), "commands"));
    hints.push((key_of(state, KeyAction::Help), "help"));
    hints.push((key_of(state, KeyAction::Quit), "quit"));
    hints
}

fn draw_status(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let line = if let Some((text, level)) = &state.message {
        let style = match level {
            Level::Info => theme.dim(),
            Level::Success => theme.good(),
            Level::Error => theme.bad(),
        };
        Line::from(Span::styled(format!(" {text}"), style))
    } else {
        let mut spans = vec![Span::raw(" ")];
        for (key, what) in hints(state) {
            spans.push(Span::styled(format!(" {key} "), theme.key()));
            spans.push(Span::styled(format!(" {what}  "), theme.muted()));
        }
        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(theme.p.bg0_h)),
        area,
    );
    if let Some(progress) = state.collecting.as_ref().filter(|_| area.width >= MEDIUM) {
        let mut spans = vec![Span::styled(
            format!(
                "⟳ {}/{} ",
                progress.done.min(progress.total),
                progress.total
            ),
            theme.warn(),
        )];
        spans.extend(widgets::progress_bar(theme, progress.ratio(), 16));
        spans.push(Span::styled(
            format!(" +{} new ", progress.new),
            theme.good(),
        ));
        frame.render_widget(Paragraph::new(Line::from(spans).right_aligned()), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use sentri_app::Ranked;
    use sentri_core::{
        CanonicalUrl, Category, FeedItem, InteractionState, Opportunity, OpportunityId, Reason,
        Relevance, SourceKind,
    };

    use crate::state::{Density, Mode, Msg};

    fn sample() -> Ranked {
        let url = CanonicalUrl::parse("https://devpost.com/hack").unwrap();
        let mut o = Opportunity::new(
            SourceKind::Devpost,
            "manual",
            url.as_str(),
            &url,
            "AI Hackathon Cusco",
            Utc::now(),
        );
        o.category = Category::Hackathon;
        o.location = Some("Cusco".into());
        o.dates.deadline = Some(Utc::now() + Duration::days(5) + Duration::hours(1));
        o.perks.travel_covered = Some(true);
        Ranked {
            item: FeedItem {
                id: OpportunityId(1),
                opportunity: o,
                state: InteractionState {
                    saved: true,
                    ..InteractionState::default()
                },
            },
            relevance: Relevance {
                score: 42,
                reasons: vec![Reason {
                    points: 42,
                    text: "Matches interest 'AI'".into(),
                }],
            },
        }
    }

    fn render(width: u16, height: u16, state: &mut State) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| draw(f, state, &Theme::default()))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn wide_layout_shows_sidebar_feed_and_details() {
        let mut state = State::default();
        state.on_msg(Msg::Loaded(vec![sample()]));
        let screen = render(140, 40, &mut state);
        assert!(screen.contains("SENTRI"));
        assert!(screen.contains("Dashboard"));
        assert!(screen.contains("AI Hackathon Cusco"));
        assert!(screen.contains(" DP "), "source badge");
        assert!(screen.contains("in 5d"));
        assert!(screen.contains("SENTRI recommendation"));
        assert!(screen.contains("Matches interest 'AI'"));
        assert!(screen.contains('★'));
    }

    #[test]
    fn narrow_layout_hides_sidebar_and_details() {
        let mut state = State::default();
        state.on_msg(Msg::Loaded(vec![sample()]));
        let screen = render(60, 20, &mut state);
        assert!(!screen.contains("Dashboard"));
        assert!(screen.contains("AI Hackathon"));
        assert!(!screen.contains("SENTRI recommendation"));
    }

    #[test]
    fn top_picks_mix_sources() {
        let with = |source, id| {
            let mut r = sample();
            r.item.opportunity.source = source;
            r.item.id = OpportunityId(id);
            r
        };
        let items: Vec<Ranked> = (1..=6)
            .map(|id| with(SourceKind::GetOnBoard, id))
            .chain([with(SourceKind::DevEvents, 7), with(SourceKind::Rss, 8)])
            .collect();
        let ids =
            |picked: Vec<&Ranked>| -> Vec<i64> { picked.iter().map(|r| r.item.id.0).collect() };
        assert_eq!(ids(dashboard::diversified(&items, 5, 3)), [1, 2, 3, 7, 8]);
        // Not enough variety: the rest is filled in ranking order.
        assert_eq!(
            ids(dashboard::diversified(&items, 7, 3)),
            [1, 2, 3, 7, 8, 4, 5]
        );
    }

    #[test]
    fn first_run_shows_welcome_and_progress() {
        let mut state = State {
            collecting: Some(crate::state::collect::CollectProgress {
                done: 5,
                total: 20,
                new: 34,
                first_run: true,
                ..Default::default()
            }),
            ..State::default()
        };
        let screen = render(140, 30, &mut state);
        assert!(screen.contains("Welcome to SENTRI"));
        assert!(screen.contains("from 20 sources."), "{screen}");
        assert!(screen.contains("collecting 5/20 · +34"), "header");
        assert!(screen.contains("+34 new"), "status bar");
        assert!(!screen.contains("No opportunities here yet"));
    }

    #[test]
    fn status_hints_follow_custom_shortcuts() {
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("like".to_owned(), "f5".to_owned());
        let mut state = State {
            keymap: crate::state::keymap(&keys),
            ..State::default()
        };
        let screen = render(140, 20, &mut state);
        assert!(screen.contains(" f5  like"), "{screen}");
    }

    #[test]
    fn sidebar_shows_counts_that_fit() {
        let mut state = State::default();
        state.overview.counts = vec![
            (sentri_core::FeedView::Jobs, 49),
            (sentri_core::FeedView::Hackathons, 7),
            (sentri_core::FeedView::CallForSpeakers, 3),
        ];
        let screen = render(140, 30, &mut state);
        let rows: Vec<&str> = screen.lines().collect();
        let row = |label: &str| rows.iter().find(|r| r.contains(label)).copied().unwrap();
        assert!(
            row("Jobs").contains("Jobs          49 │"),
            "{}",
            row("Jobs")
        );
        assert!(row("Hackathons").contains(" 7 │"), "{}", row("Hackathons"));
        assert!(
            row("Call for").contains("Call for Spea… 3 │"),
            "{}",
            row("Call for")
        );
    }

    #[test]
    fn quitting_with_unsaved_settings_asks_first() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let config = sentri_app::Config::parse(
            sentri_app::config::DEFAULT_CONFIG,
            std::path::Path::new("d"),
        )
        .unwrap();
        let mut state = State {
            settings: crate::state::settings::SettingsState::from_config(&config),
            ..State::default()
        };
        let row = state
            .settings
            .rows
            .iter_mut()
            .find(|r| r.key == "github.username")
            .unwrap();
        row.value = "hustavojhon".into();
        assert!(state.settings.is_dirty());

        let screen = render(140, 30, &mut state);
        assert!(
            screen.contains("Settings       ● │"),
            "sidebar marker\n{screen}"
        );
        state.on_key(key('q'));
        assert!(!state.quit, "asks first");
        assert!(render(140, 30, &mut state).contains("Unsaved settings"));
        state.on_key(key('x'));
        assert!(!state.quit, "any other key keeps working");
        state.on_key(key('q'));
        state.on_key(key('y'));
        assert!(state.quit);
    }

    #[test]
    fn empty_feed_explains_what_to_do() {
        let mut state = State::default();
        let screen = render(100, 20, &mut state);
        assert!(screen.contains("Press n to add a link"));
    }

    #[test]
    fn densities_change_card_height_and_mark_new_items() {
        let mut state = State::default();
        state.on_msg(Msg::Loaded(vec![sample()]));
        for density in [Density::Compact, Density::Comfortable, Density::Detailed] {
            state.density = density;
            let screen = render(140, 30, &mut state);
            assert!(screen.contains("NEW"), "{density:?}");
            assert!(screen.contains(density.label()));
            assert_eq!(state.hit.item_height, feed::item_height(density));
        }
    }

    #[test]
    fn dashboard_and_popups_render() {
        let mut state = State {
            section: 0,
            ..State::default()
        };
        state.on_msg(Msg::Loaded(vec![sample()]));
        let screen = render(120, 30, &mut state);
        assert!(screen.contains("Top recommendations"));
        assert!(screen.contains("Upcoming dates"));
        assert!(screen.contains("Discovered · 14 days"));
        let wide = render(160, 40, &mut state);
        assert!(wide.contains("Latest tech news"));
        assert!(wide.contains("Categories · sources · modality"));

        state.mode = Mode::Help;
        let help = render(120, 50, &mut state);
        assert!(help.contains("command palette"));
        assert!(help.contains("Like"), "remappable actions listed");
        state.mode = Mode::Palette {
            input: crate::input::LineInput::with_value("saved"),
            selected: 0,
        };
        assert!(render(120, 40, &mut state).contains("Go to Saved"));
    }

    #[test]
    fn calendar_month_view_renders_entries() {
        use crate::state::calendar::CalItem;
        let mut state = State {
            section: Section::ALL
                .iter()
                .position(|s| *s == Section::Calendar)
                .unwrap(),
            ..State::default()
        };
        let today = state.calendar.today;
        state.calendar.items = vec![CalItem {
            at: Utc::now(),
            date: today,
            time: Some("18:00".into()),
            title: "Rust meetup".into(),
            kind: sentri_core::EntryKind::Personal,
            category: None,
            entry_id: Some(1),
            opportunity_id: None,
        }];
        let screen = render(160, 44, &mut state);
        assert!(screen.contains("Month view"));
        assert!(screen.contains("Mon"));
        assert!(screen.contains("Rust meetup"));
        assert!(screen.contains("Google Calendar: not connected"));
        state.calendar.sunday_first = true;
        let sunday = render(160, 44, &mut state);
        let header = sunday
            .lines()
            .find(|l| l.contains("Mon") && l.contains("Sun"))
            .unwrap();
        assert!(
            header.find("Sun") < header.find("Mon"),
            "weeks start on Sunday"
        );
        state.calendar.view = crate::state::calendar::CalView::Agenda;
        assert!(render(160, 44, &mut state).contains("Agenda from"));
    }

    #[test]
    fn ascii_mode_renders_only_ascii_for_ascii_content() {
        let mut item = sample();
        item.item.opportunity.location = Some("Cusco".into());
        for section in 0..Section::ALL.len() {
            let mut state = State {
                section,
                ..State::default()
            };
            state.ui.ascii_icons = true;
            state.on_msg(Msg::Loaded(vec![item.clone()]));
            let screen = render(150, 44, &mut state);
            let bad: String = screen.chars().filter(|c| !c.is_ascii()).collect();
            assert!(
                bad.is_empty(),
                "section {section}: non-ASCII glyphs {bad:?}"
            );
        }
    }

    #[test]
    fn records_hit_areas_for_mouse() {
        let mut state = State::default();
        state.on_msg(Msg::Loaded(vec![sample()]));
        render(140, 40, &mut state);
        assert_eq!(state.hit.sidebar.width, SIDEBAR_WIDTH);
        assert!(state.hit.list.width > 0);
        assert_eq!(state.hit.item_height, feed::item_height(state.density));
    }
}
