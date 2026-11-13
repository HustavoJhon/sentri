//! Detail panel: every known field, clearly separated dates, and the
//! explanation of the relevance score.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sentri_app::Ranked;
use sentri_core::{Eligibility, Modality};

use super::widgets::{badge, flag_glyphs, panel, relative, source_badge};
use crate::state::{Focus, State};
use crate::theme::Theme;

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

/// Builds the lines for one opportunity.
pub fn lines<'a>(theme: &Theme, r: &'a Ranked, now: DateTime<Utc>) -> Vec<Line<'a>> {
    let mut out = header(theme, r);
    dates(&mut out, theme, r, now);
    details(&mut out, theme, r);
    score(&mut out, theme, r);
    out
}

fn header<'a>(theme: &Theme, r: &'a Ranked) -> Vec<Line<'a>> {
    let o = &r.item.opportunity;
    let mut out = vec![
        Line::from(Span::styled(
            o.title.as_str(),
            Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];

    let mut chips = vec![
        source_badge(theme, o.source),
        Span::raw(" "),
        badge(theme, o.category.label(), theme.category_color(o.category)),
    ];
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
    out.push(Line::from(chips));
    let glyphs = flag_glyphs(&r.item.state);
    if !glyphs.is_empty() {
        out.push(Line::from(Span::styled(glyphs, theme.accent())));
    }

    if let Some(summary) = &o.summary {
        out.push(Line::default());
        out.push(Line::from(Span::styled(summary.as_str(), theme.dim())));
    }
    out
}

fn dates<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, r: &'a Ranked, now: DateTime<Utc>) {
    let o = &r.item.opportunity;
    out.push(Line::default());
    out.push(section(theme, "Dates"));
    let dates = [
        ("Deadline", o.dates.deadline),
        ("Starts", o.dates.starts_at),
        ("Ends", o.dates.ends_at),
        ("Published", o.dates.published_at),
        ("Discovered", Some(o.discovered_at)),
    ];
    for (name, date) in dates {
        if let Some(d) = date {
            out.push(field(theme, name, date_value(d, now)));
        }
    }
}

fn details<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, r: &'a Ranked) {
    let o = &r.item.opportunity;
    out.push(Line::default());
    out.push(section(theme, "Details"));
    out.push(field(
        theme,
        "Source",
        format!("{} · {}", o.source.label(), o.source_name),
    ));
    if let Some(location) = &o.location {
        out.push(field(theme, "Location", location.as_str()));
    }
    if let Some(country) = &o.country {
        out.push(field(theme, "Country", country.as_str()));
    }
    out.push(field(theme, "Cost", o.cost.label()));
    let eligibility = match o.eligibility {
        Eligibility::Open => Span::styled("open to your country", theme.good()),
        Eligibility::NotEligible => Span::styled("not open to your country", theme.bad()),
        Eligibility::Unknown => Span::styled("not stated", theme.muted()),
    };
    out.push(field(theme, "Eligibility", eligibility));
    out.push(field(theme, "Certificate", yes_no(o.perks.certificate)));
    out.push(field(theme, "Travel", yes_no(o.perks.travel_covered)));
    out.push(field(
        theme,
        "Lodging",
        yes_no(o.perks.accommodation_covered),
    ));
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

fn score<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, r: &'a Ranked) {
    out.push(Line::default());
    out.push(section(theme, "Why this score"));
    out.push(Line::from(vec![
        Span::styled("Relevance  ", theme.muted()),
        Span::styled(
            r.relevance.score.to_string(),
            Style::new()
                .fg(theme.score_color(r.relevance.score))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    if r.relevance.reasons.is_empty() {
        out.push(Line::from(Span::styled(
            "No rule matched; it stays in the feed for discovery.",
            theme.muted(),
        )));
    }
    for reason in &r.relevance.reasons {
        let color = if reason.points >= 0 {
            theme.p.green
        } else {
            theme.p.red
        };
        out.push(Line::from(vec![
            Span::styled(format!("{:>+4}  ", reason.points), Style::new().fg(color)),
            Span::raw(reason.text.as_str()),
        ]));
    }
    out.push(Line::default());
    out.push(Line::from(Span::styled(
        "l like · s save · d dismiss · p applied · e enrolled · c completed · o open",
        theme.muted(),
    )));
}

pub fn render(frame: &mut Frame, area: Rect, state: &State, theme: &Theme) {
    let block = panel(theme, "Details", state.focus == Focus::Detail);
    let Some(r) = state.selected_item() else {
        frame.render_widget(
            Paragraph::new("Select an opportunity to see its details.")
                .style(theme.muted())
                .block(block),
            area,
        );
        return;
    };
    let paragraph = Paragraph::new(lines(theme, r, Utc::now()))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((state.detail_scroll, 0));
    frame.render_widget(paragraph, area);
}
