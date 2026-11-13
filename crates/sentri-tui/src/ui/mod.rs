//! Frame composition and responsive layout.
//!
//! - ≥ 110 columns: sidebar · feed · details
//! - ≥ 80 columns: sidebar · feed (details replace the feed when opened)
//! - narrower: feed only (Tab switches sections)

mod charts;
mod dashboard;
mod detail;
mod feed;
mod pages;
mod popups;
mod widgets;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use sentri_core::{PriceFilter, SortOrder};

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
    if state.loading {
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

fn active_filters(state: &State) -> Vec<&'static str> {
    let q = &state.query;
    let mut chips = Vec::new();
    match q.sort {
        SortOrder::Relevance => {}
        SortOrder::Newest => chips.push("newest"),
        SortOrder::Deadline => chips.push("by deadline"),
    }
    if q.price == Some(PriceFilter::Free) {
        chips.push("free");
    }
    if q.with_travel {
        chips.push("travel");
    }
    if q.only_upcoming {
        chips.push("upcoming");
    }
    if q.open_to_country {
        chips.push("eligible");
    }
    if q.with_certificate {
        chips.push("certificate");
    }
    chips
}

fn draw_body(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme) {
    let main = if area.width >= MEDIUM {
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
        Section::Sources => pages::sources(frame, main, state, theme),
        Section::Settings => pages::settings(frame, main, state, theme),
        Section::Calendar => draw_feed(frame, main, state, theme, "Calendar · upcoming dates"),
        Section::Feed(view) => draw_feed(frame, main, state, theme, view.label()),
    }
}

fn draw_feed(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme, title: &str) {
    if frame.area().width >= WIDE {
        let [list, detail] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(area);
        feed::render(frame, list, state, theme, title);
        detail::render(frame, detail, state, theme);
    } else if state.detail_open {
        detail::render(frame, area, state, theme);
    } else {
        feed::render(frame, area, state, theme, title);
    }
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
            ListItem::new(Line::from(vec![
                Span::styled(format!("{key} "), theme.muted()),
                Span::styled(
                    format!("{} ", section.icon()),
                    Style::new().fg(theme.p.aqua),
                ),
                Span::raw(section.label()),
            ]))
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
    let mut list_state = ListState::default().with_selected(Some(state.section));
    frame.render_stateful_widget(list, area, &mut list_state);
    state.hit.sidebar = area;
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
        let hints = [
            ("?", "help"),
            (":", "commands"),
            ("/", "search"),
            ("n", "add link"),
            ("l", "like"),
            ("s", "save"),
            ("d", "dismiss"),
            ("o", "open"),
            ("q", "quit"),
        ];
        let mut spans = vec![Span::raw(" ")];
        for (key, what) in hints {
            spans.push(Span::styled(format!(" {key} "), theme.key()));
            spans.push(Span::styled(format!(" {what}  "), theme.muted()));
        }
        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(theme.p.bg0_h)),
        area,
    );
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

    use crate::state::{Mode, Msg};

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
        assert!(screen.contains("Why this score"));
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
        assert!(!screen.contains("Why this score"));
    }

    #[test]
    fn empty_feed_explains_what_to_do() {
        let mut state = State::default();
        let screen = render(100, 20, &mut state);
        assert!(screen.contains("Press n to add a link"));
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
        assert!(render(120, 40, &mut state).contains("command palette"));
        state.mode = Mode::Palette {
            input: crate::input::LineInput::with_value("saved"),
            selected: 0,
        };
        assert!(render(120, 40, &mut state).contains("Go to Saved"));
    }

    #[test]
    fn records_hit_areas_for_mouse() {
        let mut state = State::default();
        state.on_msg(Msg::Loaded(vec![sample()]));
        render(140, 40, &mut state);
        assert_eq!(state.hit.sidebar.width, SIDEBAR_WIDTH);
        assert!(state.hit.list.width > 0);
        assert_eq!(state.hit.item_height, feed::ITEM_HEIGHT);
    }
}
