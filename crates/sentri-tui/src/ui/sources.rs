//! Sources manager: status of every source and the add/edit form.

use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};

use super::widgets::{centered, panel, relative, source_badge};
use crate::state::State;
use crate::state::sources::{FORM_ROWS, FormRow, SourceForm};
use crate::theme::Theme;

fn source_items<'a>(
    state: &'a State,
    theme: &Theme,
    now: chrono::DateTime<Utc>,
) -> Vec<ListItem<'a>> {
    let o = &state.overview;
    o.config
        .sources
        .iter()
        .map(|source| {
            let st = o.states.iter().find(|s| s.source_name == source.name);
            let (status, style) = match st {
                Some(s) if s.backing_off(now) => ("waiting", theme.warn()),
                Some(s) if s.consecutive_failures > 0 => ("failing", theme.bad()),
                Some(s) if s.last_success_at.is_some() => ("ok", theme.good()),
                _ => ("never run", theme.muted()),
            };
            let enabled = if source.enabled {
                Span::styled("● ", theme.good())
            } else {
                Span::styled("○ ", theme.muted())
            };
            let content = if source.content == sentri_core::ContentKind::News {
                "news"
            } else {
                "opportunities"
            };
            let last = st
                .and_then(|s| s.last_success_at)
                .map(|t| format!("ok {}", relative(t, now)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                enabled,
                source_badge(theme, source.kind),
                Span::raw(" "),
                Span::styled(
                    format!("{:<26}", source.name),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{content:<14}"), theme.dim()),
                Span::styled(format!("{status:<10}"), style),
                Span::styled(last, theme.muted()),
            ]))
        })
        .collect()
}

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let o = &state.overview;
    let now = Utc::now();
    let block = panel(theme, "Sources", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list_area, detail, help] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(inner);

    let items = source_items(state, theme, now);
    let mut list_state = ListState::default().with_selected(Some(state.sources.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme.selected().fg(theme.p.yellow))
            .highlight_symbol("▌"),
        list_area,
        &mut list_state,
    );

    let selected = o.config.sources.get(state.sources.selected);
    let detail_lines = selected.map_or_else(Vec::new, |source| {
        let st = o.states.iter().find(|s| s.source_name == source.name);
        let mut lines = vec![Line::from(vec![
            Span::styled(format!("{} ", source.kind.label()), theme.dim()),
            Span::styled(source.url.clone().unwrap_or_default(), theme.muted()),
        ])];
        if let Some(s) = st.filter(|s| s.last_error.is_some()) {
            let when = s
                .last_error_at
                .map(|t| relative(t, now))
                .unwrap_or_default();
            lines.push(Line::from(Span::styled(
                format!(
                    "Last error ({when}, {} in a row): {}",
                    s.consecutive_failures,
                    s.last_error.as_deref().unwrap_or_default()
                ),
                if s.consecutive_failures > 0 {
                    theme.bad()
                } else {
                    theme.muted()
                },
            )));
        }
        lines
    });
    frame.render_widget(
        Paragraph::new(detail_lines).wrap(Wrap { trim: true }),
        detail,
    );
    let footer = if let Some(name) = &state.sources.confirm_delete {
        Line::from(Span::styled(
            format!("Delete source {name}? Its items stay in your feed. y = delete"),
            theme.warn(),
        ))
    } else {
        Line::from(Span::styled(
            "+ add feed/calendar · e edit · space enable/disable · u refresh now · x delete · Reddit, LinkedIn, X: add posts with n",
            theme.muted(),
        ))
    };
    frame.render_widget(Paragraph::new(footer).wrap(Wrap { trim: true }), help);

    if let Some(form) = &state.sources.form {
        draw_form(frame, area, form, theme);
    }
}

fn draw_form(frame: &mut Frame, area: Rect, form: &SourceForm, theme: &Theme) {
    let popup = centered(area, 84, 14);
    frame.render_widget(Clear, popup);
    let title = if form.original.is_some() {
        "Edit source"
    } else {
        "Add source"
    };
    let block = panel(theme, title, true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let d = &form.draft;
    let mut lines = Vec::new();
    for (i, row) in FORM_ROWS.iter().enumerate() {
        let (label, value) = match row {
            FormRow::Name => ("Name", d.name.clone()),
            FormRow::Kind => ("Kind", format!("‹ {} ›", d.kind)),
            FormRow::Url => ("Feed / calendar URL", d.url.clone()),
            FormRow::Content => (
                "Shows in",
                format!(
                    "‹ {} ›",
                    if d.content == "news" {
                        "Tech News"
                    } else {
                        "Discover"
                    }
                ),
            ),
            FormRow::Category => (
                "Default category",
                if d.category.is_empty() {
                    "—".into()
                } else {
                    d.category.clone()
                },
            ),
            FormRow::Enabled => (
                "Enabled",
                if d.enabled {
                    "● on".into()
                } else {
                    "○ off".into()
                },
            ),
        };
        let selected = i == form.row;
        lines.push(Line::from(vec![
            Span::styled(if selected { "▌" } else { " " }, theme.accent()),
            Span::styled(format!("{label:<22}"), theme.muted()),
            Span::styled(
                value,
                if selected {
                    theme.selected()
                } else {
                    Style::new()
                },
            ),
        ]));
    }
    lines.push(Line::default());
    if let Some(input) = &form.editing {
        let y = inner.y + u16::try_from(lines.len()).unwrap_or(0);
        lines.push(Line::from(vec![
            Span::styled("› ", theme.accent()),
            Span::raw(input.value()),
        ]));
        let x = inner.x + 2 + u16::try_from(input.cursor()).unwrap_or(0);
        frame.set_cursor_position(Position::new(x.min(inner.right().saturating_sub(1)), y));
    } else if let Some(error) = &form.error {
        lines.push(Line::from(Span::styled(format!("✗ {error}"), theme.bad())));
    } else {
        lines.push(Line::from(Span::styled(
            "RSS/Atom, iCal (.ics / webcal://), Luma, Meetup, Mastodon, Bluesky, YouTube, Google News.",
            theme.dim(),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Enter edit · ←/→ change · Ctrl-s test & save (backup) · Esc cancel",
        theme.muted(),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
