//! Modal overlays: help, command palette, search and add-link.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};

use super::widgets::{centered, panel};
use crate::input::LineInput;
use crate::state::{Mode, State};
use crate::theme::Theme;

/// Fixed navigation keys (not remappable).
const NAVIGATION: &[(&str, &str)] = &[
    ("j k ↓ ↑", "move"),
    ("g G", "first / last"),
    ("Enter", "open details"),
    ("Esc", "close / clear search"),
    ("Tab ⇧Tab [ ]", "next / previous section"),
    ("1…9 0", "jump to section"),
    ("← →", "focus sidebar / list / details"),
    ("Ctrl-p", "command palette"),
    ("Ctrl-c", "quit"),
];

fn confirm_quit(frame: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    let popup = centered(area, 60, 7);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from(Span::styled(
            "Settings have changes that are not saved yet.",
            theme.warn(),
        )),
        Line::default(),
        Line::from(vec![
            Span::styled(" s ", theme.key()),
            Span::styled(" go to Settings (then Ctrl-s)   ", theme.muted()),
            Span::styled(" y ", theme.key()),
            Span::styled(" quit without saving", theme.muted()),
        ]),
        Line::from(Span::styled("Any other key keeps working.", theme.muted())),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(panel(theme, "Unsaved settings", true)),
        popup,
    );
}

pub fn render(frame: &mut Frame, state: &State, theme: &Theme) {
    let area = frame.area();
    match &state.mode {
        Mode::Normal | Mode::Editor(_) => {}
        Mode::Help => help(frame, area, state, theme),
        Mode::ConfirmQuit => confirm_quit(frame, area, theme),
        Mode::Palette { input, selected } => palette(frame, area, state, theme, input, *selected),
        Mode::Search(input) => prompt(
            frame,
            area,
            theme,
            "Search",
            "Words must all appear (title, summary, tags, location). Enter applies, Esc cancels.",
            input,
        ),
        Mode::AddLink(input) => prompt(
            frame,
            area,
            theme,
            "Add link",
            "Paste a URL found while browsing. SENTRI extracts what it can; edit later if needed.",
            input,
        ),
    }
}

fn help(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let line = |keys: String, what: &str| {
        Line::from(vec![
            Span::styled(format!("{keys:>14}  "), theme.accent()),
            Span::raw(what.to_owned()),
        ])
    };
    let mut lines: Vec<Line> = NAVIGATION
        .iter()
        .map(|(k, w)| line((*k).to_owned(), w))
        .collect();
    lines.push(Line::default());
    let mut actions: Vec<_> = state
        .keymap
        .iter()
        .map(|(spec, action)| (*action, spec.to_string()))
        .collect();
    actions.sort();
    for (action, key) in actions {
        lines.push(line(key, action.label()));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Change shortcuts in Settings › Shortcuts. Sections like Calendar have their own keys.",
        theme.muted(),
    )));
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2;
    let popup = centered(area, 72, height);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(panel(theme, "Keyboard · any key closes", true)),
        popup,
    );
}

fn input_line<'a>(theme: &Theme, input: &'a LineInput) -> Line<'a> {
    Line::from(vec![
        Span::styled("› ", theme.accent()),
        Span::styled(
            input.value(),
            Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn place_cursor(frame: &mut Frame, inner: Rect, input: &LineInput) {
    let x = inner.x + 2 + u16::try_from(input.cursor()).unwrap_or(u16::MAX);
    frame.set_cursor_position(Position::new(
        x.min(inner.right().saturating_sub(1)),
        inner.y,
    ));
}

fn prompt(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    hint: &str,
    input: &LineInput,
) {
    let popup = centered(area, 80, 6);
    let block = panel(theme, title, true);
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(vec![
            input_line(theme, input),
            Line::default(),
            Line::from(Span::styled(hint, theme.muted())),
        ])
        .wrap(Wrap { trim: true })
        .block(block),
        popup,
    );
    place_cursor(frame, inner, input);
}

fn palette(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    theme: &Theme,
    input: &LineInput,
    selected: usize,
) {
    let popup = centered(area, 60, 18);
    let block = panel(theme, "Command palette", true);
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    let [query, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(inner);
    frame.render_widget(Paragraph::new(input_line(theme, input)), query);
    let items: Vec<ListItem> = state
        .palette_matches()
        .into_iter()
        .map(|c| ListItem::new(c.label()))
        .collect();
    let list = List::new(items)
        .highlight_style(theme.selected().fg(theme.p.yellow))
        .highlight_symbol("▌");
    let mut list_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, list_area, &mut list_state);
    place_cursor(frame, query, input);
}
