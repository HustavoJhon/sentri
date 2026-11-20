//! Details: an information panel rather than a long text dump.
//!
//! ```text
//! HACKATHON   DP Devpost                         NEW ★
//! AI Hackathon Cusco                                      ← title
//! by GDG Cusco                                            ← organizer
//! ┌ image ┐  (right column when wide, on top when narrow)
//! ── Key facts ────────  only what is known: dates, place, cost, perks
//! ── Overview ─────────  summary, paragraphs and bullets kept
//! ── Requirements ─────  requirements and eligibility
//! ── Tags ─────────────
//! ── SENTRI recommendation · sources · edits · history · notes
//! ```
//!
//! Unknown values are left out instead of printing "not stated" rows.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sentri_app::{ImageMode, Ranked};
use sentri_core::{ContentKind, Eligibility, Modality, Opportunity, Stance};
use unicode_width::UnicodeWidthStr;

use super::card::{self, stance_color};
use super::charts::bar;
use super::widgets::{badge, distance, panel, source_badge};
use crate::graphics::Fit;
use crate::state::{DetailData, Focus, ImageSlot, State};
use crate::theme::Theme;

/// Label column width in fact grids.
const LABEL: usize = 11;

/// `── Name ────…` filling the width.
fn heading(theme: &Theme, name: &str, width: usize) -> Line<'static> {
    let used = name.width() + 4;
    Line::from(vec![
        Span::styled("── ", Style::new().fg(theme.p.bg3)),
        Span::styled(
            name.to_owned(),
            Style::new().fg(theme.p.aqua).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", "─".repeat(width.saturating_sub(used))),
            Style::new().fg(theme.p.bg2),
        ),
    ])
}

fn field<'a>(theme: &Theme, name: &str, value: impl Into<Span<'a>>) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{name:<LABEL$}"), theme.muted()),
        value.into(),
    ])
}

/// A labelled value that may span several lines (benefits, prizes): the
/// value wraps in its own column, aligned under the first line.
fn long_field(
    out: &mut Vec<Line<'_>>,
    theme: &Theme,
    name: &str,
    text: &str,
    style: Style,
    width: usize,
) {
    let lines = paragraphs(text, width.saturating_sub(LABEL).max(12));
    for (i, line) in lines.into_iter().enumerate() {
        let label = if i == 0 { name } else { "" };
        out.push(field(theme, label, Span::styled(line, style)));
    }
}

fn date_value(d: DateTime<Utc>, now: DateTime<Utc>) -> String {
    format!("{}  ({})", card::short_date(d, now), distance(d, now))
}

/// Text with Markdown emphasis markers removed (`**bold**`, `__x__`).
fn plain(text: &str) -> String {
    text.replace("**", "").replace("__", "")
}

/// Summary split into paragraphs and list items, each wrapped to `width`
/// with a hanging indent for items. Blank lines in the source separate
/// paragraphs; single line breaks are kept; bullets (`•`, `-`, `*`) and
/// numbered items (`1.`, `2)`) keep their marker.
pub fn paragraphs(text: &str, width: usize) -> Vec<String> {
    let text = plain(text);
    // (gap before, marker, body)
    let mut blocks: Vec<(bool, Option<String>, String)> = Vec::new();
    let mut gap = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            gap = !blocks.is_empty();
            continue;
        }
        // Inline "a • b • c" lists, and "intro: - a - b" lists flattened by
        // the source, become separate bullets.
        let separator = if line.contains(" • ") {
            " • "
        } else if line.contains(": - ") && line.matches(" - ").count() >= 2 {
            " - "
        } else {
            " • "
        };
        let mut parts = line.split(separator);
        if let Some(first) = parts.next() {
            let (marker, body) = marker(first);
            blocks.push((gap, marker, body.to_owned()));
        }
        blocks.extend(parts.map(|p| (false, Some("•".to_owned()), p.trim().to_owned())));
        gap = false;
    }
    let mut out = Vec::new();
    for (i, (gap, marker, body)) in blocks.iter().enumerate() {
        if i > 0 && (*gap || marker.is_some() != blocks[i - 1].1.is_some()) {
            out.push(String::new());
        }
        if let Some(marker) = marker {
            let indent = marker.width() + 1;
            for (j, line) in card::wrap(body, width.saturating_sub(indent), usize::MAX)
                .into_iter()
                .enumerate()
            {
                if j == 0 {
                    out.push(format!("{marker} {line}"));
                } else {
                    out.push(format!("{}{line}", " ".repeat(indent)));
                }
            }
        } else {
            out.extend(card::wrap(body, width, usize::MAX));
        }
    }
    out
}

