//! iCalendar (RFC 5545) connector for Luma, Meetup, GDG and any `.ics` feed.
//!
//! The parser covers what public event feeds use: folded lines, escaped
//! text, UTC / `TZID` / all-day dates, and `VTIMEZONE` definitions with
//! yearly `BYMONTH`/`BYDAY` daylight-saving rules. Events without a public
//! URL, cancelled events and past events are skipped; nothing is invented.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc, Weekday};
use sentri_core::{Author, CanonicalUrl, Category, Modality, Opportunity, SourceKind, dates};
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::http::Validators;

/// Cap on parsed events, so a huge feed cannot exhaust memory.
const MAX_EVENTS: usize = 5_000;

/// A configured calendar feed.
#[derive(Debug, Clone)]
pub struct Calendar {
    pub(crate) spec: SourceSpec,
    url: Url,
}

impl Calendar {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let raw = spec
            .url
            .as_deref()
            .ok_or_else(|| SourceError::Config(format!("{}: `url` is required", spec.name)))?;
        // webcal:// is the same resource over https.
        let raw = raw
            .strip_prefix("webcal://")
            .map_or_else(|| raw.to_owned(), |r| format!("https://{r}"));
        let url = Url::parse(&raw).map_err(|e| SourceError::Config(e.to_string()))?;
        Ok(Self { spec, url })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        Ok(self
            .fetch_conditional(http, now, &Validators::default())
            .await?
            .0)
    }

    pub async fn fetch_conditional(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
        validators: &Validators,
    ) -> Result<(Vec<Opportunity>, Validators), SourceError> {
        let page = http.get_conditional(&self.url, validators).await?;
        let text = String::from_utf8_lossy(&page.body);
        let mut items = parse(&text, &self.spec.name, self.spec.kind, now)?;
        // Soonest first, so `max_items` keeps the nearest events.
        items.sort_by_key(|o| o.dates.starts_at);
        Ok((items, page.validators))
    }
}

/// A content line: name, parameters and unescaped value.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

/// Joins folded lines (a line starting with space or tab continues the
/// previous one).
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = line.strip_prefix([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push_str(rest);
        } else if !line.is_empty() {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn parse_line(line: &str) -> Option<Prop> {
    // The value starts at the first ':' that is not inside a quoted param.
    let mut quoted = false;
    let split = line.char_indices().find_map(|(i, c)| match c {
        '"' => {
            quoted = !quoted;
            None
        }
        ':' if !quoted => Some(i),
        _ => None,
    })?;
    let (head, value) = (&line[..split], &line[split + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_owned()))
        .collect();
    Some(Prop {
        name,
        params,
        value: unescape(value),
    })
}

/// One daylight-saving transition of a `VTIMEZONE`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Transition {
    offset_secs: i64,
    start: NaiveDateTime,
    /// `(month, nth weekday)` from `RRULE:FREQ=YEARLY;BYMONTH=..;BYDAY=..`.
    rule: Option<(u32, i32, Weekday)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TimeZone {
    transitions: Vec<Transition>,
}

fn parse_offset(s: &str) -> Option<i64> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => (1, s),
    };
    let h: i64 = rest.get(0..2)?.parse().ok()?;
    let m: i64 = rest.get(2..4)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

fn parse_rule(rrule: &str) -> Option<(u32, i32, Weekday)> {
    let mut month = None;
    let mut byday = None;
    for part in rrule.split(';') {
        match part.split_once('=')? {
            ("BYMONTH", m) => month = m.parse().ok(),
            ("BYDAY", d) => byday = Some(d.to_owned()),
            _ => {}
        }
    }
    let day = byday?;
    let (n, wd) = day.split_at(day.len().checked_sub(2)?);
    let weekday = match wd {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    };
    Some((month?, n.parse().unwrap_or(1), weekday))
}

/// Date of the `nth` (negative = from the end) `weekday` of a month.
fn nth_weekday(year: i32, month: u32, nth: i32, weekday: Weekday) -> Option<NaiveDate> {
    if nth > 0 {
        NaiveDate::from_weekday_of_month_opt(year, month, weekday, u8::try_from(nth).ok()?)
    } else {
        let next = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)?
        };
        let last = next - Duration::days(1);
        let back = (7 + last.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
        Some(last - Duration::days(i64::from(back) + 7 * i64::from(-nth - 1)))
    }
}

