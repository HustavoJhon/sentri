//! Cards: the one visual system every feed section uses (Discover, Jobs,
//! Events, Communities, Courses, Tech News, Open Source, Call for Speakers…).
//!
//! Anatomy, top to bottom (Comfortable shows rows 1–3, Detailed adds the
//! summary, Compact squeezes everything into one aligned row):
//!
//! ```text
//! ┃ HACKATHON   DP Devpost                       NEW ★ ●●●○○
//! ┃ AI Hackathon Cusco
//! ┃ ⏳ closes in 5d · GDG Cusco · Cusco · ⌂ in person · ✈ travel
//! ```
//!
//! - **Rail** (first column): the card's state at a glance. `▌` yellow when
//!   selected, `┃` in the category color for important items, green for a
//!   strong match, `│` gray when expired. In ASCII mode it becomes `>` / `|`,
//!   so the selection never depends on color.
//! - **Kicker**: kind (category or news classification) and source.
//! - **Status** (right): new, your marks (♥ ★ ✓…), closed, and the match
//!   meter. The meter is a small 5-step scale of SENTRI's relevance score: visible
//!   but quieter than the title.
//! - **Title**, then per-kind **facts** (dates, organizer, place, salary…).

use chrono::{DateTime, Duration, Local, Timelike, Utc};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use sentri_app::Ranked;
use sentri_core::{Category, ContentKind, InteractionState, Modality, Opportunity, Stance};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::widgets::{distance, relative};
use crate::state::Density;
use crate::theme::Theme;

/// Rows each card takes, spacer included.
pub const fn rows(density: Density) -> u16 {
    match density {
        Density::Compact => 1,
        Density::Comfortable => 4,
        Density::Detailed => 6,
    }
}

/// Width of the kind column, so titles line up from card to card.
const TAG_WIDTH: usize = 11;

/// Discovered in the last day and never opened.
pub fn is_new(r: &Ranked, now: DateTime<Utc>) -> bool {
    r.item.state.last_viewed_at.is_none()
        && now - r.item.opportunity.discovered_at < Duration::hours(24)
}

/// The deadline passed, or the event already ended, with nothing left open.
pub fn is_expired(o: &Opportunity, now: DateTime<Utc>) -> bool {
    if o.content == ContentKind::News {
        return false;
    }
    if let Some(deadline) = o.dates.deadline {
        return deadline < now;
    }
    let end = o.dates.ends_at.or(o.dates.starts_at);
    end.is_some_and(|end| sentri_core::dates::effective_end(end) < now)
}

/// Categories and perks that deserve an accent.
pub fn is_important(o: &Opportunity) -> bool {
    o.content != ContentKind::News
        && (matches!(
            o.category,
            Category::Hackathon
                | Category::Scholarship
                | Category::Fellowship
                | Category::Exchange
                | Category::Job
                | Category::Internship
        ) || o.perks.offers_travel()
            || o.perks.prizes.is_some()
            || (o.category.is_learning() && o.cost.is_free()))
}

/// A job or internship that matches the profile well.
pub fn is_strong_match(r: &Ranked) -> bool {
    r.item.opportunity.category.is_job() && r.relevance.score >= 50
}

/// What the rail says about a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Important,
    StrongMatch,
    Expired,
    Dismissed,
}

pub fn tone(r: &Ranked, now: DateTime<Utc>) -> Tone {
    let o = &r.item.opportunity;
    if r.item.state.dismissed {
        Tone::Dismissed
    } else if is_expired(o, now) {
        Tone::Expired
    } else if is_strong_match(r) {
        Tone::StrongMatch
    } else if is_important(o) {
        Tone::Important
    } else {
        Tone::Normal
    }
}

