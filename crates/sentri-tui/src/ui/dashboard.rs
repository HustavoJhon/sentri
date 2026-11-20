//! The dashboard: metrics, time series, distributions and short lists.
//!
//! Every number comes from persisted data (`Insights`, `Stats`). Which
//! widgets appear, their order, the metrics and the layout come from
//! `[dashboard]` in the configuration (Settings → Dashboard).

use chrono::{Local, Utc};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Paragraph, Sparkline, Wrap};

use super::card;
use super::charts::{Scale, hbar_line, series};
use super::widgets::{deadline_style, panel, relative, score_pill, source_badge};
use sentri_app::Ranked;

use crate::state::State;
use crate::state::presentation::{DashboardPrefs, Widget};
use crate::theme::Theme;

const DAYS: i64 = 14;

/// Draws the configured widgets in order: consecutive charts share a row,
/// and so do consecutive lists. Rows hold as many widgets as the width
/// allows and wrap.
pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let prefs = &state.dashboard;
    // Short terminals get the compact layout whatever the setting says.
    let compact = prefs.compact || area.height < 24;
    let rows = rows(&prefs.widgets, area.width);
    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Every dashboard widget is hidden. Settings → Dashboard → Widgets brings them back.",
                theme.muted(),
            )))
            .wrap(Wrap { trim: true })
            .block(panel(theme, "Dashboard", false)),
            area,
        );
        return;
    }
    let tall = area.height >= 34;
    let constraints: Vec<Constraint> = rows
        .iter()
        .map(|row| match row.first() {
            Some(Widget::Metrics) if compact => Constraint::Length(1),
            Some(Widget::Metrics) => Constraint::Length(metric_rows(prefs, area.width) * 4),
            Some(w) if w.is_chart() && compact => Constraint::Length(7),
            Some(w) if w.is_chart() && tall => Constraint::Length(12),
            Some(w) if w.is_chart() => Constraint::Length(9),
            _ => Constraint::Min(4),
        })
        .collect();
    let areas = Layout::vertical(constraints).split(area);
    for (row, row_area) in rows.iter().zip(areas.iter()) {
        if row.first() == Some(&Widget::Metrics) {
            if compact {
                metrics_strip(frame, *row_area, state, theme);
            } else {
                draw_tiles(frame, *row_area, state, theme);
            }
            continue;
        }
        let cells = Layout::horizontal(vec![Constraint::Fill(1); row.len()]).split(*row_area);
        for (widget, cell) in row.iter().zip(cells.iter()) {
            if cell.height < 3 {
                continue;
            }
            match widget {
                Widget::Activity => discovered_chart(frame, *cell, state, theme),
                Widget::Interactions => interactions_chart(frame, *cell, state, theme),
                Widget::Distribution => distribution(frame, *cell, state, theme),
                Widget::Recommendations => recommendations(frame, *cell, state, theme),
                Widget::Upcoming => upcoming(frame, *cell, state, theme),
                Widget::News => news(frame, *cell, state, theme),
                Widget::Jobs => jobs(frame, *cell, state, theme),
                Widget::Metrics => {}
            }
        }
    }
}

/// Widgets per row for a terminal `width` columns wide.
const fn per_row(width: u16, chart: bool) -> usize {
    match (chart, width) {
        (_, 110..) => 3,
        (true, 70..) | (false, 60..) => 2,
        _ => 1,
    }
}

/// Groups the configured widgets into rows (see [`render`]).
pub(super) fn rows(widgets: &[Widget], width: u16) -> Vec<Vec<Widget>> {
    let mut rows: Vec<Vec<Widget>> = Vec::new();
    for &w in widgets {
        let joins = rows.last().is_some_and(|row| {
            let first = row[0];
            first != Widget::Metrics
                && w != Widget::Metrics
                && first.is_chart() == w.is_chart()
                && row.len() < per_row(width, w.is_chart())
        });
        match rows.last_mut() {
            Some(row) if joins => row.push(w),
            _ => rows.push(vec![w]),
        }
    }
    rows
}

/// A metric's icon, label, value and color.
fn metric(
    state: &State,
    theme: &Theme,
    name: &str,
) -> Option<(&'static str, &'static str, i64, Color)> {
    let s = &state.overview.stats;
    let i = &state.overview.insights;
    let p = theme.p;
    let count = |view: sentri_core::FeedView| {
        state
            .overview
            .counts
            .iter()
            .find(|(v, _)| *v == view)
            .map_or(0, |(_, n)| i64::try_from(*n).unwrap_or(i64::MAX))
    };
    Some(match name {
        "found" => ("✦", "found", s.total, p.fg),
        "new" => ("●", "new 24h", s.new_since, p.green),
        "news" => ("▤", "news new", i.news_new, p.aqua),
        "events" => ("◷", "events", i.upcoming_events, p.blue),
        "courses" => (
            "✎",
            "courses",
            count(sentri_core::FeedView::Courses),
            p.green,
        ),
        "jobs" => ("⚒", "jobs", count(sentri_core::FeedView::Jobs), p.blue),
        "saved" => ("★", "saved", s.saved, p.yellow),
        "liked" => ("♥", "liked", s.liked, p.red),
        "applied" => ("✓", "applied", i.applied, p.purple),
        "calendar" => ("▦", "calendar", i.calendar_entries, p.orange),
        _ => return None,
    })
}

