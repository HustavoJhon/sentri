//! Details: header with publisher, image, every known field with dates kept
//! apart, personal notes, SENTRI's recommendation and the change history.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sentri_app::Ranked;
use sentri_core::{ContentKind, Eligibility, Modality, Stance};

use super::charts::bar;
use super::feed::stance_color;
use super::widgets::{badge, flag_glyphs, panel, relative, source_badge};
use crate::graphics::Fit;
use crate::state::{DetailData, Focus, ImageSlot, State};
use crate::theme::Theme;
use sentri_app::ImageMode;

fn field<'a>(theme: &Theme, name: &'a str, value: impl Into<Span<'a>>) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{name:<12}"), theme.muted()),
        value.into(),
    ])
}

fn section<'a>(theme: &Theme, name: &'a str) -> Line<'a> {
    Line::from(Span::styled(
        format!("── {name} "),
        Style::new().fg(theme.p.aqua).add_modifier(Modifier::BOLD),
    ))
}

fn date_value(d: DateTime<Utc>, now: DateTime<Utc>) -> String {
    format!("{}  ({})", d.format("%Y-%m-%d %H:%M UTC"), relative(d, now))
}

fn yes_no(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "not stated",
    }
}

fn chips<'a>(theme: &Theme, r: &'a Ranked) -> Line<'a> {
    let o = &r.item.opportunity;
    let mut chips = vec![
        source_badge(theme, o.source),
        Span::styled(format!(" {} ", o.source.label()), theme.dim()),
    ];
    if o.content == ContentKind::News {
        let stance = o.stance.unwrap_or(Stance::Report);
        chips.push(badge(theme, stance.label(), stance_color(theme, stance)));
        if let Some(topic) = o.news_topic() {
            chips.extend([Span::raw(" "), badge(theme, topic, theme.p.aqua)]);
        }
    } else {
        chips.push(badge(
            theme,
            o.category.label(),
            theme.category_color(o.category),
        ));
    }
    if o.modality != Modality::Unknown {
        chips.extend([
            Span::raw(" "),
            badge(theme, o.modality.label(), theme.p.bg3),
        ]);
    }
    if o.cost.is_free() {
        chips.extend([Span::raw(" "), badge(theme, "Free", theme.p.green)]);
    }
    if o.perks.certificate == Some(true) {
        chips.extend([Span::raw(" "), badge(theme, "Certificate", theme.p.blue)]);
    }
    if o.perks.offers_travel() {
        chips.extend([Span::raw(" "), badge(theme, "✈ Travel", theme.p.purple)]);
    }
    Line::from(chips)
}

/// Title, chips and publisher.
fn header<'a>(theme: &Theme, r: &'a Ranked) -> Vec<Line<'a>> {
    let o = &r.item.opportunity;
    let mut out = vec![
        Line::from(Span::styled(
            o.title.as_str(),
            Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD),
        )),
        chips(theme, r),
    ];
    let author = &o.author;
    if !author.is_empty() {
        let mut spans = vec![Span::styled("by ", theme.muted())];
        if let Some(name) = &author.name {
            spans.push(Span::styled(
                name.as_str(),
                Style::new().add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
        }
        if let Some(handle) = &author.handle {
            spans.push(Span::styled(handle.as_str(), Style::new().fg(theme.p.aqua)));
            spans.push(Span::raw(" "));
        }
        if let Some(url) = &author.profile_url {
            spans.push(Span::styled(url.as_str(), theme.muted()));
        }
        out.push(Line::from(spans));
    }
    let glyphs = flag_glyphs(&r.item.state);
    if !glyphs.is_empty() {
        out.push(Line::from(Span::styled(glyphs, theme.accent())));
    }
    if o.stance == Some(Stance::Speculation) {
        out.push(Line::from(Span::styled(
            "⚠ Unconfirmed: the wording signals rumors or anonymous sources.",
            Style::new().fg(theme.p.orange),
        )));
    }
    out
}

fn dates<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, r: &'a Ranked, now: DateTime<Utc>) {
    let o = &r.item.opportunity;
    out.push(section(theme, "Dates"));
    for (name, date) in [
        ("Event/start", o.dates.starts_at),
        ("Ends", o.dates.ends_at),
        ("Deadline", o.dates.deadline),
        ("Published", o.dates.published_at),
        ("Discovered", Some(o.discovered_at)),
    ] {
        if let Some(d) = date {
            out.push(field(theme, name, date_value(d, now)));
        }
    }
}