fn rail(theme: &Theme, tone: Tone, category: Category, selected: bool) -> Span<'static> {
    if selected {
        return Span::styled(
            "▌",
            Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD),
        );
    }
    match tone {
        Tone::Important => Span::styled("┃", Style::new().fg(theme.category_color(category))),
        Tone::StrongMatch => Span::styled("┃", Style::new().fg(theme.p.green)),
        Tone::Normal => Span::styled("│", Style::new().fg(theme.p.bg2)),
        Tone::Expired | Tone::Dismissed => Span::styled("│", Style::new().fg(theme.p.bg1)),
    }
}

/// Short, fixed-width kind label.
pub const fn tag(category: Category) -> &'static str {
    match category {
        Category::Event => "EVENT",
        Category::Conference => "CONFERENCE",
        Category::Workshop => "WORKSHOP",
        Category::Meetup => "MEETUP",
        Category::Course => "COURSE",
        Category::Training => "TRAINING",
        Category::Hackathon => "HACKATHON",
        Category::Competition => "CONTEST",
        Category::Scholarship => "SCHOLARSHIP",
        Category::Exchange => "EXCHANGE",
        Category::Fellowship => "FELLOWSHIP",
        Category::Volunteering => "VOLUNTEER",
        Category::Call => "OPEN CALL",
        Category::Job => "JOB",
        Category::Internship => "INTERNSHIP",
        Category::OpenSource => "OPEN SOURCE",
        Category::Community => "COMMUNITY",
        Category::CallForSpeakers => "CFP",
        Category::Other => "OTHER",
    }
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

/// Kind label: category for opportunities, classification for news.
fn kind(theme: &Theme, o: &Opportunity, muted: bool) -> Span<'static> {
    let (text, color) = if o.content == ContentKind::News {
        let stance = o.stance.unwrap_or(Stance::Report);
        (stance.label().to_uppercase(), stance_color(theme, stance))
    } else {
        (tag(o.category).to_owned(), theme.category_color(o.category))
    };
    let color = if muted { theme.p.gray } else { color };
    Span::styled(
        format!("{text:<TAG_WIDTH$}"),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    )
}

/// Source badge in the source's color plus the publisher's name.
fn source(theme: &Theme, o: &Opportunity, with_name: bool) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        format!("{:<2}", o.source.badge()),
        Style::new()
            .fg(theme.source_color(o.source))
            .add_modifier(Modifier::BOLD),
    )];
    if with_name {
        spans.push(Span::styled(format!(" {}", publisher(o)), theme.muted()));
    }
    spans
}

/// The platform's name, or the configured feed's name for generic sources
/// (RSS, iCal, web pages), where it is the publisher.
pub fn publisher(o: &Opportunity) -> String {
    use sentri_core::SourceKind as K;
    match o.source {
        K::Rss | K::ICal | K::Web | K::Manual => o.source_name.clone(),
        kind => kind.label().to_owned(),
    }
}

/// Your marks, each in its own color.
pub fn marks(theme: &Theme, state: &InteractionState) -> Vec<Span<'static>> {
    [
        (state.liked, "♥", theme.p.red),
        (state.saved, "★", theme.p.yellow),
        (state.interesting, "!", theme.p.orange),
        (state.applied, "✓", theme.p.green),
        (state.enrolled, "✎", theme.p.blue),
        (state.completed, "◉", theme.p.aqua),
    ]
    .into_iter()
    .filter(|(on, _, _)| *on)
    .map(|(_, glyph, color)| Span::styled(glyph, Style::new().fg(color)))
    .collect()
}

/// Five-step scale of the relevance score.
pub fn meter_level(score: i32) -> usize {
    match score {
        i32::MIN..=0 => 0,
        1..=19 => 1,
        20..=34 => 2,
        35..=49 => 3,
        50..=69 => 4,
        _ => 5,
    }
}

pub fn meter(theme: &Theme, score: i32) -> Vec<Span<'static>> {
    let level = meter_level(score);
    vec![
        Span::styled("●".repeat(level), Style::new().fg(theme.score_color(score))),
        Span::styled("○".repeat(5 - level), Style::new().fg(theme.p.bg3)),
    ]
}