impl TimeZone {
    /// UTC offset in effect at local time `t`.
    fn offset_at(&self, t: NaiveDateTime) -> Option<i64> {
        // Find the most recent transition at or before `t`.
        self.transitions
            .iter()
            .filter_map(|tr| {
                let at = match tr.rule {
                    Some((month, nth, wd)) => {
                        let this_year =
                            nth_weekday(t.year(), month, nth, wd)?.and_time(tr.start.time());
                        if this_year <= t {
                            this_year
                        } else {
                            nth_weekday(t.year() - 1, month, nth, wd)?.and_time(tr.start.time())
                        }
                    }
                    None => tr.start,
                };
                (at <= t).then_some((at, tr.offset_secs))
            })
            .max_by_key(|(at, _)| *at)
            .map(|(_, offset)| offset)
    }
}

/// Fixed-offset zones commonly referenced without a `VTIMEZONE`.
fn known_zone(tzid: &str) -> Option<i64> {
    Some(match tzid {
        "UTC" | "Etc/UTC" | "GMT" => 0,
        "America/Lima" | "America/Bogota" | "America/Guayaquil" | "America/Panama" => -5 * 3600,
        "America/La_Paz" | "America/Caracas" => -4 * 3600,
        "America/Argentina/Buenos_Aires" | "America/Sao_Paulo" | "America/Montevideo" => -3 * 3600,
        _ => return None,
    })
}

fn parse_local(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()
}

/// Converts a `DTSTART`/`DTEND` property to UTC.
fn to_utc(
    prop: &Prop,
    zones: &std::collections::HashMap<String, TimeZone>,
) -> Option<DateTime<Utc>> {
    let v = prop.value.trim();
    if prop.param("VALUE") == Some("DATE") || v.len() == 8 {
        return NaiveDate::parse_from_str(v, "%Y%m%d")
            .ok()
            .map(dates::all_day);
    }
    let local = parse_local(v)?;
    if v.ends_with('Z') {
        return Some(local.and_utc());
    }
    let offset = prop
        .param("TZID")
        .and_then(|tz| {
            zones
                .get(tz)
                .and_then(|z| z.offset_at(local))
                .or_else(|| known_zone(tz))
        })
        .unwrap_or(0); // floating time without zone: treated as UTC
    Some((local - Duration::seconds(offset)).and_utc())
}

fn first_url(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '(' | ')' | '[' | ']'))
        .find(|w| w.starts_with("https://") || w.starts_with("http://"))
        .map(|w| w.trim_end_matches(['.', ',', ';']).to_owned())
}

fn modality(location: Option<&str>) -> Modality {
    match location.map(str::to_lowercase) {
        None => Modality::Unknown,
        Some(l) if l.trim().is_empty() => Modality::Unknown,
        Some(l)
            if l.starts_with("http")
                || ["online", "virtual", "zoom", "google meet", "remote"]
                    .iter()
                    .any(|k| l.contains(k)) =>
        {
            Modality::Online
        }
        Some(_) => Modality::InPerson,
    }
}

/// Parses a calendar into upcoming events.
pub fn parse(
    text: &str,
    source_name: &str,
    kind: SourceKind,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let lines = unfold(text);
    if !lines
        .first()
        .is_some_and(|l| l.eq_ignore_ascii_case("BEGIN:VCALENDAR"))
    {
        return Err(SourceError::Parse("not an iCalendar document".into()));
    }
    let props: Vec<Prop> = lines.iter().filter_map(|l| parse_line(l)).collect();
    let zones = parse_zones(&props);
    let mut out = Vec::new();
    let mut i = 0;
    while i < props.len() && out.len() < MAX_EVENTS {
        if props[i].name == "BEGIN" && props[i].value == "VEVENT" {
            let end = props[i..]
                .iter()
                .position(|p| p.name == "END" && p.value == "VEVENT")
                .map_or(props.len(), |e| i + e);
            if let Some(o) = event(&props[i + 1..end], &zones, source_name, kind, now) {
                out.push(o);
            }
            i = end;
        }
        i += 1;
    }
    Ok(out)
}

