//! The opportunity and news list, rendered as cards in three densities.

use chrono::{DateTime, Duration, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use sentri_app::Ranked;
use sentri_core::{Category, ContentKind, Opportunity, Stance};

use super::widgets::{
    badge, category_label, deadline_style, flag_glyphs, modality_icon, panel, relative, score_pill,
    source_badge,
};
use crate::state::{Density, State};
use crate::theme::Theme;

/// Rows used by each card for a density (including the spacer).
pub const fn item_height(density: Density) -> u16 {
    match density {
        Density::Compact => 1,
        Density::Comfortable => 3,
        Density::Detailed => 5,
    }
}

/// Discovered in the last day and never opened.
pub fn is_new(r: &Ranked, now: DateTime<Utc>) -> bool {
    r.item.state.last_viewed_at.is_none()
        && now - r.item.opportunity.discovered_at < Duration::hours(24)
}

/// Categories and perks that deserve a visual accent.
fn highlight(theme: &Theme, o: &Opportunity) -> Option<Color> {
    if o.content == ContentKind::News {
        return None;
    }
    let important = matches!(
        o.category,
        Category::Hackathon
            | Category::Scholarship
            | Category::Fellowship
            | Category::Exchange
            | Category::Job
            | Category::Internship
    ) || o.perks.offers_travel()
        || o.perks.prizes.is_some()
        || (o.category.is_learning() && o.cost.is_free());
    important.then(|| theme.category_color(o.category))
}

pub const fn stance_color(theme: &Theme, stance: Stance) -> Color {
    match stance {
        Stance::Report => theme.p.fg4,
        Stance::Official => theme.p.green,
        Stance::Speculation => theme.p.orange,
        Stance::Opinion => theme.p.purple,
        Stance::Discussion => theme.p.blue,
    }
}

fn header<'a>(theme: &Theme, r: &'a Ranked, now: DateTime<Utc>) -> Line<'a> {
    let o = &r.item.opportunity;
    let accent = highlight(theme, o)
        .map_or_else(|| Span::raw(" "), |c| Span::styled("◆", Style::new().fg(c)));
    let title_style = if r.item.state.dismissed {
        theme.muted().add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD)
    };
    let mut spans = vec![
        accent,
        score_pill(theme, r.relevance.score),
        source_badge(theme, o.source),
        Span::raw(" "),
    ];
    if o.content == ContentKind::News {
        let stance = o.stance.unwrap_or(Stance::Report);
        spans.push(Span::styled(
            stance.label().to_uppercase(),
            Style::new().fg(stance_color(theme, stance)),
        ));
    } else {
        spans.push(category_label(theme, o.category));
    }
    spans.push(Span::raw("  "));
    if is_new(r, now) {
        spans.push(badge(theme, "NEW", theme.p.green));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(o.title.as_str(), title_style));
    let glyphs = flag_glyphs(&r.item.state);
    if !glyphs.is_empty() {
        spans.push(Span::styled(format!("  {glyphs}"), theme.accent()));
    }
    Line::from(spans)
}

fn news_parts<'a>(theme: &Theme, o: &'a Opportunity, now: DateTime<Utc>) -> Vec<Span<'a>> {
    let mut parts: Vec<Span> = Vec::new();
    if let Some(topic) = o.news_topic() {
        parts.push(Span::styled(topic, Style::new().fg(theme.p.aqua)));
    }
    parts.push(Span::styled(o.source_name.as_str(), theme.dim()));
    if let Some(author) = o.author.display() {
        parts.push(Span::styled(format!("by {author}"), theme.dim()));
    }
    let published = o.dates.published_at.unwrap_or(o.discovered_at);
    parts.push(Span::styled(relative(published, now), theme.muted()));
    parts
}

fn meta_parts<'a>(theme: &Theme, o: &'a Opportunity, now: DateTime<Utc>) -> Vec<Span<'a>> {
    if o.content == ContentKind::News {
        return news_parts(theme, o, now);
    }
    let mut parts: Vec<Span> = Vec::new();
    if let Some(deadline) = o.dates.deadline {
        parts.push(Span::styled(
            format!("⏳ {}", relative(deadline, now)),
            deadline_style(theme, deadline, now),
        ));
    } else if let Some(start) = o.dates.starts_at {
        parts.push(Span::styled(
            format!("◷ {} · {}", short_date(start, now), relative(start, now)),
            Style::new().fg(theme.p.blue),
        ));
    } else if o.category.is_job()
        && let Some(published) = o.dates.published_at
    {
        parts.push(Span::styled(
            format!("posted {}", relative(published, now)),
            theme.muted(),
        ));
    }
    if let Some(org) = o.author.display() {
        parts.push(Span::styled(org, Style::new().fg(theme.p.aqua)));
    }
    if let Some(location) = &o.location {
        parts.push(Span::styled(location.as_str(), theme.dim()));
    }
    let modality = modality_icon(o.modality);
    if !modality.is_empty() {
        parts.push(Span::styled(modality, theme.dim()));
    }
    if o.cost.is_free() {
        parts.push(Span::styled("free", theme.good()));
    }
    if let Some(salary) = o
        .perks
        .benefits
        .as_deref()
        .filter(|b| o.category.is_job() && b.starts_with("USD"))
    {
        parts.push(Span::styled(salary, theme.good()));
    }
    if o.perks.prizes.is_some() {
        parts.push(Span::styled("$ prizes", Style::new().fg(theme.p.yellow)));
    }
    if o.perks.offers_travel() {
        parts.push(Span::styled("✈ travel", Style::new().fg(theme.p.purple)));
    }
    if parts.is_empty() {
        parts.push(Span::styled(o.source_name.as_str(), theme.muted()));
    }
    parts
}