/// Right-hand status cluster.
fn status(
    theme: &Theme,
    r: &Ranked,
    now: DateTime<Utc>,
    tone: Tone,
    full: bool,
) -> Vec<Span<'static>> {
    let o = &r.item.opportunity;
    let mut out: Vec<Span> = Vec::new();
    let gap = |out: &mut Vec<Span<'static>>| {
        if !out.is_empty() {
            out.push(Span::raw(" "));
        }
    };
    if is_new(r, now) {
        out.push(Span::styled(" NEW ", theme.badge(theme.p.green)));
    }
    let marks = marks(theme, &r.item.state);
    if !marks.is_empty() {
        gap(&mut out);
        out.extend(marks);
    }
    match tone {
        Tone::Expired => {
            gap(&mut out);
            out.push(Span::styled("closed", theme.muted()));
        }
        Tone::Dismissed => {
            gap(&mut out);
            out.push(Span::styled("✗ dismissed", theme.muted()));
        }
        _ => {}
    }
    if o.content == ContentKind::News {
        if full {
            gap(&mut out);
            let published = o.dates.published_at.unwrap_or(o.discovered_at);
            out.push(Span::styled(relative(published, now), theme.muted()));
        }
    } else if full {
        gap(&mut out);
        out.extend(meter(theme, r.relevance.score));
    } else {
        gap(&mut out);
        out.push(Span::styled(
            format!("{:>3}", r.relevance.score),
            Style::new().fg(theme.score_color(r.relevance.score)),
        ));
    }
    out
}

/// `Fri 20 Nov`, `Fri 20 Nov 18:00`, or `20 Nov 2027` outside this year.
pub fn short_date(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    use chrono::Datelike;
    use std::fmt::Write;
    let day = sentri_core::dates::calendar_date(at, |d| d.with_timezone(&Local).date_naive());
    let mut out = if day.year() == now.with_timezone(&Local).year() {
        day.format("%a %d %b").to_string()
    } else {
        day.format("%d %b %Y").to_string()
    };
    if !sentri_core::dates::is_all_day(at) {
        let local = at.with_timezone(&Local);
        let _ = write!(out, " {:02}:{:02}", local.hour(), local.minute());
    }
    out
}

/// A salary as published (`USD 1200–1800 / month`, `S/ 3000`, `$90k`).
pub fn salary(o: &Opportunity) -> Option<&str> {
    let text = o.perks.benefits.as_deref()?.trim();
    let currency = text.split_whitespace().next().unwrap_or("");
    let is_code = currency.len() == 3 && currency.chars().all(|c| c.is_ascii_uppercase());
    let is_symbol = ["S/", "$", "€", "£"].iter().any(|s| text.starts_with(s));
    (o.category.is_job() && (is_code || is_symbol) && text.chars().any(|c| c.is_ascii_digit()))
        .then_some(text)
}

pub const fn modality(m: Modality) -> &'static str {
    match m {
        Modality::Online => "◎ remote",
        Modality::InPerson => "⌂ on-site",
        Modality::Hybrid => "◐ hybrid",
        Modality::Unknown => "",
    }
}

const fn event_modality(m: Modality) -> &'static str {
    match m {
        Modality::Online => "◎ online",
        Modality::InPerson => "⌂ in person",
        Modality::Hybrid => "◐ hybrid",
        Modality::Unknown => "",
    }
}

fn deadline_color(theme: &Theme, date: DateTime<Utc>, now: DateTime<Utc>) -> Color {
    let days = (date - now).num_days();
    if date < now {
        theme.p.gray
    } else if days <= 3 {
        theme.p.red
    } else if days <= 14 {
        theme.p.orange
    } else {
        theme.p.fg4
    }
}

/// Tags worth showing (source names repeated as tags are noise).
fn skills(o: &Opportunity) -> Vec<&str> {
    let source = o.source_name.to_lowercase();
    o.tags
        .iter()
        .map(String::as_str)
        .filter(|t| {
            let t = t.to_lowercase();
            t != source && t != o.source.as_str() && !source.contains(&t)
        })
        .take(3)
        .collect()
}

