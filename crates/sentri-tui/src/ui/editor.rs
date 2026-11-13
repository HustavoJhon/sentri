//! Editor overlay.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap};
use sentri_core::FieldKind;

use super::widgets::{centered, panel};
use crate::state::editor::{EditorState, Row};
use crate::theme::Theme;

fn row_line<'a>(theme: &Theme, e: &'a EditorState, row: Row) -> Line<'a> {
    let label_style = theme.muted();
    let (label, value, marker) = match row {
        Row::Field(field) => {
            let value = e.value(field);
            let marker = if e.reset.contains(&field) {
                Span::styled(" ↺ detected", theme.warn())
            } else if e.is_manual(field) {
                Span::styled(" ✎ manual", theme.accent())
            } else {
                Span::styled("", theme.muted())
            };
            let shown = if value.is_empty() { "—" } else { value };
            let shown = if matches!(field.kind(), FieldKind::Choice(_)) {
                format!("‹ {shown} ›")
            } else {
                shown.to_owned()
            };
            (field.label().to_owned(), shown, marker)
        }
        Row::Notes => (
            "Notes".into(),
            if e.notes.is_empty() {
                "—".into()
            } else {
                e.notes.clone()
            },
            Span::raw(""),
        ),
        Row::Priority => (
            "Priority".into(),
            format!("‹ {} ›", e.priority.map_or("none", |p| p.as_str())),
            Span::raw(""),
        ),
        Row::Custom(i) => (
            format!("• {}", e.custom[i].0),
            e.custom[i].1.clone(),
            Span::styled(" custom", theme.muted()),
        ),
        Row::AddCustom => ("+ Add custom field".into(), String::new(), Span::raw("")),
    };
    Line::from(vec![
        Span::styled(format!("{label:<18}"), label_style),
        Span::styled(value, Style::new().fg(theme.p.fg)),
        marker,
    ])
}

pub fn render(frame: &mut Frame, editor: &EditorState, theme: &Theme) {
    let area = frame.area();
    let popup = centered(
        area,
        area.width.saturating_sub(8).min(110),
        area.height.saturating_sub(4),
    );
    frame.render_widget(Clear, popup);
    let title = format!("Edit #{} · {}", editor.id, editor.effective.title);
    let block = panel(theme, &title, true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let [list_area, input_area, help_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .areas(inner);

    let rows = editor.rows();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(row_line(theme, editor, *row)))
        .collect();
    let mut list_state = ListState::default().with_selected(Some(editor.row.min(rows.len() - 1)));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(theme.selected().fg(theme.p.yellow))
            .highlight_symbol("▌"),
        list_area,
        &mut list_state,
    );

    let status = if editor.confirm_discard {
        Line::from(Span::styled(
            "Discard unsaved changes? y = discard · any other key = keep editing",
            theme.warn().add_modifier(Modifier::BOLD),
        ))
    } else if let Some(error) = &editor.error {
        Line::from(Span::styled(format!("✗ {error}"), theme.bad()))
    } else if let Some(input) = &editor.editing {
        let line = Line::from(vec![
            Span::styled("› ", theme.accent()),
            Span::raw(input.value()),
        ]);
        let x = input_area.x + 2 + u16::try_from(input.cursor()).unwrap_or(0);
        frame.set_cursor_position(Position::new(
            x.min(input_area.right().saturating_sub(1)),
            input_area.y,
        ));
        line
    } else if let Row::Field(field) = editor.current_row() {
        let detected = field.get(&editor.detected);
        Line::from(Span::styled(
            format!(
                "Detected: {}",
                if detected.is_empty() {
                    "—"
                } else {
                    &detected
                }
            ),
            theme.muted(),
        ))
    } else {
        Line::default()
    };
    let status = if editor.editing.is_some() && editor.error.is_some() {
        vec![
            Line::from(vec![
                Span::styled("› ", theme.accent()),
                Span::raw(
                    editor
                        .editing
                        .as_ref()
                        .map(crate::input::LineInput::value)
                        .unwrap_or_default(),
                ),
            ]),
            status,
        ]
    } else {
        vec![status]
    };
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: true }), input_area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter edit · ←/→ choose · r reset to detected · + custom field · x delete custom · Ctrl-s save · Esc cancel",
            theme.muted(),
        ))
        .wrap(Wrap { trim: true }),
        help_area,
    );
}