/// The list marker a line starts with, normalized (`-`/`*`/`·` → `•`), and
/// the rest of the line.
fn marker(line: &str) -> (Option<String>, &str) {
    for bullet in ["• ", "- ", "* ", "· ", "▪ ", "◦ "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return (Some("•".to_owned()), rest.trim());
        }
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let rest = &line[digits..];
        for close in [". ", ") "] {
            if let Some(body) = rest.strip_prefix(close) {
                return (Some(format!("{}.", &line[..digits])), body.trim());
            }
        }
    }
    (None, line)
}

/// Kind and source on the left, your marks on the right.
fn kicker<'a>(theme: &Theme, r: &'a Ranked, width: usize) -> Line<'a> {
    let o = &r.item.opportunity;
    let mut left = vec![source_badge(theme, o.source), Span::raw(" ")];
    if o.content == ContentKind::News {
        let stance = o.stance.unwrap_or(Stance::Report);
        left.push(badge(theme, stance.label(), stance_color(theme, stance)));
        if let Some(topic) = o.news_topic() {
            left.push(Span::styled(
                format!(" {topic}"),
                Style::new().fg(theme.p.aqua),
            ));
        }
    } else {
        left.push(badge(
            theme,
            o.category.label(),
            theme.category_color(o.category),
        ));
    }
    left.push(Span::styled(
        format!("  {}", card::publisher(o)),
        theme.dim(),
    ));
    let mut right = Vec::new();
    if card::is_new(r, Utc::now()) {
        right.push(Span::styled(" NEW ", theme.badge(theme.p.green)));
        right.push(Span::raw(" "));
    }
    right.extend(card::marks(theme, &r.item.state));
    if r.item.state.dismissed {
        right.push(Span::styled(" ✗ dismissed", theme.muted()));
    }
    let left_width: usize = left.iter().map(Span::width).sum();
    let right_width: usize = right.iter().map(Span::width).sum();
    if left_width + right_width < width {
        left.push(Span::raw(" ".repeat(width - left_width - right_width)));
        left.extend(right);
    }
    Line::from(left)
}