/// The date that matters most for an opportunity that is not a job.
fn when(theme: &Theme, o: &Opportunity, now: DateTime<Utc>) -> Vec<Span<'static>> {
    let mut parts = Vec::new();
    let dated = o.category.is_dated_event() || o.category == Category::Community;
    if o.category == Category::CallForSpeakers {
        if let Some(deadline) = o.dates.deadline {
            parts.push(Span::styled(
                format!("⏳ CFP closes {}", relative(deadline, now)),
                Style::new().fg(deadline_color(theme, deadline, now)),
            ));
        }
        if let Some(start) = o.dates.starts_at {
            parts.push(Span::styled(
                format!("◷ {}", short_date(start, now)),
                Style::new().fg(theme.p.blue),
            ));
        }
    } else if let (true, Some(start)) = (dated, o.dates.starts_at) {
        parts.push(Span::styled(
            format!("◷ {} · {}", short_date(start, now), relative(start, now)),
            Style::new().fg(theme.p.blue),
        ));
        if let Some(deadline) = o.dates.deadline.filter(|d| *d >= now && *d < start) {
            parts.push(Span::styled(
                format!("⏳ register {}", relative(deadline, now)),
                Style::new().fg(deadline_color(theme, deadline, now)),
            ));
        }
    } else if let Some(deadline) = o.dates.deadline {
        parts.push(Span::styled(
            format!("⏳ closes {}", relative(deadline, now)),
            Style::new().fg(deadline_color(theme, deadline, now)),
        ));
    } else if let Some(start) = o.dates.starts_at {
        parts.push(Span::styled(
            format!("◷ {}", short_date(start, now)),
            Style::new().fg(theme.p.blue),
        ));
    }
    parts
}

