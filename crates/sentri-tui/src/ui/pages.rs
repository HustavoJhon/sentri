//! Non-feed pages: dashboard, sources and settings.

use chrono::Utc;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sentri_app::RunStatus;

use super::widgets::{deadline_style, panel, relative, score_pill, source_badge};
use crate::state::State;
use crate::theme::Theme;

fn stat_card(frame: &mut Frame, area: Rect, theme: &Theme, label: &str, value: i64, color: Color) {
    let text = vec![
        Line::from(Span::styled(
            value.to_string(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(label, theme.muted())),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .centered()
            .block(panel(theme, "", false)),
        area,
    );
}

pub fn dashboard(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let s = &state.overview.stats;
    let [cards, body] = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(area);
    let p = theme.p;
    let entries = [
        ("opportunities", s.total, p.fg),
        ("new · 24h", s.new_since, p.green),
        ("saved", s.saved, p.yellow),
        ("liked", s.liked, p.red),
        ("applied", s.applied, p.blue),
        ("open deadlines", s.upcoming_deadlines, p.orange),
    ];
    let shown = if area.width < 90 { 3 } else { entries.len() };
    let cells = Layout::horizontal(vec![Constraint::Fill(1); shown]).split(cards);
    for ((label, value, color), cell) in entries.iter().zip(cells.iter()) {
        stat_card(frame, *cell, theme, label, *value, *color);
    }

    let [top, upcoming] = if body.width >= 100 {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(body)
    } else {
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(body)
    };
    let now = Utc::now();
    let best: Vec<Line> = state
        .items
        .iter()
        .take(usize::from(top.height.saturating_sub(2)))
        .map(|r| {
            Line::from(vec![
                score_pill(theme, r.relevance.score),
                source_badge(theme, r.item.opportunity.source),
                Span::raw(" "),
                Span::raw(r.item.opportunity.title.as_str()),
            ])
        })
        .collect();
    let best = if best.is_empty() {
        vec![Line::from(Span::styled(
            "Nothing yet — press n to add your first link.",
            theme.muted(),
        ))]
    } else {
        best
    };
    frame.render_widget(
        Paragraph::new(best).block(panel(theme, "Top recommendations", false)),
        top,
    );

    let mut dated: Vec<_> = state
        .items
        .iter()
        .filter_map(|r| {
            r.item
                .opportunity
                .dates
                .deadline
                .filter(|d| *d >= now)
                .map(|d| (d, r))
        })
        .collect();
    dated.sort_by_key(|(d, _)| *d);
    let lines: Vec<Line> = dated
        .iter()
        .take(usize::from(upcoming.height.saturating_sub(2)))
        .map(|(d, r)| {
            Line::from(vec![
                Span::styled(
                    format!("{:<9}", relative(*d, now)),
                    deadline_style(theme, *d, now),
                ),
                Span::raw(r.item.opportunity.title.as_str()),
            ])
        })
        .collect();
    let lines = if lines.is_empty() {
        vec![Line::from(Span::styled(
            "No upcoming deadlines.",
            theme.muted(),
        ))]
    } else {
        lines
    };
    frame.render_widget(
        Paragraph::new(lines).block(panel(theme, "Upcoming deadlines", false)),
        upcoming,
    );
}

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

pub fn settings(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let o = &state.overview;
    let p = &o.profile;
    let field = |name: &'static str, value: String| {
        Line::from(vec![
            Span::styled(format!("{name:<14}"), theme.muted()),
            Span::raw(value),
        ])
    };
    let none = || "—".to_owned();
    let mut lines = vec![
        field("Config", o.config_path.clone()),
        field("Database", o.database_path.clone()),
        Line::default(),
        field("City", p.home_city.clone().unwrap_or_else(none)),
        field("Country", p.country.clone().unwrap_or_else(none)),
        field(
            "Modalities",
            p.preferred_modalities
                .iter()
                .map(|m| m.label())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        field("Avoid", p.avoid_keywords.join(", ")),
        Line::default(),
        Line::from(Span::styled("Interests", theme.title())),
    ];
    for interest in &p.interests {
        let priority = match interest.priority {
            sentri_core::Priority::High => Span::styled("high  ", theme.good()),
            sentri_core::Priority::Medium => Span::styled("medium", theme.warn()),
            sentri_core::Priority::Low => Span::styled("low   ", theme.muted()),
        };
        lines.push(Line::from(vec![
            priority,
            Span::raw("  "),
            Span::styled(
                interest.name.as_str(),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {}", interest.keywords.join(", ")), theme.dim()),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Edit config.toml to change the profile, interests and scoring weights, then press r.",
        theme.muted(),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(theme, "Settings", true)),
        area,
    );
}