/// `Fri 20 Nov`, or `20 Nov 2027` outside the current year. All-day dates
/// keep their calendar day; instants use local time.
fn short_date(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    use chrono::Datelike;
    let day =
        sentri_core::dates::calendar_date(at, |d| d.with_timezone(&chrono::Local).date_naive());
    if day.year() == now.with_timezone(&chrono::Local).year() {
        day.format("%a %d %b").to_string()
    } else {
        day.format("%d %b %Y").to_string()
    }
}

fn joined<'a>(indent: &'a str, parts: Vec<Span<'a>>, theme: &Theme) -> Line<'a> {
    let mut spans = vec![Span::raw(indent)];
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme.muted()));
        }
        spans.push(part);
    }
    Line::from(spans)
}

fn card<'a>(
    theme: &Theme,
    r: &'a Ranked,
    now: DateTime<Utc>,
    density: Density,
    width: u16,
) -> ListItem<'a> {
    let o = &r.item.opportunity;
    let indent = "        ";
    match density {
        Density::Compact => {
            let mut line = header(theme, r, now);
            if let Some(first) = meta_parts(theme, o, now).into_iter().next() {
                line.spans.push(Span::styled("  ", theme.muted()));
                line.spans.push(first);
            }
            ListItem::new(line)
        }
        Density::Comfortable => ListItem::new(vec![
            header(theme, r, now),
            joined(indent, meta_parts(theme, o, now), theme),
            Line::default(),
        ]),
        Density::Detailed => {
            let summary_width = usize::from(width.saturating_sub(12)).max(10);
            let summary: String = o
                .summary
                .as_deref()
                .unwrap_or("No description.")
                .chars()
                .take(summary_width * 2)
                .collect();
            let (first, second) = split_at_width(&summary, summary_width);
            ListItem::new(vec![
                header(theme, r, now),
                Line::from(vec![Span::raw(indent), Span::styled(first, theme.dim())]),
                Line::from(vec![Span::raw(indent), Span::styled(second, theme.dim())]),
                joined(indent, meta_parts(theme, o, now), theme),
                Line::default(),
            ])
        }
    }
}

/// Splits text into two lines at a word boundary near `width` characters.
fn split_at_width(text: &str, width: usize) -> (String, String) {
    if text.chars().count() <= width {
        return (text.to_owned(), String::new());
    }
    let cut: String = text.chars().take(width).collect();
    let at = cut.rfind(' ').unwrap_or(cut.len());
    let (a, b) = text.split_at(at);
    (a.to_owned(), b.trim_start().to_owned())
}

/// Renders the list and stores scroll/hit information for mouse support.
pub fn render(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme, title: &str) {
    let focused = state.focus == crate::state::Focus::List;
    let count = state.items.len();
    let heading = if count == 0 {
        title.to_owned()
    } else {
        format!(
            "{title} · {}/{count} · {}",
            state.selected + 1,
            state.density.label()
        )
    };
    let block = panel(theme, &heading, focused);

    if state.items.is_empty() {
        let text = if state.loading {
            "Loading…".to_owned()
        } else if state.query.text.is_some() {
            "Nothing matches this search. Esc clears it.".to_owned()
        } else {
            "No opportunities here yet.\n\nPress n to add a link you found, R to collect from sources, or : for the command palette.".to_owned()
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(theme.muted())
                .wrap(Wrap { trim: true })
                .block(block),
            area,
        );
        state.hit.list = area;
        return;
    }

    let now = Utc::now();
    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|r| card(theme, r, now, state.density, area.width))
        .collect();
    let list = List::new(items)
        .block(block)
        .highlight_style(theme.selected())
        .highlight_symbol("▌");
    let mut list_state = ListState::default()
        .with_offset(state.hit.list_offset)
        .with_selected(Some(state.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
    state.hit.list = area;
    state.hit.item_height = item_height(state.density);
    state.hit.list_offset = list_state.offset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_summaries_on_words() {
        let (a, b) = split_at_width("one two three four", 9);
        assert_eq!(a, "one two");
        assert_eq!(b, "three four");
        assert_eq!(split_at_width("short", 10), ("short".into(), String::new()));
    }
}
