//! Calendar views inspired by Google Calendar: month grid, week columns,
//! day agenda and an upcoming list. Scheduled entries are solid; dated
//! opportunities not yet added appear dimmed as suggestions.

use chrono::{Datelike, Duration, NaiveDate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use sentri_core::EntryKind;

use super::widgets::panel;
use crate::state::calendar::{
    CalInput, CalItem, CalView, CalendarState, first_of_month, week_start_on,
};
use crate::theme::Theme;

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WEEKDAYS_SUNDAY: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const fn weekdays(c: &CalendarState) -> [&'static str; 7] {
    if c.sunday_first {
        WEEKDAYS_SUNDAY
    } else {
        WEEKDAYS
    }
}

fn item_color(theme: &Theme, item: &CalItem) -> Color {
    match (item.kind, item.category) {
        (EntryKind::Deadline, _) => theme.p.orange,
        (EntryKind::Reminder, _) => theme.p.yellow,
        (EntryKind::Personal, _) => theme.p.purple,
        (_, Some(category)) => theme.category_color(category),
        _ => theme.p.aqua,
    }
}

fn item_span<'a>(theme: &Theme, item: &'a CalItem, width: usize, selected: bool) -> Line<'a> {
    let color = item_color(theme, item);
    let mut style = Style::new().fg(color);
    if !item.is_scheduled() {
        style = style.add_modifier(Modifier::DIM);
    }
    if selected {
        style = style.add_modifier(Modifier::REVERSED);
    }
    let marker = if item.is_scheduled() { "▪" } else { "·" };
    let time = item
        .time
        .as_deref()
        .map(|t| format!("{t} "))
        .unwrap_or_default();
    let text: String = format!("{marker}{}{time}{}", item.kind.icon(), item.title)
        .chars()
        .take(width)
        .collect();
    Line::from(Span::styled(text, style))
}

pub fn render(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme) {
    let side_width = if area.width >= 110 { 38 } else { 0 };
    let [main, side] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(side_width)]).areas(area);
    let title = match c.view {
        CalView::Month => c.cursor.format("%B %Y").to_string(),
        CalView::Week => {
            let start = week_start_on(c.cursor, c.sunday_first);
            format!("Week of {}", start.format("%d %b %Y"))
        }
        CalView::Day => c.cursor.format("%A %d %B %Y").to_string(),
        CalView::Agenda => format!("Agenda from {}", c.cursor.format("%d %b %Y")),
    };
    let heading = format!("▦ {title} · {} view", c.view.label());
    let block = panel(theme, &heading, true).title_bottom(Line::from(Span::styled(
        " Google Calendar: not connected ",
        theme.muted(),
    )));
    let inner = block.inner(main);
    frame.render_widget(block, main);
    let [body, help] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    match c.view {
        CalView::Month => month(frame, body, c, theme),
        CalView::Week => week(frame, body, c, theme),
        CalView::Day => day(frame, body, c, theme, c.cursor),
        CalView::Agenda => agenda(frame, body, c, theme),
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            "←→ day · ↑↓ week · < > period · t today · v view · J/K pick · Enter open · a add · + new · e edit · x delete · g sync",
            theme.muted(),
        )),
        help,
    );
    if side_width > 0 {
        let block = panel(theme, "Selected day", false);
        let inner = block.inner(side);
        frame.render_widget(block, side);
        day(frame, inner, c, theme, c.cursor);
    }
    draw_input(frame, area, c, theme);
}

fn month(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme) {
    let [head, grid] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let cols = Layout::horizontal(vec![Constraint::Fill(1); 7]).split(head);
    for (i, name) in weekdays(c).iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Span::styled(*name, theme.muted())).centered(),
            cols[i],
        );
    }
    let start = week_start_on(first_of_month(c.cursor), c.sunday_first);
    let rows = Layout::vertical(vec![Constraint::Fill(1); 6]).split(grid);
    for (w, row) in rows.iter().enumerate() {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); 7]).split(*row);
        for (d, cell) in cells.iter().enumerate() {
            let date = start + Duration::days(i64::try_from(w * 7 + d).unwrap_or(0));
            day_cell(frame, *cell, c, theme, date);
        }
    }
}

