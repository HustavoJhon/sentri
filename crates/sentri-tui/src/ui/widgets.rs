//! Small rendering helpers shared by every view.

use chrono::{DateTime, Utc};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Padding};
use sentri_core::{Category, InteractionState, Modality, SourceKind};

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

pub fn category_label(theme: &Theme, category: Category) -> Span<'static> {
    Span::styled(
        category.label().to_uppercase(),
        Style::new().fg(theme.category_color(category)),
    )
}

pub fn score_pill(theme: &Theme, score: i32) -> Span<'static> {
    Span::styled(
        format!("{score:>4} "),
        Style::new().fg(theme.score_color(score)),
    )
}

pub const fn modality_icon(m: Modality) -> &'static str {
    match m {
        Modality::Online => "◎ online",
        Modality::InPerson => "⌂ in person",
        Modality::Hybrid => "◐ hybrid",
        Modality::Unknown => "",
    }
}

/// Status glyphs for the set flags.
pub fn flag_glyphs(state: &InteractionState) -> String {
    [
        (state.liked, '♥'),
        (state.saved, '★'),
        (state.interesting, '!'),
        (state.applied, '✓'),
        (state.enrolled, '✎'),
        (state.completed, '◉'),
        (state.dismissed, '✗'),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, glyph)| *glyph)
    .collect()
}

/// "today", "in 3d", "2d ago", or a date when far away.
pub fn relative(date: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = (date - now).num_days();
    match days {
        0 if date >= now => "today".into(),
        0 => "earlier today".into(),
        1..=60 => format!("in {days}d"),
        -60..=-1 => format!("{}d ago", -days),
        _ => date.format("%Y-%m-%d").to_string(),
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
    fn relative_dates() {
        let now = Utc::now();
        assert_eq!(relative(now + Duration::hours(2), now), "today");
        assert_eq!(
            relative(now + Duration::days(3) + Duration::hours(1), now),
            "in 3d"
        );
        assert_eq!(
            relative(now - Duration::days(2) - Duration::hours(1), now),
            "2d ago"
        );
    }

    #[test]
    fn centers_rectangles() {
        let r = centered(Rect::new(0, 0, 100, 40), 50, 10);
        assert_eq!(r, Rect::new(25, 15, 50, 10));
        let clipped = centered(Rect::new(0, 0, 20, 5), 50, 10);
        assert_eq!(clipped, Rect::new(0, 0, 20, 5));
    }

    #[test]
    fn glyphs_for_flags() {
        let state = InteractionState {
            liked: true,
            saved: true,
            ..InteractionState::default()
        };
        assert_eq!(flag_glyphs(&state), "♥★");
    }
}
