//! The dashboard: counters, time series, distributions and short lists.
//!
//! Every number comes from persisted data (`Insights`, `Stats`).

use chrono::{Local, Utc};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Paragraph, Sparkline};

use super::charts::{Scale, hbar_line, series};
use super::widgets::{deadline_style, panel, relative, score_pill, source_badge};
use crate::state::State;
use crate::theme::Theme;

const DAYS: i64 = 14;

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let compact = area.height < 34;
    let [tiles, charts, lists] = Layout::vertical([
        Constraint::Length(if area.width < 100 { 6 } else { 3 }),
        Constraint::Length(if compact { 9 } else { 12 }),
        Constraint::Min(6),
    ])
    .areas(area);
    draw_tiles(frame, tiles, state, theme);
    draw_charts(frame, charts, state, theme);
    draw_lists(frame, lists, state, theme);
}

fn draw_tiles(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let s = &state.overview.stats;
    let i = &state.overview.insights;
    let p = theme.p;
    let tiles = [
        ("✦", "found", s.total, p.fg),
        ("●", "new 24h", s.new_since, p.green),
        ("▤", "news new", i.news_new, p.aqua),
        ("◷", "events", i.upcoming_events, p.blue),
        ("✎", "courses", i.pending_courses, p.green),
        ("⚒", "jobs ★", i.interesting_jobs, p.blue),
        ("★", "saved", s.saved, p.yellow),
        ("♥", "liked", s.liked, p.red),
        ("✓", "applied", i.applied, p.purple),
        ("▦", "calendar", i.calendar_entries, p.orange),
    ];
    let rows: Vec<&[_]> = if area.width < 100 {
        vec![&tiles[..5], &tiles[5..]]
    } else {
        vec![&tiles[..]]
    };
    let row_areas = Layout::vertical(vec![Constraint::Length(3); rows.len()]).split(area);
    for (row, row_area) in rows.iter().zip(row_areas.iter()) {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); row.len()]).split(*row_area);
        for ((icon, label, value, color), cell) in row.iter().zip(cells.iter()) {
            let line = Line::from(vec![
                Span::styled(format!("{icon} "), Style::new().fg(*color)),
                Span::styled(
                    value.to_string(),
                    Style::new().fg(*color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {label}"), theme.muted()),
            ]);
            frame.render_widget(
                Paragraph::new(line)
                    .centered()
                    .block(panel(theme, "", false)),
                *cell,
            );
        }
    }
}

fn draw_charts(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let wide = area.width >= 110;
    let [left, middle, right] = if wide {
        Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .areas(area)
    } else {
        let [l, r] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(area);
        [l, r, Rect::default()]
    };
    discovered_chart(frame, left, state, theme);
    interactions_chart(frame, middle, state, theme);
    if wide {
        distribution(frame, right, state, theme);
    }
}

fn discovered_chart(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let today = Local::now().date_naive();
    let values = series(&state.overview.insights.discovered_per_day, today, DAYS);
    let bars: Vec<Bar> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let day = today - chrono::Duration::days(DAYS - 1 - i64::try_from(i).unwrap_or(0));
            Bar::default()
                .value(*v)
                .label(Line::from(day.format("%d").to_string()))
                .style(Style::new().fg(if i + 1 == values.len() {
                    theme.p.orange
                } else {
                    theme.p.yellow
                }))
                .value_style(Style::new().fg(theme.p.bg0).bg(theme.p.yellow))
        })
        .collect();
    let inner_width = area.width.saturating_sub(4);
    let bar_width = (inner_width / u16::try_from(DAYS).unwrap_or(14))
        .saturating_sub(1)
        .max(1);
    let chart = BarChart::default()
        .block(panel(theme, "Discovered · 14 days", false))
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(1)
        .label_style(theme.muted());
    frame.render_widget(chart, area);
}

