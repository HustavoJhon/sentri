//! History: GitHub-style activity heatmap, statistics and day summary.

use chrono::{Datelike, Duration};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::widgets::panel;
use crate::state::State;
use crate::state::history::HistoryState;
use crate::theme::Theme;

/// Five intensity levels, from empty to most active.
fn level_color(theme: &Theme, level: usize) -> Color {
    let p = theme.p;
    match (level, p.green) {
        (0, _) => p.bg1,
        (_, Color::Rgb(..)) => [
            Color::Rgb(0x4a, 0x4d, 0x1e),
            Color::Rgb(0x6f, 0x74, 0x1f),
            Color::Rgb(0x94, 0x98, 0x22),
            p.green,
        ][level.min(4) - 1],
        _ => [
            Color::Indexed(58),
            Color::Indexed(100),
            Color::Indexed(142),
            Color::Indexed(148),
        ][level.min(4) - 1],
    }
}

/// Level 0–4 relative to the busiest day (quartiles of the maximum).
pub fn level(count: i64, max: i64) -> usize {
    if count <= 0 || max <= 0 {
        return 0;
    }
    usize::try_from((count * 4 + max - 1) / max)
        .unwrap_or(4)
        .clamp(1, 4)
}

fn heatmap(frame: &mut Frame, area: Rect, h: &HistoryState, theme: &Theme) {
    let weeks_fit = usize::from(area.width.saturating_sub(4) / 2).min(53);
    let last_monday = h.today - Duration::days(i64::from(h.today.weekday().num_days_from_monday()));
    let first = last_monday - Duration::weeks(i64::try_from(weeks_fit).unwrap_or(1) - 1);
    let max = h.max();
    let mut months = String::from("    ");
    let mut last_month = 0;
    for w in 0..weeks_fit {
        let monday = first + Duration::weeks(i64::try_from(w).unwrap_or(0));
        if monday.month() != last_month {
            last_month = monday.month();
            months.push_str(&monday.format("%b").to_string());
            months.push(' ');
        } else if months.chars().count() <= 4 + w * 2 {
            months.push_str("  ");
        }
    }
    let mut lines = vec![Line::from(Span::styled(months, theme.muted()))];
    for (row, name) in ["Mon", "", "Wed", "", "Fri", "", "Sun"].iter().enumerate() {
        let mut spans = vec![Span::styled(format!("{name:<4}"), theme.muted())];
        for w in 0..weeks_fit {
            let date = first
                + Duration::weeks(i64::try_from(w).unwrap_or(0))
                + Duration::days(i64::try_from(row).unwrap_or(0));
            if date > h.today {
                spans.push(Span::raw("  "));
                continue;
            }
            let mut style = Style::new().fg(level_color(theme, level(h.day(date), max)));
            if date == h.cursor {
                style = Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(
                if date == h.cursor { "◆ " } else { "■ " },
                style,
            ));
        }
        lines.push(Line::from(spans));
    }
    let mut legend = vec![Span::styled("    less ", theme.muted())];
    for l in 0..=4 {
        legend.push(Span::styled("■ ", Style::new().fg(level_color(theme, l))));
    }
    legend.push(Span::styled(
        "more   h/l day · H/L week · t today",
        theme.muted(),
    ));
    lines.push(Line::from(legend));
    frame.render_widget(Paragraph::new(lines), area);
}

fn stats_lines<'a>(h: &HistoryState, theme: &Theme) -> Vec<Line<'a>> {
    let s = &h.stats;
    let value =
        |v: String| Span::styled(v, Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD));
    let row = |label: &'static str, v: String| {
        Line::from(vec![
            Span::styled(format!("{label:<20}"), theme.muted()),
            value(v),
        ])
    };
    vec![
        row("Current streak", format!("{} days", s.current_streak)),
        row("Longest streak", format!("{} days", s.longest_streak)),
        row(
            "Most active day",
            s.most_active_day.map_or_else(
                || "—".into(),
                |(d, n)| format!("{} ({n})", d.format("%d %b %Y")),
            ),
        ),
        row(
            "Most active week",
            s.most_active_week.map_or_else(
                || "—".into(),
                |(d, n)| format!("of {} ({n})", d.format("%d %b")),
            ),
        ),
        row("Active days", s.active_days.to_string()),
        row("Reviewed", s.reviewed.to_string()),
        row("Saved · liked", format!("{} · {}", s.saved, s.liked)),
        row(
            "Applied · enrolled",
            format!("{} · {}", s.applied, s.enrolled),
        ),
        row("Added · edited", format!("{} · {}", s.added, s.edited)),
        row("Scheduled", s.scheduled.to_string()),
    ]
}

fn day_lines<'a>(h: &'a HistoryState, theme: &Theme) -> Vec<Line<'a>> {
    let mut lines = vec![Line::from(Span::styled(
        h.cursor.format("%A %d %B %Y").to_string(),
        theme.title(),
    ))];
    let activity = h.days.get(&h.cursor);
    if let Some(a) = activity {
        lines.push(Line::from(Span::styled(
            format!(
                "{} discovered · {} added by you",
                a.discovered, a.added_manually
            ),
            theme.dim(),
        )));
    }
    match h.entries_for_cursor() {
        None => lines.push(Line::from(Span::styled("Loading…", theme.muted()))),
        Some([]) => lines.push(Line::from(Span::styled(
            "No activity this day.",
            theme.muted(),
        ))),
        Some(entries) => {
            for e in entries.iter().filter(|e| e.action != "discovered").take(30) {
                lines.push(Line::from(vec![
                    Span::styled(format!("{:<11}", e.action), theme.accent()),
                    Span::raw(e.title.as_str()),
                ]));
            }
        }
    }
    lines
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme) {
    let h = &state.history;
    let [top, bottom] = Layout::vertical([Constraint::Length(11), Constraint::Min(0)]).areas(area);
    let [map, stats_area] = if top.width >= 140 {
        Layout::horizontal([Constraint::Min(0), Constraint::Length(42)]).areas(top)
    } else {
        [top, Rect::default()]
    };
    let block = panel(theme, "Activity · last year", false);
    let inner = block.inner(map);
    frame.render_widget(block, map);
    if h.loaded {
        heatmap(frame, inner, h, theme);
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled("Loading…", theme.muted())),
            inner,
        );
    }
    if stats_area.width > 0 {
        frame.render_widget(
            Paragraph::new(stats_lines(h, theme)).block(panel(theme, "Stats", false)),
            stats_area,
        );
    }
    let [list, day_area] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(bottom);
    let mut day_text = day_lines(&state.history, theme);
    if stats_area.width == 0 {
        day_text.push(Line::default());
        day_text.extend(stats_lines(&state.history, theme));
    }
    frame.render_widget(
        Paragraph::new(day_text).block(panel(theme, "Selected day", false)),
        day_area,
    );
    super::feed::render(frame, list, state, theme, "Recently viewed");
}

#[cfg(test)]
mod tests {
    use super::level;

    #[test]
    fn levels_scale_to_the_busiest_day() {
        assert_eq!(level(0, 10), 0);
        assert_eq!(level(1, 10), 1);
        assert_eq!(level(5, 10), 2);
        assert_eq!(level(10, 10), 4);
        assert_eq!(level(3, 0), 0);
    }
}
