//! Settings form.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use sentri_app::settings::SettingKind;

use super::widgets::panel;
use crate::state::State;
use crate::theme::Theme;

/// List items grouped by section, and the setting index of each line.
fn rows<'a>(
    s: &'a crate::state::settings::SettingsState,
    theme: &Theme,
) -> (Vec<ListItem<'a>>, Vec<Option<usize>>) {
    let mut items = Vec::new();
    let mut index_map = Vec::new();
    let mut group = "";
    for (i, row) in s.rows.iter().enumerate() {
        if row.group != group {
            group = &row.group;
            items.push(ListItem::new(Line::from(Span::styled(
                format!("── {group}"),
                Style::new().fg(theme.p.aqua).add_modifier(Modifier::BOLD),
            ))));
            index_map.push(None);
        }
        let value = match row.kind {
            SettingKind::Bool => {
                if row.value == "true" {
                    "● on".to_owned()
                } else {
                    "○ off".to_owned()
                }
            }
            SettingKind::Choice(_) | SettingKind::Int { .. } => format!("‹ {} ›", row.value),
            SettingKind::Text | SettingKind::List(_) if row.value.is_empty() => "—".to_owned(),
            SettingKind::Text | SettingKind::List(_) => row.value.clone(),
        };
        let mut spans = vec![
            Span::styled(format!("  {:<34}", row.label), theme.muted()),
            Span::styled(value, Style::new().fg(theme.p.fg)),
        ];
        if row.changed() {
            spans.push(Span::styled("  ✎", theme.accent()));
        }
        if !row.live {
            spans.push(Span::styled("  restart", theme.muted()));
        }
        items.push(ListItem::new(Line::from(spans)));
        index_map.push(Some(i));
    }
    (items, index_map)
}

/// How images are drawn here and, without graphics, why (details:
/// `sentri terminal`).
fn images_line<'a>(state: &State, theme: &Theme) -> Line<'a> {
    let d = state.graphics.diagnosis();
    let mut spans = vec![
        Span::styled("Images   ", theme.muted()),
        Span::styled(
            d.active.label().to_owned(),
            Style::new().fg(theme.p.aqua).add_modifier(Modifier::BOLD),
        ),
    ];
    if d.emulator != crate::terminal::Emulator::Unknown {
        spans.push(Span::styled(
            format!(" · {}", d.emulator.name()),
            theme.dim(),
        ));
    }
    if d.tmux.is_some() {
        spans.push(Span::styled(" · tmux", theme.dim()));
    }
    spans.push(Span::styled(" · details: sentri terminal", theme.muted()));
    if let Some(reason) = d.reason() {
        spans.push(Span::styled(format!(" · {reason}"), theme.muted()));
    }
    Line::from(spans)
}

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let s = &state.settings;
    let o = &state.overview;
    let title = if s.is_dirty() {
        "Settings · unsaved changes"
    } else {
        "Settings"
    };
    let block = panel(theme, title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [head, list_area, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(3),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Config   ", theme.muted()),
                Span::raw(o.config_path.as_str()),
            ]),
            Line::from(vec![
                Span::styled("Database ", theme.muted()),
                Span::raw(o.database_path.as_str()),
            ]),
            images_line(state, theme),
        ]),
        head,
    );

    let (items, index_map) = rows(s, theme);
    let selected_line = index_map.iter().position(|i| *i == Some(s.selected));
    let mut list_state = ListState::default().with_selected(selected_line);
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme.selected().fg(theme.p.yellow))
            .highlight_symbol("▌"),
        list_area,
        &mut list_state,
    );

    let row = s.rows.get(s.selected);
    let status = if s.confirm_restore {
        Line::from(Span::styled(
            "Replace config.toml with the defaults? A backup is kept. y = restore",
            theme.warn(),
        ))
    } else if let Some(error) = &s.error {
        Line::from(Span::styled(format!("✗ {error}"), theme.bad()))
    } else if let Some(input) = &s.editing {
        let x = foot.x + 2 + u16::try_from(input.cursor()).unwrap_or(0);
        frame.set_cursor_position(Position::new(x.min(foot.right().saturating_sub(1)), foot.y));
        Line::from(vec![
            Span::styled("› ", theme.accent()),
            Span::raw(input.value()),
        ])
    } else {
        Line::from(Span::styled(
            row.map(|r| r.help.clone()).unwrap_or_default(),
            theme.dim(),
        ))
    };
    frame.render_widget(
        Paragraph::new(vec![
            status,
            match row.map(|r| r.kind) {
                Some(SettingKind::List(options)) => Line::from(vec![
                    Span::styled("Options: ", theme.muted()),
                    Span::styled(options.join(", "), Style::new().fg(theme.p.aqua)),
                ]),
                _ => Line::from(Span::styled(
                    "↑↓ move · Enter edit · ←/→ change · r default · u undo all · Ctrl-s save (backup) · D restore all defaults · O rerun onboarding · shortcuts: ?",
                    theme.muted(),
                )),
            },
        ])
        .wrap(Wrap { trim: true }),
        foot,
    );
}
