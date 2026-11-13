//! The opportunity list: two lines per item for quick scanning.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use sentri_app::Ranked;

use super::widgets::{
    category_label, deadline_style, flag_glyphs, modality_icon, panel, relative, score_pill,
    source_badge,
};
use crate::state::State;
use crate::theme::Theme;

/// Rows used by each item (two text lines and a spacer).
pub const ITEM_HEIGHT: u16 = 3;

fn item_lines<'a>(theme: &Theme, r: &'a Ranked, now: DateTime<Utc>, width: u16) -> ListItem<'a> {
    let o = &r.item.opportunity;
    let dim = r.item.state.dismissed;
    let title_style = if dim {
        theme.muted().add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD)
    };
    let first = Line::from(vec![
        score_pill(theme, r.relevance.score),
        source_badge(theme, o.source),
        Span::raw(" "),
        category_label(theme, o.category),
        Span::raw("  "),
        Span::styled(o.title.as_str(), title_style),
    ]);

    let mut parts: Vec<Span> = Vec::new();
    let mut push = |span: Span<'a>| parts.push(span);
    if let Some(deadline) = o.dates.deadline {
        push(Span::styled(
            format!("⏳ {}", relative(deadline, now)),
            deadline_style(theme, deadline, now),
        ));
    } else if let Some(start) = o.dates.starts_at {
        push(Span::styled(
            format!("◷ {}", relative(start, now)),
            theme.dim(),
        ));
    }
    if let Some(location) = &o.location {
        push(Span::styled(location.as_str(), theme.dim()));
    }
    let modality = modality_icon(o.modality);
    if !modality.is_empty() {
        push(Span::styled(modality, theme.dim()));
    }
    if o.cost.is_free() {
        push(Span::styled("free", theme.good()));
    }
    if o.perks.offers_travel() {
        push(Span::styled("✈ travel", Style::new().fg(theme.p.purple)));
    }
    let glyphs = flag_glyphs(&r.item.state);
    if !glyphs.is_empty() {
        push(Span::styled(glyphs, theme.accent()));
    }
    if parts.is_empty() && width > 40 {
        parts.push(Span::styled(o.source_name.as_str(), theme.muted()));
    }
    let mut meta = vec![Span::raw("      ")];
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            meta.push(Span::styled(" · ", theme.muted()));
        }
        meta.push(part);
    }
    ListItem::new(vec![first, Line::from(meta), Line::default()])
}

/// Renders the list and stores scroll/hit information for mouse support.
pub fn render(frame: &mut Frame, area: Rect, state: &mut State, theme: &Theme, title: &str) {
    let focused = state.focus == crate::state::Focus::List;
    let count = state.items.len();
    let heading = if count == 0 {
        title.to_owned()
    } else {
        format!("{title} · {}/{count}", state.selected + 1)
    };
    let block = panel(theme, &heading, focused);

    if state.items.is_empty() {
        let text = if state.loading {
            "Loading…".to_owned()
        } else if state.query.text.is_some() {
            "Nothing matches this search. Esc clears it.".to_owned()
        } else {
            "No opportunities here yet.\n\nPress n to add a link you found, or : for the command palette.".to_owned()
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
        .map(|r| item_lines(theme, r, now, area.width))
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
    state.hit.item_height = ITEM_HEIGHT;
    state.hit.list_offset = list_state.offset();
}