fn day_cell(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme, date: NaiveDate) {
    let in_month = date.month() == c.cursor.month();
    let is_cursor = date == c.cursor;
    let is_today = date == c.today;
    let border = if is_cursor {
        Style::new().fg(theme.p.yellow)
    } else {
        Style::new().fg(theme.p.bg2)
    };
    let block = Block::new()
        .borders(Borders::TOP | Borders::LEFT)
        .border_style(border);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let number_style = match (is_today, in_month) {
        (true, _) => theme.badge(theme.p.orange),
        (false, true) => Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD),
        (false, false) => theme.muted(),
    };
    let items = c.items_on(date);
    let capacity = usize::from(inner.height.saturating_sub(1));
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:>2}", date.day()), number_style),
        Span::styled(
            if items.len() > 1 {
                format!(" {}●", items.len())
            } else {
                String::new()
            },
            theme.muted(),
        ),
    ])];
    let width = usize::from(inner.width);
    for (i, item) in items.iter().enumerate() {
        if i + 1 == capacity && items.len() > capacity {
            lines.push(Line::from(Span::styled(
                format!("+{} more", items.len() - i),
                theme.muted(),
            )));
            break;
        }
        if i >= capacity {
            break;
        }
        lines.push(item_span(theme, item, width, is_cursor && i == c.selected));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn week(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme) {
    let start = week_start_on(c.cursor, c.sunday_first);
    let cols = Layout::horizontal(vec![Constraint::Fill(1); 7]).split(area);
    for (i, col) in cols.iter().enumerate() {
        let date = start + Duration::days(i64::try_from(i).unwrap_or(0));
        let selected = date == c.cursor;
        let style = if date == c.today {
            theme.badge(theme.p.orange)
        } else if selected {
            Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD)
        } else {
            theme.muted()
        };
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{} {}", weekdays(c)[i], date.day()),
                style,
            )),
            Line::default(),
        ];
        let width = usize::from(col.width.saturating_sub(1));
        for (n, item) in c.items_on(date).iter().enumerate() {
            lines.push(item_span(theme, item, width, selected && n == c.selected));
        }
        let border = if selected {
            theme.border(true)
        } else {
            Style::new().fg(theme.p.bg2)
        };
        frame.render_widget(
            Paragraph::new(lines).block(Block::new().borders(Borders::LEFT).border_style(border)),
            *col,
        );
    }
}

fn day(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme, date: NaiveDate) {
    let items = c.items_on(date);
    let mut lines = vec![
        Line::from(Span::styled(
            date.format("%A %d %B").to_string(),
            theme.title(),
        )),
        Line::default(),
    ];
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nothing this day. Press + to add an entry.",
            theme.muted(),
        )));
    }
    for (i, item) in items.iter().enumerate() {
        let selected = i == c.selected;
        let time = item.time.clone().unwrap_or_else(|| "all day".into());
        lines.push(Line::from(vec![
            Span::styled(if selected { "▌" } else { " " }, theme.accent()),
            Span::styled(format!("{time:<8}"), theme.muted()),
            Span::styled(
                format!("{} {}", item.kind.icon(), item.kind.label()),
                Style::new().fg(item_color(theme, item)),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                item.title.as_str(),
                if selected {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                },
            ),
        ]));
        if !item.is_scheduled() {
            lines.push(Line::from(Span::styled(
                "  suggestion · a adds it to your calendar",
                theme.muted(),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn agenda(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme) {
    let mut lines = Vec::new();
    let mut last: Option<NaiveDate> = None;
    for (i, item) in c.agenda().iter().enumerate() {
        if last != Some(item.date) {
            lines.push(Line::from(Span::styled(
                item.date.format("%a %d %b").to_string(),
                theme.title(),
            )));
            last = Some(item.date);
        }
        let mut line = item_span(
            theme,
            item,
            usize::from(area.width).saturating_sub(4),
            i == c.selected,
        );
        line.spans.insert(0, Span::raw("  "));
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No upcoming dates. Add opportunities with a, or entries with +.",
            theme.muted(),
        )));
    }
    let skip = c.selected.saturating_sub(usize::from(area.height) / 2);
    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(skip).unwrap_or(0), 0)),
        area,
    );
}

fn draw_input(frame: &mut Frame, area: Rect, c: &CalendarState, theme: &Theme) {
    let (title, input) = match &c.input {
        Some(CalInput::New(i)) => ("New entry", i),
        Some(CalInput::Edit(_, i)) => ("Edit entry", i),
        None => {
            if c.confirm_delete.is_some() {
                let popup = super::widgets::centered(area, 60, 3);
                frame.render_widget(ratatui::widgets::Clear, popup);
                frame.render_widget(
                    Paragraph::new(Span::styled(
                        "Delete this SENTRI entry? y = delete · any key = cancel",
                        theme.warn(),
                    ))
                    .block(panel(theme, "Confirm", true)),
                    popup,
                );
            }
            return;
        }
    };
    let popup = super::widgets::centered(area, 70, 5);
    frame.render_widget(ratatui::widgets::Clear, popup);
    let block = panel(theme, title, true);
    let inner = block.inner(popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("› ", theme.accent()),
                Span::raw(input.value()),
            ]),
            Line::from(Span::styled(
                format!(
                    "On {} · e.g. \"18:00-20:00 Rust meetup\" or \"remind Send CV\"",
                    c.cursor.format("%d %b")
                ),
                theme.muted(),
            )),
        ])
        .block(block),
        popup,
    );
    let x = inner.x + 2 + u16::try_from(input.cursor()).unwrap_or(0);
    frame.set_cursor_position(Position::new(
        x.min(inner.right().saturating_sub(1)),
        inner.y,
    ));
}
