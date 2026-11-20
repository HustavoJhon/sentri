//! Small rendering helpers shared by every view.

use chrono::{DateTime, Local, NaiveDate, Utc};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Padding};
use sentri_core::SourceKind;

use crate::theme::Theme;

/// Rounded panel with an optional title.
pub fn panel<'a>(theme: &Theme, title: &'a str, focused: bool) -> Block<'a> {
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(focused))
        .style(theme.panel())
        .padding(Padding::horizontal(1));
    if !title.is_empty() {
        block = block.title(Span::styled(format!(" {title} "), theme.title()));
    }
    block
}

/// ` GH ` style badge.
pub fn badge(theme: &Theme, text: &str, color: Color) -> Span<'static> {
    Span::styled(format!(" {text} "), theme.badge(color))
}

pub fn source_badge(theme: &Theme, kind: SourceKind) -> Span<'static> {
    badge(theme, kind.badge(), theme.source_color(kind))
}

pub fn score_pill(theme: &Theme, score: i32) -> Span<'static> {
    Span::styled(
        format!("{score:>4} "),
        Style::new().fg(theme.score_color(score)),
    )
}

/// "today", "tomorrow", "in 3d", "2d ago", or a date when far away.
/// Days are calendar days in local time (all-day dates keep their own day),
/// so 08:00 tomorrow is "tomorrow" even when it is less than 24 hours away.
pub fn relative(date: DateTime<Utc>, now: DateTime<Utc>) -> String {
    relative_in(date, now, |d| d.with_timezone(&Local).date_naive())
}

fn relative_in(
    date: DateTime<Utc>,
    now: DateTime<Utc>,
    local: impl Fn(DateTime<Utc>) -> NaiveDate,
) -> String {
    let day = sentri_core::dates::calendar_date(date, &local);
    let days = (day - local(now)).num_days();
    match days {
        0 if date >= now || sentri_core::dates::is_all_day(date) => "today".into(),
        0 => "earlier today".into(),
        1 => "tomorrow".into(),
        -1 => "yesterday".into(),
        2..=60 => format!("in {days}d"),
        -60..=-2 => format!("{}d ago", -days),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

/// Like [`relative`], but always a distance ("in 90d", "71d ago"), for
/// places where the full date is already shown or not needed.
pub fn distance(date: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = (date - now).num_days();
    match days {
        -60..=60 => relative(date, now),
        61.. => format!("in {days}d"),
        _ => format!("{}d ago", -days),
    }
}

/// Deadline urgency color.
pub fn deadline_style(theme: &Theme, date: DateTime<Utc>, now: DateTime<Utc>) -> Style {
    let days = (date - now).num_days();
    Style::new().fg(if date < now {
        theme.p.gray
    } else if days <= 3 {
        theme.p.red
    } else if days <= 14 {
        theme.p.orange
    } else {
        theme.p.fg4
    })
}

/// A rectangle of at most `width`×`height` centered in `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(row);
    cell
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn relative_dates_count_calendar_days() {
        use chrono::TimeZone;
        let utc = |d: DateTime<Utc>| d.date_naive();
        let at = |d: u32, h: u32, m: u32| Utc.with_ymd_and_hms(2026, 11, d, h, m, 1).unwrap();
        let now = at(20, 17, 0);
        assert_eq!(relative_in(at(20, 20, 0), now, utc), "today");
        assert_eq!(relative_in(at(20, 9, 0), now, utc), "earlier today");
        assert_eq!(
            relative_in(at(21, 8, 0), now, utc),
            "tomorrow",
            "15 hours away, but tomorrow"
        );
        assert_eq!(relative_in(at(19, 23, 0), now, utc), "yesterday");
        assert_eq!(relative_in(at(23, 1, 0), now, utc), "in 3d");
        assert_eq!(relative_in(at(18, 23, 0), now, utc), "2d ago");
        // All-day dates (midnight UTC) are today all day long.
        let all_day = Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap();
        assert_eq!(relative_in(all_day, now, utc), "today");
    }

    #[test]
    fn distances_never_turn_into_dates() {
        let now = Utc::now();
        assert_eq!(
            distance(now - Duration::days(71) - Duration::hours(1), now),
            "71d ago"
        );
        assert_eq!(
            distance(now + Duration::days(90) + Duration::hours(1), now),
            "in 90d"
        );
        assert_eq!(distance(now + Duration::days(3), now), "in 3d");
    }

    #[test]
    fn centers_rectangles() {
        let r = centered(Rect::new(0, 0, 100, 40), 50, 10);
        assert_eq!(r, Rect::new(25, 15, 50, 10));
        let clipped = centered(Rect::new(0, 0, 20, 5), 50, 10);
        assert_eq!(clipped, Rect::new(0, 0, 20, 5));
    }
}

/// A thin progress bar: `━` for the done part, `─` for the rest.
pub fn progress_bar(theme: &Theme, ratio: f64, width: u16) -> Vec<Span<'static>> {
    let width = usize::from(width);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let done = ((ratio.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    vec![
        Span::styled("━".repeat(done), Style::new().fg(theme.p.yellow)),
        Span::styled("─".repeat(width - done), theme.muted()),
    ]
}

/// Characters that change how wide a terminal draws the previous glyph
/// without changing the width ratatui computes. `🛠️` (U+1F6E0 + VS16) is
/// laid out as one column but most terminals draw it as two, which shifts
/// the rest of the row and breaks panel borders. Dropping the selector keeps
/// the symbol and makes both agree.
const PRESENTATION_SELECTORS: [char; 2] = ['\u{FE0F}', '\u{FE0E}'];

/// Removes emoji presentation selectors from every cell of the frame.
pub fn normalize_widths(buf: &mut ratatui::buffer::Buffer) {
    for cell in &mut buf.content {
        let symbol = cell.symbol();
        if symbol.contains(PRESENTATION_SELECTORS) {
            let cleaned: String = symbol
                .chars()
                .filter(|c| !PRESENTATION_SELECTORS.contains(c))
                .collect();
            cell.set_symbol(if cleaned.is_empty() { " " } else { &cleaned });
        }
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn emoji_selectors_no_longer_widen_cells() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        buf.set_string(0, 0, "a\u{1F6E0}\u{FE0F}b", Style::new());
        normalize_widths(&mut buf);
        let row: Vec<&str> = buf
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        // ratatui reserves two columns; the plain symbol and a blank fill
        // them on every terminal, whatever width it gives the selector.
        assert_eq!(row[..4], ["a", "\u{1F6E0}", " ", "b"]);
    }
}
