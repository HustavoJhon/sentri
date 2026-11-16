//! Devpost hackathons via the JSON endpoint used by devpost.com itself
//! (`/api/hackathons`).
//!
//! It is not a documented public API: it may change without notice. It is
//! allowed by Devpost's `robots.txt` and its Terms of Service do not forbid
//! it (checked 2026-11-16). One request per run; opt-in in the default
//! configuration.

use chrono::{DateTime, NaiveDate, Utc};
use sentri_core::{Author, CanonicalUrl, Category, Modality, Opportunity, SourceKind, dates};
use serde_json::Value;
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::html_to_text;

const API: &str = "https://devpost.com/api/hackathons";

#[derive(Debug, Clone)]
pub struct Devpost {
    pub(crate) spec: SourceSpec,
    url: Url,
}

impl Devpost {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let mut url = Url::parse(spec.url.as_deref().unwrap_or(API))
            .map_err(|e| SourceError::Config(e.to_string()))?;
        if url.query().is_none() {
            let per_page = spec.max_items.clamp(1, 50).to_string();
            url.query_pairs_mut()
                .append_pair("status[]", "upcoming")
                .append_pair("status[]", "open")
                .append_pair("order_by", "deadline")
                .append_pair("per_page", &per_page);
            if let Some(q) = spec.query.as_deref().filter(|q| !q.trim().is_empty()) {
                url.query_pairs_mut().append_pair("search", q);
            }
        }
        Ok(Self { spec, url })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.url).await?;
        let json: Value =
            serde_json::from_slice(&page.body).map_err(|e| SourceError::Parse(e.to_string()))?;
        parse(&json, &self.spec.name, now)
    }
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%b %d, %Y").ok()
}

/// Parses "Oct 30 - Dec 04, 2026", "Nov 01 - 30, 2026" and
/// "Dec 01, 2026 - Jan 15, 2027" into (start, end).
pub fn submission_period(s: &str) -> Option<(NaiveDate, NaiveDate)> {
    let (a, b) = s.split_once(" - ")?;
    let end = date(b).or_else(|| {
        // "Nov 01 - 30, 2026": end has only the day.
        let (day, year) = b.split_once(", ")?;
        let month = a.split_whitespace().next()?;
        date(&format!("{month} {day}, {year}"))
    })?;
    let start = date(a).or_else(|| date(&format!("{a}, {}", end.format("%Y"))))?;
    // A period crossing the new year without an explicit start year.
    let start = if start > end {
        use chrono::Datelike;
        start.with_year(start.year() - 1)?
    } else {
        start
    };
    Some((start, end))
}

/// Removes markup from short inline values such as `$<span>10,000</span>`.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    html_to_text(out.trim())
}

pub fn parse(
    json: &Value,
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let list = json
        .get("hackathons")
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("missing `hackathons` array".into()))?;
    Ok(list
        .iter()
        .filter_map(|h| hackathon(h, source_name, now))
        .collect())
}

fn hackathon(h: &Value, source_name: &str, now: DateTime<Utc>) -> Option<Opportunity> {
    let s = |k: &str| {
        h.get(k)
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
    };
    let link = s("url")?;
    let canonical = CanonicalUrl::parse(link).ok()?;
    let mut o = Opportunity::new(
        SourceKind::Devpost,
        source_name,
        link,
        &canonical,
        s("title")?,
        now,
    );
    o.category = Category::Hackathon;
    o.external_id = h.get("id").map(Value::to_string);
    if let Some((start, end)) = s("submission_period_dates").and_then(submission_period) {
        o.dates.starts_at = Some(dates::all_day(start));
        o.dates.deadline = Some(dates::all_day(end));
    }
    let location = h
        .get("displayed_location")
        .and_then(|l| l.get("location"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if location.eq_ignore_ascii_case("online") {
        o.modality = Modality::Online;
    } else if !location.is_empty() {
        o.modality = Modality::InPerson;
        o.location = Some(location.to_owned());
    }
    if let Some(prize) = s("prize_amount")
        .map(strip_tags)
        .filter(|p| p != "$0" && !p.is_empty())
    {
        o.perks.prizes = Some(format!("{prize} in prizes"));
    }
    if h.get("invite_only").and_then(Value::as_bool) == Some(true) {
        o.requirements = Some("Invite only".into());
    }
    o.author = Author {
        name: s("organization_name").map(str::to_owned),
        ..Author::default()
    };
    o.tags = h
        .get("themes")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(|x| x.get("name")?.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let registrations = h.get("registrations_count").and_then(Value::as_u64);
    o.summary = registrations.map(|n| format!("{n} participants registered on Devpost"));
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parses_submission_periods() {
        assert_eq!(
            submission_period("Oct 30 - Dec 04, 2026"),
            Some((d(2026, 10, 30), d(2026, 12, 4)))
        );
        assert_eq!(
            submission_period("Nov 01 - 30, 2026"),
            Some((d(2026, 11, 1), d(2026, 11, 30)))
        );
        assert_eq!(
            submission_period("Dec 01, 2026 - Jan 15, 2027"),
            Some((d(2026, 12, 1), d(2027, 1, 15)))
        );
        assert_eq!(
            submission_period("Dec 20 - Jan 10, 2027"),
            Some((d(2026, 12, 20), d(2027, 1, 10)))
        );
        assert_eq!(submission_period("soon"), None);
    }

    #[test]
    fn maps_hackathons() {
        let body = json!({"hackathons": [
            {"id": 31667, "title": "Kiroween 2026", "url": "https://kiroween.devpost.com/",
             "displayed_location": {"location": "Online"}, "submission_period_dates": "Oct 30 - Dec 04, 2026",
             "prize_amount": "$<span data-currency-value>100,000</span>", "organization_name": "Kiro",
             "themes": [{"name": "Machine Learning/AI"}], "registrations_count": 14341, "invite_only": false},
            {"id": 2, "title": "Lima Hack", "url": "https://lima.devpost.com/",
             "displayed_location": {"location": "Lima, Peru"}, "submission_period_dates": "bad",
             "prize_amount": "$<span data-currency-value>0</span>", "invite_only": true},
            {"id": 3, "title": "No url"}
        ]});
        let items = parse(&body, "devpost", Utc::now()).unwrap();
        assert_eq!(items.len(), 2);
        let k = &items[0];
        assert_eq!(k.category, Category::Hackathon);
        assert_eq!(k.modality, Modality::Online);
        assert_eq!(k.perks.prizes.as_deref(), Some("$100,000 in prizes"));
        assert_eq!(k.author.name.as_deref(), Some("Kiro"));
        assert_eq!(k.dates.deadline, Some(dates::all_day(d(2026, 12, 4))));
        let l = &items[1];
        assert_eq!(l.location.as_deref(), Some("Lima, Peru"));
        assert_eq!(l.perks.prizes, None);
        assert_eq!(l.requirements.as_deref(), Some("Invite only"));
        assert_eq!(l.dates.deadline, None, "unparseable dates stay unknown");
        assert!(parse(&json!({"error": 1}), "d", Utc::now()).is_err());
    }
}