/// Kicker, title, organizer and warnings.
fn header<'a>(theme: &Theme, r: &'a Ranked, width: usize) -> Vec<Line<'a>> {
    let o = &r.item.opportunity;
    let mut out = vec![
        kicker(theme, r, width),
        Line::from(Span::styled(
            o.title.as_str(),
            Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD),
        )),
    ];
    let author = &o.author;
    if !author.is_empty() {
        let mut spans = vec![Span::styled("by ", theme.muted())];
        if let Some(name) = &author.name {
            spans.push(Span::styled(
                name.as_str(),
                Style::new().fg(theme.p.aqua).add_modifier(Modifier::BOLD),
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
    if card::is_expired(o, Utc::now()) {
        out.push(Line::from(Span::styled(
            "Closed: the deadline or the event date has passed.",
            theme.muted(),
        )));
    }
    if o.stance == Some(Stance::Speculation) {
        out.push(Line::from(Span::styled(
            "⚠ Unconfirmed: the wording signals rumors or anonymous sources.",
            Style::new().fg(theme.p.orange),
        )));
    }
    out
}

fn dates(out: &mut Vec<Line<'_>>, theme: &Theme, o: &Opportunity, now: DateTime<Utc>) {
    let names: [(&str, Option<DateTime<Utc>>); 5] = if o.content == ContentKind::News {
        [
            ("Published", o.dates.published_at),
            ("Discovered", Some(o.discovered_at)),
            ("", None),
            ("", None),
            ("", None),
        ]
    } else {
        [
            ("Starts", o.dates.starts_at),
            ("Ends", o.dates.ends_at),
            ("Deadline", o.dates.deadline),
            ("Published", o.dates.published_at),
            ("Discovered", Some(o.discovered_at)),
        ]
    };
    for (name, date) in names {
        let Some(d) = date else { continue };
        let style = if name == "Deadline" && d >= now {
            Style::new().fg(theme.p.orange).add_modifier(Modifier::BOLD)
        } else if name == "Starts" {
            Style::new().fg(theme.p.blue)
        } else {
            theme.dim()
        };
        out.push(field(theme, name, Span::styled(date_value(d, now), style)));
    }
}

fn place<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, o: &'a Opportunity) {
    let mut where_ = Vec::new();
    if let Some(location) = &o.location {
        where_.push(location.as_str());
    }
    if let Some(country) = o
        .country
        .as_deref()
        .filter(|c| !o.location.as_deref().is_some_and(|l| l.contains(c)))
    {
        where_.push(country);
    }
    if !where_.is_empty() {
        out.push(field(theme, "Where", where_.join(", ")));
    }
    if o.modality != Modality::Unknown {
        let text = if o.category.is_job() {
            card::modality(o.modality)
        } else {
            match o.modality {
                Modality::Online => "◎ online",
                Modality::InPerson => "⌂ in person",
                _ => "◐ hybrid",
            }
        };
        out.push(field(
            theme,
            "Modality",
            Span::styled(text, Style::new().fg(theme.p.blue)),
        ));
    }
}

fn money<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, o: &'a Opportunity, width: usize) {
    if let Some(pay) = card::salary(o) {
        out.push(field(
            theme,
            "Salary",
            Span::styled(pay, theme.good().add_modifier(Modifier::BOLD)),
        ));
    } else if let Some(benefits) = &o.perks.benefits {
        long_field(
            out,
            theme,
            "Benefits",
            benefits,
            Style::new().fg(theme.p.fg),
            width,
        );
    }
    if o.cost != sentri_core::Cost::Unknown {
        let style = if o.cost.is_free() {
            theme.good()
        } else {
            theme.dim()
        };
        out.push(field(theme, "Cost", Span::styled(o.cost.label(), style)));
    }
    if let Some(prizes) = &o.perks.prizes {
        long_field(
            out,
            theme,
            "Prizes",
            prizes,
            Style::new().fg(theme.p.yellow),
            width,
        );
    }
    let perks: Vec<&str> = [
        (o.perks.certificate, "certificate"),
        (o.perks.travel_covered, "✈ travel covered"),
        (o.perks.accommodation_covered, "lodging covered"),
    ]
    .into_iter()
    .filter(|(v, _)| *v == Some(true))
    .map(|(_, name)| name)
    .collect();
    if !perks.is_empty() {
        out.push(field(
            theme,
            "Includes",
            Span::styled(perks.join(" · "), Style::new().fg(theme.p.purple)),
        ));
    }
}

fn requirements<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, o: &'a Opportunity, width: usize) {
    let eligibility = match o.eligibility {
        Eligibility::Open => Some(Span::styled("open to your country", theme.good())),
        Eligibility::NotEligible => Some(Span::styled("not open to your country", theme.bad())),
        Eligibility::Unknown => None,
    };
    if o.requirements.is_none() && eligibility.is_none() {
        return;
    }
    out.push(Line::default());
    out.push(heading(theme, "Requirements", width));
    if let Some(e) = eligibility {
        out.push(field(theme, "Eligibility", e));
    }
    if let Some(r) = &o.requirements {
        out.extend(paragraphs(r, width).into_iter().map(Line::from));
    }
}

fn tags<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, o: &'a Opportunity, width: usize) {
    if o.tags.is_empty() {
        return;
    }
    out.push(Line::default());
    out.push(heading(theme, "Tags", width));
    let mut spans = Vec::new();
    for tag in &o.tags {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!("#{tag}"),
            Style::new().fg(theme.p.aqua),
        ));
    }
    out.push(Line::from(spans));
}

fn notes<'a>(out: &mut Vec<Line<'a>>, theme: &Theme, detail: Option<&'a DetailData>, width: usize) {
    let Some(d) = detail else {
        return;
    };
    let a = &d.edit.annotations;
    if a.notes.is_none() && a.priority.is_none() && a.custom.is_empty() {
        return;
    }
    out.push(Line::default());
    out.push(heading(theme, "My notes", width));
    if let Some(p) = a.priority {
        out.push(field(theme, "Priority", p.as_str()));
    }
    if let Some(notes) = &a.notes {
        out.push(Line::from(notes.as_str()));
    }
    for (key, value) in &a.custom {
        out.push(field(theme, key, value.as_str()));
    }
}