fn facts<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, r: &'a Ranked) {
    let o = &r.item.opportunity;
    out.push(section(theme, "Details"));
    out.push(field(theme, "Source", o.source_name.as_str()));
    for (name, value) in [("Location", &o.location), ("Country", &o.country)] {
        if let Some(v) = value {
            out.push(field(theme, name, v.as_str()));
        }
    }
    out.push(field(theme, "Cost", o.cost.label()));
    out.push(field(
        theme,
        "Eligibility",
        match o.eligibility {
            Eligibility::Open => Span::styled("open to your country", theme.good()),
            Eligibility::NotEligible => Span::styled("not open to your country", theme.bad()),
            Eligibility::Unknown => Span::styled("not stated", theme.muted()),
        },
    ));
    if o.content == ContentKind::Opportunity {
        out.push(field(theme, "Certificate", yes_no(o.perks.certificate)));
        out.push(field(theme, "Travel", yes_no(o.perks.travel_covered)));
        out.push(field(
            theme,
            "Lodging",
            yes_no(o.perks.accommodation_covered),
        ));
    }
    for (name, value) in [
        ("Prizes", &o.perks.prizes),
        ("Benefits", &o.perks.benefits),
        ("Requires", &o.requirements),
    ] {
        if let Some(v) = value {
            out.push(field(theme, name, v.as_str()));
        }
    }
    if !o.tags.is_empty() {
        out.push(field(theme, "Tags", o.tags.join(", ")));
    }
    out.push(field(
        theme,
        "Link",
        Span::styled(
            o.original_url.as_str(),
            Style::new()
                .fg(theme.p.blue)
                .add_modifier(Modifier::UNDERLINED),
        ),
    ));
}

fn notes<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, detail: Option<&'a DetailData>) {
    let Some(d) = detail else {
        return;
    };
    let a = &d.edit.annotations;
    if a.notes.is_none() && a.priority.is_none() && a.custom.is_empty() {
        return;
    }
    out.push(Line::default());
    out.push(section(theme, "My notes"));
    if let Some(p) = a.priority {
        out.push(field(theme, "Priority", p.as_str()));
    }
    if let Some(notes) = &a.notes {
        out.push(Line::from(notes.as_str()));
    }
    for (key, value) in &a.custom {
        out.push(Line::from(vec![
            Span::styled(format!("{key:<12}"), theme.muted()),
            Span::raw(value.as_str()),
        ]));
    }
}

/// Every field, notes and custom fields.
pub fn body<'a>(
    theme: &Theme,
    r: &'a Ranked,
    detail: Option<&'a DetailData>,
    now: DateTime<Utc>,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    if let Some(summary) = &r.item.opportunity.summary {
        out.push(Line::from(Span::styled(summary.as_str(), theme.dim())));
        out.push(Line::default());
    }
    dates(&mut out, theme, r, now);
    out.push(Line::default());
    facts(&mut out, theme, r);
    notes(&mut out, theme, detail);
    out
}