fn parse_zones(props: &[Prop]) -> std::collections::HashMap<String, TimeZone> {
    let mut zones = std::collections::HashMap::new();
    let mut tzid: Option<String> = None;
    let mut current: Option<Transition> = None;
    for p in props {
        match (p.name.as_str(), p.value.as_str()) {
            ("BEGIN", "VTIMEZONE") => tzid = None,
            ("TZID", id) => tzid = Some(id.to_owned()),
            ("BEGIN", "STANDARD" | "DAYLIGHT") => {
                current = Some(Transition {
                    offset_secs: 0,
                    start: NaiveDateTime::MIN,
                    rule: None,
                });
            }
            ("TZOFFSETTO", v) => {
                if let (Some(t), Some(o)) = (current.as_mut(), parse_offset(v)) {
                    t.offset_secs = o;
                }
            }
            ("DTSTART", v) => {
                if let (Some(t), Some(d)) = (current.as_mut(), parse_local(v)) {
                    t.start = d;
                }
            }
            ("RRULE", v) => {
                if let Some(t) = current.as_mut() {
                    t.rule = parse_rule(v);
                }
            }
            ("END", "STANDARD" | "DAYLIGHT") => {
                if let (Some(id), Some(t)) = (tzid.as_ref(), current.take()) {
                    zones
                        .entry(id.clone())
                        .or_insert_with(TimeZone::default)
                        .transitions
                        .push(t);
                }
            }
            _ => {}
        }
    }
    zones
}

fn event(
    props: &[Prop],
    zones: &std::collections::HashMap<String, TimeZone>,
    source_name: &str,
    kind: SourceKind,
    now: DateTime<Utc>,
) -> Option<Opportunity> {
    let get = |name: &str| props.iter().find(|p| p.name == name);
    if get("STATUS").is_some_and(|s| s.value.eq_ignore_ascii_case("CANCELLED")) {
        return None;
    }
    let title = get("SUMMARY")?.value.trim().to_owned();
    let starts_at = to_utc(get("DTSTART")?, zones)?;
    let ends_at = get("DTEND").and_then(|p| to_utc(p, zones));
    // Skip events that already finished (keep those running today).
    if ends_at.unwrap_or(starts_at) < now - Duration::hours(12) {
        return None;
    }
    let description = get("DESCRIPTION").map(|p| p.value.clone());
    let link = get("URL")
        .map(|p| p.value.trim().to_owned())
        .or_else(|| description.as_deref().and_then(first_url))?;
    let canonical = CanonicalUrl::parse(&link).ok()?;
    let mut o = Opportunity::new(kind, source_name, &link, &canonical, &title, now);
    o.category = Category::Event;
    o.external_id = get("UID").map(|p| p.value.clone());
    o.summary = description;
    o.location = get("LOCATION")
        .map(|p| p.value.trim().to_owned())
        .filter(|l| !l.is_empty());
    o.modality = modality(o.location.as_deref());
    o.dates.starts_at = Some(starts_at);
    o.dates.ends_at = ends_at;
    if let Some(org) = get("ORGANIZER") {
        o.author = Author {
            name: org.param("CN").map(str::to_owned),
            ..Author::default()
        };
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-16T00:00:00Z")
            .unwrap()
            .to_utc()
    }

    const MEETUP: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:America/New_York\r\n\
BEGIN:DAYLIGHT\r\nTZOFFSETFROM:-0500\r\nTZOFFSETTO:-0400\r\nDTSTART:19700308T020000\r\n\
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r\nEND:DAYLIGHT\r\nBEGIN:STANDARD\r\nTZOFFSETFROM:-0400\r\n\
TZOFFSETTO:-0500\r\nDTSTART:19701101T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\r\n\
END:STANDARD\r\nEND:VTIMEZONE\r\n\
BEGIN:VEVENT\r\nUID:event_1@meetup.com\r\nDTSTART;TZID=America/New_York:20261204T140000\r\n\
DTEND;TZID=America/New_York:20261204T180000\r\nSUMMARY:Rust NYC: The UnConf\\, 2026\r\n\
DESCRIPTION:Talks\\nand workshops. More at https://example.com/\r\n info\r\n\
LOCATION:Jay Conference Center\\, New York\r\nURL:https://www.meetup.com/rust-nyc/events/1/\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:summer\r\nDTSTART;TZID=America/New_York:20270701T100000\r\n\
SUMMARY:Summer\r\nURL:https://www.meetup.com/rust-nyc/events/2/\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:old\r\nDTSTART:20230101T100000Z\r\nSUMMARY:Old\r\nURL:https://x.example/old\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:cancel\r\nDTSTART:20270101T100000Z\r\nSTATUS:CANCELLED\r\nSUMMARY:Cancelled\r\nURL:https://x.example/c\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn parses_meetup_with_dst_rules() {
        let items = parse(MEETUP, "rust-nyc", SourceKind::Meetup, now()).unwrap();
        assert_eq!(items.len(), 2, "old and cancelled skipped");
        let unconf = &items[0];
        assert_eq!(unconf.title, "Rust NYC: The UnConf, 2026");
        // December: EST (UTC-5).
        assert_eq!(
            unconf.dates.starts_at.unwrap().to_rfc3339(),
            "2026-12-04T19:00:00+00:00"
        );
        assert_eq!(
            unconf.location.as_deref(),
            Some("Jay Conference Center, New York")
        );
        assert_eq!(unconf.modality, Modality::InPerson);
        assert_eq!(
            unconf.summary.as_deref(),
            Some("Talks\nand workshops. More at https://example.com/info")
        );
        assert_eq!(unconf.external_id.as_deref(), Some("event_1@meetup.com"));
        // July: EDT (UTC-4).
        assert_eq!(
            items[1].dates.starts_at.unwrap().to_rfc3339(),
            "2027-07-01T14:00:00+00:00"
        );
    }

    #[test]
    fn luma_style_events_use_the_description_url() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20261201T020000Z\nDTEND:20261201T043000Z\n\