/// Key facts, overview, requirements, tags and notes.
pub fn body<'a>(
    theme: &Theme,
    r: &'a Ranked,
    detail: Option<&'a DetailData>,
    now: DateTime<Utc>,
    width: usize,
) -> Vec<Line<'a>> {
    let o = &r.item.opportunity;
    // Facts first: a long description must not hide dates and place.
    let mut out = vec![heading(theme, "Key facts", width)];
    dates(&mut out, theme, o, now);
    place(&mut out, theme, o);
    money(&mut out, theme, o, width);
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
    if let Some(summary) = &o.summary {
        out.push(Line::default());
        out.push(heading(theme, "Overview", width));
        out.extend(
            paragraphs(summary, width)
                .into_iter()
                .map(|l| Line::from(Span::styled(l, Style::new().fg(theme.p.fg)))),
        );
    }
    requirements(&mut out, theme, o, width);
    tags(&mut out, theme, o, width);
    notes(&mut out, theme, detail, width);
    out
}

fn recommendation<'a>(theme: &Theme, r: &'a Ranked, width: usize) -> Vec<Line<'a>> {
    let mut score = vec![Span::styled("Match ", theme.muted())];
    score.extend(card::meter(theme, r.relevance.score));
    score.push(Span::styled(
        format!("  {}", r.relevance.score),
        Style::new()
            .fg(theme.score_color(r.relevance.score))
            .add_modifier(Modifier::BOLD),
    ));
    let mut out = vec![
        heading(theme, "SENTRI recommendation", width),
        Line::from(score),
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

/// Score explanation, sources, manual changes and history.
pub fn side<'a>(
    theme: &Theme,
    r: &'a Ranked,
    detail: Option<&'a DetailData>,
    width: usize,
) -> Vec<Line<'a>> {
    let mut out = recommendation(theme, r, width);
    if let Some(d) = detail {
        if !d.sightings.is_empty() {
            out.push(Line::default());
            out.push(heading(theme, "Sources", width));
            for sighting in &d.sightings {
                out.push(Line::from(vec![
                    Span::styled(format!("• {:<16}", sighting.source_name), theme.accent()),
                    Span::styled(sighting.original_url.as_str(), theme.muted()),
                ]));
            }
        }
        if !d.edit.overrides.is_empty() {
            out.push(Line::default());
            out.push(heading(theme, "Edited by you", width));
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
            out.push(heading(theme, "History", width));
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

/// Draws the item's image (or a quiet loading line) at the top of `host`;
/// returns the rows used, including one row of spacing.
fn draw_image(
    frame: &mut Frame,
    state: &State,
    theme: &Theme,
    url: Option<&str>,
    host: Rect,
    rows: u16,
) -> u16 {
    let Some(url) = url.filter(|_| state.ui.images != ImageMode::Off) else {
        return 0;
    };
    let area = Rect {
        height: rows.min(host.height),
        ..host
    };
    match state.images.get(url) {
        Some(ImageSlot::Ready(thumb)) => {
            let used = state.graphics.draw(
                frame.buffer_mut(),
                area,
                url,
                thumb,
                Fit::Contain,
                state.ui.images,
                state.ui.indexed_colors,
            );
            if used.height == 0 { 0 } else { used.height + 1 }
        }
        Some(ImageSlot::Loading) if area.height > 0 => {
            frame.render_widget(
                Paragraph::new(Span::styled("◌ loading image…", theme.muted())),
                area,
            );
            2
        }
        // Missing or broken images take no space.
        _ => 0,
    }
}

fn below(area: Rect, used: u16) -> Rect {
    Rect {
        y: area.y + used.min(area.height),
        height: area.height.saturating_sub(used),
        ..area
    }
}

/// Header block with the author's avatar when there is one; returns rows.
fn draw_header(frame: &mut Frame, area: Rect, state: &State, theme: &Theme, r: &Ranked) -> u16 {
    let avatar_url = r.item.opportunity.author.avatar_url.as_deref();
    let avatar = avatar_url.zip(state.image(avatar_url));
    let show_avatar = avatar.is_some() && area.width > 30 && state.ui.images != ImageMode::Off;
    let text_area = if show_avatar {
        Rect {
            x: area.x + 8,
            width: area.width - 8,
            ..area
        }
    } else {
        area
    };
    let width = usize::from(text_area.width);
    let lines = header(theme, r, width);
    let wrapped: usize = lines
        .iter()
        .map(|l| l.width().div_ceil(width.max(1)).max(1))
        .sum();
    let height = (u16::try_from(wrapped).unwrap_or(4) + 1).min(area.height);
    if let (true, Some((url, image))) = (show_avatar, avatar) {
        state.graphics.draw(
            frame.buffer_mut(),
            Rect::new(area.x, area.y, 6, height.saturating_sub(1).clamp(1, 3)),
            url,
            image,
            Fit::Cover,
            state.ui.images,
            state.ui.indexed_colors,
        );
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        Rect {
            height,
            ..text_area
        },
    );
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
    let now = Utc::now();
    // The image gets about a third of the height, never all of it.
    let image_rows = (rest.height / 3).clamp(4, 16);
    if rest.width >= 80 {
        let side_width = rest.width * 42 / 100;
        let main = Rect {
            width: rest.width - side_width - 1,
            ..rest
        };
        let side_area = Rect {
            x: rest.x + rest.width - side_width,
            width: side_width,
            ..rest
        };
        let used = draw_image(
            frame,
            state,
            theme,
            o.image_url.as_deref(),
            side_area,
            image_rows,
        );
        frame.render_widget(
            Paragraph::new(body(theme, r, detail, now, usize::from(main.width)))
                .wrap(Wrap { trim: false })
                .scroll((state.detail_scroll, 0)),
            main,
        );
        frame.render_widget(
            Paragraph::new(side(theme, r, detail, usize::from(side_area.width)))
                .wrap(Wrap { trim: false }),
            below(side_area, used),
        );
    } else {
        let width = usize::from(rest.width);
        let used = draw_image(
            frame,
            state,
            theme,
            o.image_url.as_deref(),
            rest,
            image_rows,
        );
        let mut lines = body(theme, r, detail, now, width);
        lines.push(Line::default());
        lines.extend(side(theme, r, detail, width));
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((state.detail_scroll, 0)),
            below(rest, used),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_paragraphs_and_bullets() {
        let text = "First **bold** paragraph.\n\nSecond one.\n- item one\n- item two that is long enough to wrap";
        let lines = paragraphs(text, 20);
        assert_eq!(lines[0], "First bold");
        assert_eq!(lines[1], "paragraph.");
        assert_eq!(lines[2], "", "blank line between paragraphs");
        assert_eq!(lines[3], "Second one.");
        assert_eq!(lines[4], "", "blank line before the list");
        assert_eq!(lines[5], "• item one");
        assert_eq!(lines[6], "• item two that is");
        assert_eq!(lines[7], "  long enough to");
        assert!(lines.iter().all(|l| l.width() <= 20), "{lines:#?}");
    }

    #[test]
    fn numbered_items_and_line_breaks_are_kept() {
        let text = "Steps:\n1. Apply online\n2) Interview with the team\n\nThanks\nThe team";
        let lines = paragraphs(text, 22);
        assert_eq!(
            lines,
            [
                "Steps:",
                "",
                "1. Apply online",
                "2. Interview with the",
                "   team",
                "",
                "Thanks",
                "The team"
            ]
        );
        assert_eq!(paragraphs("2026 was great", 40), ["2026 was great"]);
    }

    #[test]
    fn inline_bullets_become_a_list() {
        let lines = paragraphs("Perks: • remote • equity", 40);
        assert_eq!(lines, ["Perks:", "", "• remote", "• equity"]);
    }

    #[test]
    fn flattened_dash_lists_are_split_but_single_dashes_kept() {
        let lines = paragraphs("Details: - Place: UDEP - Time: 9:00", 40);
        assert_eq!(lines, ["Details:", "", "• Place: UDEP", "• Time: 9:00"]);
        let lines = paragraphs("Rust - the language - is fast", 40);
        assert_eq!(lines, ["Rust - the language - is fast"]);
    }

    #[test]
    fn far_dates_still_say_how_far() {
        let now = Utc::now();
        let old = date_value(now - chrono::Duration::days(71), now);
        assert!(old.ends_with("(71d ago)"), "{old}");
    }

    #[test]
    fn headings_fill_the_width() {
        let line = heading(&Theme::default(), "Tags", 20);
        assert_eq!(line.width(), 20);
    }
}
