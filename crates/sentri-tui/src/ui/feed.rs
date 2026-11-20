//! The opportunity and news list, rendered as cards in three densities.

use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use super::card;
use super::widgets::panel;
use crate::state::{Density, State};
use crate::theme::Theme;

/// Rows used by each card for a density (including the spacer).
pub const fn item_height(density: Density) -> u16 {
    card::rows(density)
}

/// What an empty section shows while sources are being collected.
fn collecting_lines<'a>(
    theme: &Theme,
    progress: &crate::state::collect::CollectProgress,
    title: &str,
    width: u16,
) -> Vec<Line<'a>> {
    let heading = if progress.first_run {
        "Welcome to SENTRI"
    } else {
        "Refreshing your sources"
    };
    let sources = if progress.total > 0 {
        format!("{} sources", progress.total)
    } else {
        "your sources".to_owned()
    };
    let mut bar = vec![Span::raw("  ")];
    bar.extend(super::widgets::progress_bar(
        theme,
        progress.ratio(),
        width.saturating_sub(24).clamp(8, 40),
    ));
    bar.push(Span::styled(
        format!("  {}/{}", progress.done.min(progress.total), progress.total),
        theme.warn(),
    ));
    bar.push(Span::styled(
        format!(" · +{} new", progress.new),
        theme.good(),
    ));
    let key = |k: &'static str, what: &'static str| {
        vec![
            Span::raw("  "),
            Span::styled(format!(" {k} "), theme.key()),
            Span::styled(format!(" {what}"), theme.muted()),
        ]
    };
    vec![
        Line::default(),
        Line::from(Span::styled(format!("  {heading}"), theme.title())),
        Line::default(),
        Line::from(Span::styled(
            "  Collecting events, jobs, courses and tech news",
            theme.dim(),
        )),
        Line::from(Span::styled(format!("  from {sources}."), theme.dim())),
        Line::from(Span::styled(
            format!("  {title} fills in as each one answers."),
            theme.dim(),
        )),
        Line::default(),
        Line::from(bar),
        Line::default(),
        Line::from(key("1", "dashboard")),
        Line::from(key("n", "add a link you found")),
        Line::from(key("?", "every shortcut")),
    ]
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
        if let Some(progress) = &state.collecting {
            let inner = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(collecting_lines(theme, progress, title, inner.width))
                    .wrap(Wrap { trim: false }),
                inner,
            );
            state.hit.list = area;
            return;
        }
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
    // Inside the rounded border and its one-column padding.
    let width = area.width.saturating_sub(4);
    let items: Vec<ListItem> = state
        .items
        .iter()
        .enumerate()
        .map(|(i, r)| {
            ListItem::new(card::lines(
                theme,
                r,
                card::Layout {
                    density: state.density,
                    width,
                    selected: i == state.selected,
                    now,
                    prefs: &state.cards,
                },
            ))
        })
        .collect();
    let mut list = List::new(items).block(block);
    if state.cards.highlight {
        list = list.highlight_style(theme.selected());
    }
    let mut list_state = ListState::default()
        .with_offset(state.hit.list_offset)
        .with_selected(Some(state.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
    state.hit.list = area;
    state.hit.item_height = item_height(state.density);
    state.hit.list_offset = list_state.offset();
}