ORGANIZER;CN=\"Fission\":MAILTO:calendar-invite@lu.ma\nUID:evt-1@events.lu.ma\nSUMMARY:DWeb Social\n\
DESCRIPTION:Get up-to-date information at: https://luma.com/van-dweb-socia\n l\\n\\nHosted by Fission\n\
LOCATION:https://zoom.us/j/1\nEND:VEVENT\nEND:VCALENDAR\n";
        let items = parse(ics, "dweb", SourceKind::Luma, now()).unwrap();
        assert_eq!(items[0].original_url, "https://luma.com/van-dweb-social");
        assert_eq!(items[0].author.name.as_deref(), Some("Fission"));
        assert_eq!(items[0].modality, Modality::Online);
        assert_eq!(items[0].source, SourceKind::Luma);
    }

    #[test]
    fn all_day_and_known_zones() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:20261221\nSUMMARY:Holiday hack\n\
URL:https://x.example/a\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART;TZID=America/Lima:20261205T090000\n\
SUMMARY:DevFest Cusco\nURL:https://x.example/b\nEND:VEVENT\nEND:VCALENDAR\n";
        let items = parse(ics, "x", SourceKind::ICal, now()).unwrap();
        assert!(dates::is_all_day(items[0].dates.starts_at.unwrap()));
        assert_eq!(
            items[1].dates.starts_at.unwrap().to_rfc3339(),
            "2026-12-05T14:00:00+00:00"
        );
    }

    #[test]
    fn skips_events_without_link_or_dates_and_rejects_garbage() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:No date\nURL:https://x.example/1\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:20270101T000000Z\nSUMMARY:No link\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:2027-01-01\nSUMMARY:Bad date\nURL:https://x.example/2\nEND:VEVENT\n\
BEGIN:VEVENT\nDTSTART:20270101T000000Z\nSUMMARY:Unsafe\nURL:javascript:alert(1)\nEND:VEVENT\nEND:VCALENDAR";
        assert_eq!(parse(ics, "x", SourceKind::ICal, now()).unwrap().len(), 0);
        assert!(parse("<html>nope</html>", "x", SourceKind::ICal, now()).is_err());
        assert!(parse("", "x", SourceKind::ICal, now()).is_err());
    }

    #[test]
    fn nth_weekday_rules() {
        assert_eq!(
            nth_weekday(2026, 3, 2, Weekday::Sun),
            NaiveDate::from_ymd_opt(2026, 3, 8)
        );
        assert_eq!(
            nth_weekday(2026, 10, -1, Weekday::Sun),
            NaiveDate::from_ymd_opt(2026, 10, 25)
        );
    }

    #[test]
    fn webcal_urls_are_accepted() {
        let spec = SourceSpec {
            name: "l".into(),
            kind: SourceKind::Luma,
            url: Some("webcal://api.lu.ma/ics/get?entity=calendar&id=cal-1".into()),
            query: None,
            target: None,
            category: None,
            tags: vec![],
            max_items: 10,
            token: None,
            content: sentri_core::ContentKind::Opportunity,
            stance: None,
        };
        assert_eq!(Calendar::new(spec).unwrap().url.scheme(), "https");
    }
}