fn interactions_chart(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let block = panel(theme, "Likes ♥ and saves ★ · 14 days", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let today = Local::now().date_naive();
    let i = &state.overview.insights;
    let likes = series(&i.likes_per_day, today, DAYS);
    let saves = series(&i.saves_per_day, today, DAYS);
    let [l_label, l_spark, s_label, s_spark, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    let total = |v: &[u64]| v.iter().sum::<u64>();
    frame.render_widget(
        Paragraph::new(Span::styled(format!("♥ {}", total(&likes)), theme.bad())),
        l_label,
    );
    frame.render_widget(
        Sparkline::default()
            .data(&likes)
            .style(Style::new().fg(theme.p.red)),
        l_spark,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(format!("★ {}", total(&saves)), theme.warn())),
        s_label,
    );
    frame.render_widget(
        Sparkline::default()
            .data(&saves)
            .style(Style::new().fg(theme.p.yellow)),
        s_spark,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("applied ", theme.muted()),
            Span::styled(i.applied.to_string(), Style::new().fg(theme.p.purple)),
            Span::styled(" · enrolled ", theme.muted()),
            Span::styled(i.enrolled.to_string(), Style::new().fg(theme.p.blue)),
            Span::styled(" · completed ", theme.muted()),
            Span::styled(i.completed.to_string(), theme.good()),
        ])),
        status,
    );
}

fn distribution(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let block = panel(theme, "Categories · sources · modality", false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let i = &state.overview.insights;
    let bar_width = inner.width.saturating_sub(18).max(4);
    let mut lines = Vec::new();
    let max_cat = i.by_category.first().map_or(0, |(_, n)| *n);
    for (category, n) in i.by_category.iter().take(4) {
        lines.push(hbar_line(
            Vec::new(),
            category.label(),
            *n,
            Scale {
                max: max_cat,
                bar_width,
                label_width: 12,
            },
            theme.category_color(*category),
            theme.muted(),
        ));
    }
    let max_src = i.by_source.first().map_or(0, |(_, n)| *n);
    for (source, n) in i.by_source.iter().take(4) {
        lines.push(hbar_line(
            vec![source_badge(theme, *source), Span::raw(" ")],
            "",
            *n,
            Scale {
                max: max_src,
                bar_width,
                label_width: 7,
            },
            theme.source_color(*source),
            theme.muted(),
        ));
    }
    let modalities: Vec<String> = i
        .by_modality
        .iter()
        .filter(|(m, _)| *m != sentri_core::Modality::Unknown)
        .map(|(m, n)| format!("{} {n}", m.label()))
        .collect();
    if !modalities.is_empty() {
        lines.push(Line::from(Span::styled(
            modalities.join(" · "),
            theme.dim(),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("No data yet.", theme.muted())));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_lists(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let columns: Vec<Rect> = if area.width >= 110 {
        Layout::horizontal(vec![Constraint::Fill(1); 3])
            .split(area)
            .to_vec()
    } else {
        Layout::horizontal(vec![Constraint::Fill(1); 2])
            .split(area)
            .to_vec()
    };
    let rows = |r: Rect| usize::from(r.height.saturating_sub(2));
    let now = Utc::now();

    let best: Vec<Line> = state
        .items
        .iter()
        .take(rows(columns[0]))
        .map(|r| {
            Line::from(vec![
                score_pill(theme, r.relevance.score),
                source_badge(theme, r.item.opportunity.source),
                Span::raw(" "),
                Span::raw(r.item.opportunity.title.as_str()),
            ])
        })
        .collect();
    list(
        frame,
        columns[0],
        theme,
        "Top recommendations",
        best,
        "Press n to add a link.",
    );

    let mut dated: Vec<_> = state
        .items
        .iter()
        .filter_map(|r| {
            let d = r.item.opportunity.next_date(now)?;
            Some((d, r))
        })
        .collect();
    dated.sort_by_key(|(d, _)| *d);
    let upcoming: Vec<Line> = dated
        .iter()
        .take(rows(columns[1]))
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
    list(
        frame,
        columns[1],
        theme,
        "Upcoming dates",
        upcoming,
        "No upcoming dates.",
    );

    if let Some(area) = columns.get(2) {
        let news: Vec<Line> = state
            .overview
            .news
            .iter()
            .take(rows(*area))
            .map(|r| {
                Line::from(vec![
                    source_badge(theme, r.item.opportunity.source),
                    Span::raw(" "),
                    Span::raw(r.item.opportunity.title.as_str()),
                ])
            })
            .collect();
        list(
            frame,
            *area,
            theme,
            "Latest tech news",
            news,
            "Press R to collect news.",
        );
    }
}

fn list(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, lines: Vec<Line>, empty: &str) {
    let lines = if lines.is_empty() {
        vec![Line::from(Span::styled(empty.to_owned(), theme.muted()))]
    } else {
        lines
    };
    frame.render_widget(
        Paragraph::new(lines).block(panel(theme, title, false)),
        area,
    );
}