/// The facts row, chosen per kind of item.
pub fn facts<'a>(theme: &Theme, o: &'a Opportunity, now: DateTime<Utc>) -> Vec<Span<'a>> {
    let mut parts: Vec<Span> = Vec::new();
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    // Organizers named like the item (communities) and "Remote" next to
    // the remote marker add nothing.
    let org = o.author.display().filter(|org| !same(org, &o.title));
    let place = o
        .location
        .as_deref()
        .filter(|p| !(o.modality == Modality::Online && same(p, "remote")));
    if o.content == ContentKind::News {
        if let Some(topic) = o.news_topic() {
            parts.push(Span::styled(topic, Style::new().fg(theme.p.aqua)));
        }
        // The publisher is already in the kicker.
        if let Some(author) = org {
            parts.push(Span::styled(format!("by {author}"), theme.dim()));
        }
        let published = o.dates.published_at.unwrap_or(o.discovered_at);
        parts.push(Span::styled(short_date(published, now), theme.muted()));
        return parts;
    }
    if o.category.is_job() {
        if let Some(pay) = salary(o) {
            parts.push(Span::styled(pay, theme.good().add_modifier(Modifier::BOLD)));
        }
        let m = modality(o.modality);
        if !m.is_empty() {
            parts.push(Span::styled(m, Style::new().fg(theme.p.blue)));
        }
        if let Some(place) = place {
            parts.push(Span::styled(place, theme.dim()));
        }
        if let Some(published) = o.dates.published_at {
            parts.push(Span::styled(
                format!("posted {}", distance(published, now)),
                theme.muted(),
            ));
        }
        if let Some(deadline) = o.dates.deadline.filter(|d| *d >= now) {
            parts.push(Span::styled(
                format!("⏳ apply {}", relative(deadline, now)),
                Style::new().fg(deadline_color(theme, deadline, now)),
            ));
        }
        let skills = skills(o);
        if !skills.is_empty() {
            parts.push(Span::styled(
                skills.join(" "),
                Style::new().fg(theme.p.aqua),
            ));
        }
        return parts;
    }
    parts.extend(when(theme, o, now));
    if let Some(org) = org {
        parts.push(Span::styled(org, Style::new().fg(theme.p.aqua)));
    }
    if let Some(place) = place {
        parts.push(Span::styled(place, theme.dim()));
    }
    let m = event_modality(o.modality);
    if !m.is_empty() {
        parts.push(Span::styled(m, theme.dim()));
    }
    if o.cost.is_free() {
        parts.push(Span::styled("free", theme.good()));
    }
    if o.perks.certificate == Some(true) && o.category.is_learning() {
        parts.push(Span::styled("certificate", Style::new().fg(theme.p.blue)));
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

/// Spans cut to `width` display columns, ending in `…` when cut.
pub fn truncate(spans: Vec<Span<'_>>, width: usize) -> Vec<Span<'_>> {
    let total: usize = spans.iter().map(Span::width).sum();
    if total <= width {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    let room = width.saturating_sub(1);
    for span in spans {
        let w = span.width();
        if used + w <= room {
            used += w;
            out.push(span);
            continue;
        }
        let mut text = String::new();
        for ch in span.content.chars() {
            let cw = ch.width().unwrap_or(0);
            if used + cw > room {
                break;
            }
            used += cw;
            text.push(ch);
        }
        let style = span.style;
        out.push(Span::styled(text, style));
        out.push(Span::styled("…", style));
        return out;
    }
    out
}

/// Left content cut to make room for the right cluster, padded between.
fn spread<'a>(left: Vec<Span<'a>>, right: Vec<Span<'a>>, width: usize) -> Vec<Span<'a>> {
    let right_width: usize = right.iter().map(Span::width).sum();
    if right_width + 2 > width {
        return truncate(left, width);
    }
    let left = truncate(left, width - right_width - 1);
    let left_width: usize = left.iter().map(Span::width).sum();
    let mut out = left;
    out.push(Span::raw(" ".repeat(width - left_width - right_width)));
    out.extend(right);
    out
}

fn joined<'a>(theme: &Theme, parts: Vec<Span<'a>>) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
    for (i, part) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::new().fg(theme.p.bg3)));
        }
        spans.push(part);
    }
    spans
}

/// Word-wrapped by display width, at most `lines` lines (the last one ends
/// in `…` when text remains).
pub fn wrap(text: &str, width: usize, lines: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let word = words[i];
        let needed = if current.is_empty() {
            word.width()
        } else {
            current.width() + 1 + word.width()
        };
        if needed <= width {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            i += 1;
        } else if current.is_empty() {
            // A word longer than the line: hard cut.
            let (head, _) = cut(word, width);
            out.push(head);
            i += 1;
        } else {
            out.push(std::mem::take(&mut current));
        }
        if out.len() == lines {
            break;
        }
    }
    if out.len() < lines && !current.is_empty() {
        out.push(current);
    }
    let consumed: usize = out.iter().map(|l| l.split_whitespace().count()).sum();
    if consumed < words.len()
        && let Some(last) = out.last_mut()
    {
        let (head, _) = cut(last, width.saturating_sub(1));
        *last = format!("{}…", head.trim_end());
    }
    out
}

fn cut(text: &str, width: usize) -> (String, usize) {
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        out.push(ch);
    }
    (out, used)
}

/// Everything needed to lay a card out.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub density: Density,
    /// Columns available inside the list.
    pub width: u16,
    pub selected: bool,
    pub now: DateTime<Utc>,
}