/// Tiles per row: at least ten columns each.
fn tiles_per_row(width: u16) -> usize {
    usize::from(width / 10).clamp(1, 10)
}

fn metric_rows(prefs: &DashboardPrefs, width: u16) -> u16 {
    let n = prefs.metrics.len().max(1);
    u16::try_from(n.div_ceil(tiles_per_row(width)))
        .unwrap_or(1)
        .min(3)
}

/// Compact metrics: one line, `✦ 120 found · ● 8 new 24h · …`.
fn metrics_strip(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let mut spans = Vec::new();
    for name in &state.dashboard.metrics {
        let Some((icon, label, value, color)) = metric(state, theme, name) else {
            continue;
        };
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", Style::new().fg(theme.p.bg3)));
        }
        spans.push(Span::styled(format!("{icon} "), Style::new().fg(color)));
        spans.push(Span::styled(
            value.to_string(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}"), theme.muted()));
    }
    let width = usize::from(area.width);
    frame.render_widget(
        Paragraph::new(Line::from(card::truncate(spans, width))),
        area,
    );
}

fn draw_tiles(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let tiles: Vec<_> = state
        .dashboard
        .metrics
        .iter()
        .filter_map(|name| metric(state, theme, name))
        .collect();
    let rows: Vec<&[_]> = tiles.chunks(tiles_per_row(area.width)).take(3).collect();
    let row_areas = Layout::vertical(vec![Constraint::Length(4); rows.len()]).split(area);
    for (row, row_area) in rows.iter().zip(row_areas.iter()) {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); row.len()]).split(*row_area);
        for ((icon, label, value, color), cell) in row.iter().zip(cells.iter()) {
            let line = vec![
                Line::from(vec![
                    Span::styled(format!("{icon} "), Style::new().fg(*color)),
                    Span::styled(
                        value.to_string(),
                        Style::new().fg(*color).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(Span::styled(*label, theme.muted())),
            ];
            frame.render_widget(
                Paragraph::new(line)
                    .centered()
                    .block(panel(theme, "", false)),
                *cell,
            );
        }
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

/// Items from one platform shown at most in a row of top picks, so a single
/// busy source (a jobs board, a news site) cannot fill the whole list.
const MAX_PER_SOURCE: usize = 3;

/// The best `n` items in ranking order, at most `per_source` per platform;
/// remaining slots are filled in order when there is not enough variety.
pub(super) fn diversified(items: &[Ranked], n: usize, per_source: usize) -> Vec<&Ranked> {
    let mut picked: Vec<&Ranked> = Vec::with_capacity(n);
    let mut counts: std::collections::HashMap<sentri_core::SourceKind, usize> =
        std::collections::HashMap::new();
    let mut rest = Vec::new();
    for r in items {
        let count = counts.entry(r.item.opportunity.source).or_default();
        if *count < per_source && picked.len() < n {
            *count += 1;
            picked.push(r);
        } else {
            rest.push(r);
        }
    }
    picked.extend(rest.into_iter().take(n.saturating_sub(picked.len())));
    picked
}

fn rows_in(area: Rect) -> usize {
    usize::from(area.height.saturating_sub(2))
}

/// One list row: a fixed lead, then the title cut to the panel.
fn list_line<'a>(mut lead: Vec<Span<'a>>, title: &'a str, width: u16) -> Line<'a> {
    lead.push(Span::raw(title));
    Line::from(card::truncate(lead, usize::from(width.saturating_sub(2))))
}

fn recommendations(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let n = rows_in(area).min(state.dashboard.recommendations);
    let best: Vec<Line> = diversified(&state.items, n, MAX_PER_SOURCE)
        .into_iter()
        .map(|r| {
            list_line(
                vec![
                    score_pill(theme, r.relevance.score),
                    source_badge(theme, r.item.opportunity.source),
                    Span::raw(" "),
                ],
                r.item.opportunity.title.as_str(),
                area.width,
            )
        })
        .collect();
    let empty = if state.loading || state.collecting.is_some() {
        "Collecting… recommendations appear as sources answer."
    } else {
        "Nothing yet. Press R to collect or n to add a link."
    };
    list(frame, area, theme, "Top recommendations", best, empty);
}

fn upcoming(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let now = Utc::now();
    let mut dated: Vec<_> = state
        .items
        .iter()
        .filter_map(|r| Some((r.item.opportunity.next_date(now)?, r)))
        .collect();
    dated.sort_by_key(|(d, _)| *d);
    let lines: Vec<Line> = dated
        .iter()
        .take(rows_in(area))
        .map(|(d, r)| {
            list_line(
                vec![Span::styled(
                    format!("{:<11}", relative(*d, now)),
                    deadline_style(theme, *d, now),
                )],
                r.item.opportunity.title.as_str(),
                area.width,
            )
        })
        .collect();
    list(
        frame,
        area,
        theme,
        "Upcoming dates",
        lines,
        "No upcoming dates.",
    );
}

fn news(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let lines: Vec<Line> = state
        .overview
        .news
        .iter()
        .take(rows_in(area))
        .map(|r| {
            list_line(
                vec![
                    source_badge(theme, r.item.opportunity.source),
                    Span::raw(" "),
                ],
                r.item.opportunity.title.as_str(),
                area.width,
            )
        })
        .collect();
    list(
        frame,
        area,
        theme,
        "Latest tech news",
        lines,
        "No news yet. Press R to collect.",
    );
}

/// Job statistics over the loaded feed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct JobStats {
    pub total: usize,
    pub remote: usize,
    pub with_salary: usize,
    pub strong: usize,
}

pub(super) fn job_stats(items: &[Ranked]) -> JobStats {
    let mut stats = JobStats::default();
    for r in items
        .iter()
        .filter(|r| r.item.opportunity.category.is_job())
    {
        let o = &r.item.opportunity;
        stats.total += 1;
        stats.remote += usize::from(o.modality == sentri_core::Modality::Online);
        stats.with_salary += usize::from(card::salary(o).is_some());
        stats.strong += usize::from(card::is_strong_match(r));
    }
    stats
}

fn jobs(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let summary = job_stats(&state.items);
    let mut lines = Vec::new();
    if summary.total > 0 {
        let sep = || Span::styled(" · ", Style::new().fg(theme.p.bg3));
        lines.push(Line::from(card::truncate(
            vec![
                Span::styled(
                    format!("{} open", summary.total),
                    Style::new().fg(theme.p.blue),
                ),
                sep(),
                Span::styled(format!("◎ {} remote", summary.remote), theme.dim()),
                sep(),
                Span::styled(format!("{} with salary", summary.with_salary), theme.good()),
                sep(),
                Span::styled(
                    format!("{} strong match", summary.strong),
                    Style::new().fg(theme.p.green),
                ),
            ],
            usize::from(area.width.saturating_sub(2)),
        )));
        let room = rows_in(area).saturating_sub(1);
        for r in state
            .items
            .iter()
            .filter(|r| r.item.opportunity.category.is_job())
            .take(room)
        {
            let o = &r.item.opportunity;
            let mut lead = vec![score_pill(theme, r.relevance.score)];
            if let Some(pay) = card::salary(o) {
                lead.push(Span::styled(format!("{pay} · "), theme.good()));
            }
            lines.push(list_line(lead, o.title.as_str(), area.width));
        }
    }
    list(
        frame,
        area,
        theme,
        "Jobs",
        lines,
        "No jobs in the feed yet.",
    );
}

fn list(frame: &mut Frame, area: Rect, theme: &Theme, title: &str, lines: Vec<Line>, empty: &str) {
    let lines = if lines.is_empty() {
        vec![Line::from(Span::styled(empty.to_owned(), theme.muted()))]
    } else {
        lines
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel(theme, title, false)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use Widget::*;

    #[test]
    fn widgets_flow_into_rows_in_the_configured_order() {
        let all = [
            Metrics,
            Activity,
            Interactions,
            Distribution,
            Recommendations,
            Upcoming,
            News,
            Jobs,
        ];
        assert_eq!(
            rows(&all, 170),
            vec![
                vec![Metrics],
                vec![Activity, Interactions, Distribution],
                vec![Recommendations, Upcoming, News],
                vec![Jobs],
            ]
        );
        assert_eq!(
            rows(&all, 80),
            vec![
                vec![Metrics],
                vec![Activity, Interactions],
                vec![Distribution],
                vec![Recommendations, Upcoming],
                vec![News, Jobs],
            ]
        );
        assert_eq!(
            rows(&[News, Activity, Metrics, Jobs], 60),
            vec![vec![News], vec![Activity], vec![Metrics], vec![Jobs]]
        );
        assert_eq!(rows(&[], 100), Vec::<Vec<Widget>>::new());
    }
}
