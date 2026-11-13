//! Plain-text rendering for CLI commands. Works in any terminal and pipes.

use std::fmt::Write;

use chrono::{DateTime, Utc};
use sentri_app::Ranked;
use sentri_core::{Eligibility, FeedItem, InteractionState};

fn date(d: Option<DateTime<Utc>>) -> String {
    d.map_or_else(|| "—".into(), |d| d.format("%Y-%m-%d").to_string())
}

fn flags(state: &InteractionState) -> String {
    let marks = [
        (state.liked, '♥'),
        (state.saved, '★'),
        (state.applied, '✓'),
        (state.enrolled, '✎'),
        (state.completed, '◉'),
        (state.dismissed, '✗'),
    ];
    marks
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, c)| *c)
        .collect()
}

/// Category for opportunities; topic and stance for news.
fn kind_label(o: &sentri_core::Opportunity) -> String {
    match (o.content, o.stance) {
        (sentri_core::ContentKind::News, stance) => {
            let topic = o.tags.first().map_or("news", String::as_str);
            let stance = stance.map_or("", sentri_core::Stance::label);
            format!("{topic}·{stance}")
        }
        _ => o.category.label().to_owned(),
    }
}

/// One line per item: `id  score  badge  category  title  [deadline]  flags`.
pub fn feed(items: &[Ranked]) -> String {
    if items.is_empty() {
        return "No opportunities match.\n".into();
    }
    let mut out = String::new();
    for r in items {
        let o = &r.item.opportunity;
        let deadline = o
            .dates
            .deadline
            .map(|d| format!("  ⏳ {}", d.format("%Y-%m-%d")))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "{:>5}  {:>4}  {:<3}  {:<18}  {}{}  {}",
            r.item.id,
            r.relevance.score,
            o.source.badge(),
            kind_label(o),
            o.title,
            deadline,
            flags(&r.item.state),
        );
    }
    out
}

/// Full details of one item, including the relevance explanation.
pub fn details(r: &Ranked) -> String {
    let FeedItem {
        id,
        opportunity: o,
        state,
    } = &r.item;
    let mut out = format!("#{id}  {}\n", o.title);
    let mut field = |name: &str, value: String| {
        let _ = writeln!(out, "  {name:<13} {value}");
    };
    field(
        "Source",
        format!(
            "[{}] {} ({})",
            o.source.badge(),
            o.source.label(),
            o.source_name
        ),
    );
    field("Category", o.category.label().into());
    field("Modality", o.modality.label().into());
    if let Some(location) = &o.location {
        field("Location", location.clone());
    }
    if let Some(country) = &o.country {
        field("Country", country.clone());
    }
    field("Cost", o.cost.label());
    field("Published", date(o.dates.published_at));
    field("Discovered", date(Some(o.discovered_at)));
    field("Starts", date(o.dates.starts_at));
    field("Ends", date(o.dates.ends_at));
    field("Deadline", date(o.dates.deadline));
    field(
        "Eligibility",
        match o.eligibility {
            Eligibility::Open => "Open to your country",
            Eligibility::NotEligible => "Not open to your country",
            Eligibility::Unknown => "Not stated",
        }
        .into(),
    );
    let yes_no = |v: Option<bool>| match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "not stated",
    };
    field("Certificate", yes_no(o.perks.certificate).into());
    field("Travel", yes_no(o.perks.travel_covered).into());
    field("Lodging", yes_no(o.perks.accommodation_covered).into());
    if let Some(prizes) = &o.perks.prizes {
        field("Prizes", prizes.clone());
    }
    if let Some(benefits) = &o.perks.benefits {
        field("Benefits", benefits.clone());
    }
    if let Some(req) = &o.requirements {
        field("Requirements", req.clone());
    }
    if !o.tags.is_empty() {
        field("Tags", o.tags.join(", "));
    }
    let status = flags(state);
    if !status.is_empty() {
        field("Status", status);
    }
    field("Link", o.original_url.clone());
    if let Some(summary) = &o.summary {
        let _ = writeln!(out, "\n{summary}");
    }
    let _ = writeln!(out, "\nRelevance: {}", r.relevance.score);
    if r.relevance.reasons.is_empty() {
        out.push_str("  (no rule matched)\n");
    }
    for reason in &r.relevance.reasons {
        let _ = writeln!(out, "  {:>+4}  {}", reason.points, reason.text);
    }
    out
}
