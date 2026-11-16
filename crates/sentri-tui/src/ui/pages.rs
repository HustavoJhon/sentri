//! Non-feed pages: dashboard, sources and settings.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sentri_app::RunStatus;

use super::widgets::{panel, source_badge};
use crate::state::State;
use crate::theme::Theme;

pub fn sources(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let o = &state.overview;
    let mut lines = vec![Line::from(Span::styled(
        "Configured sources (edit [[sources]] in config.toml). Each one is independent.",
        theme.muted(),
    ))];
    lines.push(Line::default());
    if o.sources.is_empty() {
        lines.push(Line::from(Span::styled(
            "No sources configured.",
            theme.muted(),
        )));
    }
    for source in &o.sources {
        let run = o.runs.iter().find(|r| r.source_name == source.name);
        let (status, style) = match run.map(|r| r.status) {
            Some(RunStatus::Succeeded) => ("ok", theme.good()),
            Some(RunStatus::Failed) => ("failed", theme.bad()),
            Some(RunStatus::Running) => ("running", theme.warn()),
            None => ("never run", theme.muted()),
        };
        let enabled = if source.enabled {
            Span::styled("● on ", theme.good())
        } else {
            Span::styled("○ off", theme.muted())
        };
        lines.push(Line::from(vec![
            enabled,
            Span::raw("  "),
            source_badge(theme, source.kind),
            Span::raw(" "),
            Span::styled(
                format!("{:<28}", source.name),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{status:<10}"), style),
            Span::styled(
                run.map(|r| format!("seen {} · new {}", r.items_seen, r.items_new))
                    .unwrap_or_default(),
                theme.dim(),
            ),
        ]));
        if let Some(error) = run.and_then(|r| r.error.as_deref()) {
            lines.push(Line::from(Span::styled(
                format!("      {error}"),
                theme.bad(),
            )));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Reddit, LinkedIn and X cannot be collected automatically (see docs/sources.md); add their links with n.",
        theme.muted(),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(theme, "Sources", true)),
        area,
    );
}