/// The lines of one card.
pub fn lines<'a>(theme: &Theme, r: &'a Ranked, at: Layout) -> Vec<Line<'a>> {
    let o = &r.item.opportunity;
    let tone = tone(r, at.now);
    let rail = rail(theme, tone, o.category, at.selected);
    let muted = matches!(tone, Tone::Expired | Tone::Dismissed);
    // Rail and one space of padding.
    let width = usize::from(at.width).saturating_sub(2);
    let row = |spans: Vec<Span<'a>>| {
        let mut all = vec![rail.clone(), Span::raw(" ")];
        all.extend(spans);
        Line::from(all)
    };
    let mut title_style = if muted {
        theme.muted()
    } else if at.selected {
        Style::new().fg(theme.p.yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.p.fg).add_modifier(Modifier::BOLD)
    };
    if tone == Tone::Dismissed {
        title_style = title_style.add_modifier(Modifier::CROSSED_OUT);
    }
    let mut title = vec![Span::styled(o.title.as_str(), title_style)];
    if o.category.is_job()
        && o.content != ContentKind::News
        && let Some(company) = o.author.display()
    {
        title.push(Span::styled(" · ", Style::new().fg(theme.p.bg3)));
        title.push(Span::styled(company, Style::new().fg(theme.p.aqua)));
    }

    if at.density == Density::Compact {
        let mut left = vec![kind(theme, o, muted)];
        left.extend(source(theme, o, false));
        left.push(Span::raw(" "));
        left.extend(title);
        let mut right: Vec<Span> = facts(theme, o, at.now).into_iter().take(1).collect();
        let status = status(theme, r, at.now, tone, false);
        if !right.is_empty() && !status.is_empty() {
            right.push(Span::raw("  "));
        }
        right.extend(status);
        return vec![row(spread(left, right, width))];
    }

    let mut kicker = vec![kind(theme, o, muted), Span::raw(" ")];
    kicker.extend(source(theme, o, true));
    let mut out = vec![
        row(spread(kicker, status(theme, r, at.now, tone, true), width)),
        row(truncate(title, width)),
    ];
    if at.density == Density::Detailed {
        let summary = o.summary.as_deref().unwrap_or("");
        let mut text = wrap(summary, width, 2);
        if text.is_empty() {
            text.push("No description.".to_owned());
        }
        text.resize(2, String::new());
        for line in text {
            out.push(row(vec![Span::styled(line, theme.dim())]));
        }
    }
    out.push(row(truncate(joined(theme, facts(theme, o, at.now)), width)));
    out.push(Line::default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentri_core::{CanonicalUrl, FeedItem, OpportunityId, Reason, Relevance, SourceKind};

    fn ranked(title: &str, category: Category) -> Ranked {
        let url = CanonicalUrl::parse("https://example.com/x").unwrap();
        let mut o = Opportunity::new(
            SourceKind::GetOnBoard,
            "getonboard",
            url.as_str(),
            &url,
            title,
            Utc::now(),
        );
        o.category = category;
        Ranked {
            item: FeedItem {
                id: OpportunityId(1),
                opportunity: o,
                state: InteractionState::default(),
            },
            relevance: Relevance {
                score: 55,
                reasons: vec![Reason {
                    points: 55,
                    text: "Rust".into(),
                }],
            },
        }
    }

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn layout(density: Density, width: u16) -> Layout {
        Layout {
            density,
            width,
            selected: false,
            now: Utc::now(),
        }
    }

    #[test]
    fn job_cards_show_company_salary_and_modality_with_a_quiet_meter() {
        let mut r = ranked("Senior Rust Engineer", Category::Job);
        let o = &mut r.item.opportunity;
        o.author.name = Some("Acme".into());
        o.perks.benefits = Some("USD 3500–4000 / month".into());
        o.modality = Modality::Online;
        o.location = Some("Peru".into());
        let lines = lines(&Theme::default(), &r, layout(Density::Comfortable, 70));
        let rows = text(&lines);
        assert_eq!(rows.len(), 4);
        assert!(
            rows[0].starts_with("┃ JOB"),
            "strong match rail: {:?}",
            rows[0]
        );
        assert!(rows[0].ends_with("●●●●○"), "{:?}", rows[0]);
        assert!(rows[1].contains("Senior Rust Engineer · Acme"));
        assert!(rows[2].contains("USD 3500–4000 / month · ◎ remote · Peru"));
        assert!(rows.iter().all(|r| r.width() <= 70), "{rows:#?}");
    }

    #[test]
    fn news_cards_keep_their_classification() {
        let mut r = ranked("Chip maker cuts jobs", Category::Other);
        r.item.opportunity.content = ContentKind::News;
        r.item.opportunity.stance = Some(Stance::Speculation);
        let rows = text(&lines(
            &Theme::default(),
            &r,
            layout(Density::Comfortable, 60),
        ));
        assert!(rows[0].contains("UNCONFIRMED"), "{:?}", rows[0]);
        assert!(!rows[0].contains('●'), "no match meter on news");
    }

    #[test]
    fn selection_expiry_and_dismissal_change_the_rail() {
        let theme = Theme::default();
        let mut r = ranked("Old hackathon", Category::Hackathon);
        r.relevance.score = 10;
        let now = Utc::now();
        assert_eq!(tone(&r, now), Tone::Important);
        r.item.opportunity.dates.deadline = Some(now - Duration::days(2));
        assert_eq!(tone(&r, now), Tone::Expired);
        let rows = text(&lines(&theme, &r, layout(Density::Comfortable, 60)));
        assert!(rows[0].contains("closed"));
        let selected = Layout {
            selected: true,
            ..layout(Density::Comfortable, 60)
        };
        let rows = text(&lines(&theme, &r, selected));
        assert!(rows.iter().take(3).all(|r| r.starts_with('▌')), "{rows:?}");
        r.item.state.dismissed = true;
        assert_eq!(tone(&r, now), Tone::Dismissed);
    }

    #[test]
    fn densities_have_fixed_heights_and_never_overflow() {
        let mut r = ranked(
            &"Very long title with 日本語 double width glyphs ".repeat(6),
            Category::Course,
        );
        r.item.opportunity.summary = Some("word ".repeat(200));
        r.item.state.liked = true;
        r.item.state.saved = true;
        for density in [Density::Compact, Density::Comfortable, Density::Detailed] {
            for width in [20, 40, 58, 100] {
                let lines = lines(&Theme::default(), &r, layout(density, width));
                assert_eq!(lines.len(), usize::from(rows(density)), "{density:?}");
                for line in &lines {
                    assert!(
                        line.width() <= usize::from(width),
                        "{density:?} {width}: {line:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn compact_rows_align_titles() {
        let theme = Theme::default();
        let a = text(&lines(
            &theme,
            &ranked("Zq1", Category::Job),
            layout(Density::Compact, 60),
        ));
        let b = text(&lines(
            &theme,
            &ranked("Zq2", Category::Scholarship),
            layout(Density::Compact, 60),
        ));
        assert_eq!(a[0].find("Zq1"), b[0].find("Zq2"));
    }

    #[test]
    fn wraps_summaries_by_display_width() {
        assert_eq!(wrap("one two three four", 9, 2), ["one two", "three…"]);
        assert_eq!(wrap("short", 10, 2), ["short"]);
        assert_eq!(wrap("", 10, 2), Vec::<String>::new());
        let cjk = wrap("日本語日本語日本語", 6, 1);
        assert!(cjk[0].width() <= 6, "{cjk:?}");
    }

    #[test]
    fn salaries_need_a_currency_and_a_number() {
        let mut r = ranked("x", Category::Job);
        for (benefits, expected) in [
            ("USD 1200–1800 / month", true),
            ("S/ 3000", true),
            ("Health insurance", false),
            ("USD", false),
        ] {
            r.item.opportunity.perks.benefits = Some(benefits.into());
            assert_eq!(
                salary(&r.item.opportunity).is_some(),
                expected,
                "{benefits}"
            );
        }
    }

    #[test]
    fn meter_scale() {
        assert_eq!(meter_level(-3), 0);
        assert_eq!(meter_level(42), 3);
        assert_eq!(meter_level(90), 5);
    }
}