fn recommendation<'a>(theme: &Theme, r: &'a Ranked) -> Vec<Line<'a>> {
    let mut out = vec![
        section(theme, "SENTRI recommendation"),
        Line::from(vec![
            Span::styled("Relevance ", theme.muted()),
            Span::styled(
                r.relevance.score.to_string(),
                Style::new()
                    .fg(theme.score_color(r.relevance.score))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    let max = r
        .relevance
        .reasons
        .iter()
        .map(|x| i64::from(x.points.abs()))
        .max()
        .unwrap_or(1);
    for reason in &r.relevance.reasons {
        let color = if reason.points >= 0 {
            theme.p.green
        } else {
            theme.p.red
        };
        out.push(Line::from(vec![
            Span::styled(format!("{:>+4} ", reason.points), Style::new().fg(color)),
            Span::styled(
                format!("{:<8}", bar(i64::from(reason.points.abs()), max, 8)),
                Style::new().fg(color),
            ),
            Span::raw(" "),
            Span::raw(reason.text.as_str()),
        ]));
    }
    if r.relevance.reasons.is_empty() {
        out.push(Line::from(Span::styled(
            "No rule matched; kept for discovery.",
            theme.muted(),
        )));
    }
    out
}

/// Score explanation, manual changes and history.
pub fn side<'a>(theme: &Theme, r: &'a Ranked, detail: Option<&'a DetailData>) -> Vec<Line<'a>> {
    let mut out = recommendation(theme, r);
    if let Some(d) = detail {
        if !d.sightings.is_empty() {
            out.push(Line::default());
            out.push(section(theme, "Sources"));
            for sighting in &d.sightings {
                out.push(Line::from(vec![
                    Span::styled(format!("• {:<16}", sighting.source_name), theme.accent()),
                    Span::styled(sighting.original_url.as_str(), theme.muted()),
                ]));
            }
        }
        if !d.edit.overrides.is_empty() {
            out.push(Line::default());
            out.push(section(theme, "Edited by you"));
            for o in &d.edit.overrides {
                let detected = o.field.get(&d.edit.detected);
                out.push(Line::from(vec![
                    Span::styled(format!("✎ {:<12}", o.field.label()), theme.accent()),
                    Span::raw(o.value.as_str()),
                ]));
                out.push(Line::from(Span::styled(
                    format!(
                        "  detected: {}",
                        if detected.is_empty() {
                            "—"
                        } else {
                            &detected
                        }
                    ),
                    theme.muted(),
                )));
            }
        }
        if !d.history.is_empty() {
            out.push(Line::default());
            out.push(section(theme, "History"));
            for entry in d.history.iter().take(8) {
                out.push(Line::from(vec![
                    Span::styled(entry.at.format("%m-%d %H:%M  ").to_string(), theme.muted()),
                    Span::raw(entry.action.as_db()),
                ]));
            }
        }
    }
    out.push(Line::default());
    out.push(Line::from(Span::styled(
        "E edit · l like · s save · d dismiss · p applied · a calendar · o open",
        theme.muted(),
    )));
    out
}

fn draw_image(frame: &mut Frame, state: &State, url: Option<&str>, host: Rect, rows: u16) -> u16 {
    let (Some(url), Some(thumb)) = (url, state.image(url)) else {
        return 0;
    };
    let target = state.graphics.draw(
        frame.buffer_mut(),
        Rect {
            height: rows.min(host.height),
            ..host
        },
        url,
        thumb,
        Fit::Contain,
        state.ui.images,
        state.ui.indexed_colors,
    );
    if target.height == 0 {
        0
    } else {
        target.height + 1
    }
}

fn below(area: Rect, used: u16) -> Rect {
    Rect {
        y: area.y + used.min(area.height),
        height: area.height.saturating_sub(used),
        ..area
    }
}

fn draw_header(frame: &mut Frame, area: Rect, state: &State, theme: &Theme, r: &Ranked) -> u16 {
    let lines = header(theme, r);
    let text_width = usize::from(area.width.saturating_sub(8).max(10));
    let wrapped: usize = lines
        .iter()
        .map(|l| l.width().div_ceil(text_width).max(1))
        .sum();
    let height = (u16::try_from(wrapped).unwrap_or(4) + 1).min(area.height);
    let head = Rect { height, ..area };
    let avatar_url = r.item.opportunity.author.avatar_url.as_deref();
    let text_area = match avatar_url.zip(state.image(avatar_url)) {
        Some((url, avatar)) if head.width > 20 && state.ui.images != ImageMode::Off => {
            state.graphics.draw(
                frame.buffer_mut(),
                Rect::new(head.x, head.y, 6, head.height.min(3)),
                url,
                avatar,
                Fit::Cover,
                state.ui.images,
                state.ui.indexed_colors,
            );
            Rect {
                x: head.x + 8,
                width: head.width - 8,
                ..head
            }
        }
        _ => head,
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), text_area);
    height
}

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let block = panel(theme, "Details", state.focus == Focus::Detail);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(r) = state.selected_item() else {
        frame.render_widget(
            Paragraph::new("Select an item to see its details.").style(theme.muted()),
            inner,
        );
        return;
    };
    let o = &r.item.opportunity;
    let detail = state.detail_for_selected();
    let rest = below(inner, draw_header(frame, inner, state, theme, r));

    let mut body_lines = body(theme, r, detail, Utc::now());
    if matches!(
        o.image_url.as_ref().and_then(|u| state.images.get(u)),
        Some(ImageSlot::Loading)
    ) {
        body_lines.insert(
            0,
            Line::from(Span::styled("◌ loading image…", theme.muted())),
        );
    }
    let image_rows = (rest.height / 3).clamp(4, 14);
    if rest.width >= 80 {
        let [main, side_area] =
            Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .areas(rest);
        let side_area = Rect {
            x: side_area.x + 1,
            width: side_area.width.saturating_sub(1),
            ..side_area
        };
        let used = draw_image(frame, state, o.image_url.as_deref(), side_area, image_rows);
        frame.render_widget(
            Paragraph::new(body_lines)
                .wrap(Wrap { trim: false })
                .scroll((state.detail_scroll, 0)),
            main,
        );
        frame.render_widget(
            Paragraph::new(side(theme, r, detail)).wrap(Wrap { trim: false }),
            below(side_area, used),
        );
    } else {
        let used = draw_image(frame, state, o.image_url.as_deref(), rest, image_rows);
        body_lines.push(Line::default());
        body_lines.extend(side(theme, r, detail));
        frame.render_widget(
            Paragraph::new(body_lines)
                .wrap(Wrap { trim: false })
                .scroll((state.detail_scroll, 0)),
            below(rest, used),
        );
    }
}
