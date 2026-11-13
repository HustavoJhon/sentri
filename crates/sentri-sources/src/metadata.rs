//! Metadata extraction from HTML pages (Open Graph, standard `<meta>` tags
//! and schema.org JSON-LD). Only reads what the page declares; nothing is
//! guessed here.

use chrono::{DateTime, NaiveDate, Utc};
use scraper::{Html, Selector};
use sentri_core::{Author, Modality};
use serde_json::Value;
use url::Url;

/// Maximum number of JSON-LD blocks inspected per page.
const MAX_JSON_LD_BLOCKS: usize = 10;

/// schema.org type recognized in JSON-LD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredKind {
    Event,
    JobPosting,
    Course,
}

/// Facts declared by schema.org JSON-LD.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Structured {
    pub kind: Option<StructuredKind>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub country: Option<String>,
    pub modality: Option<Modality>,
    /// `Some(true)` when an offer has price 0.
    pub free: Option<bool>,
    pub price: Option<String>,
    pub organization: Option<String>,
}

/// Everything extracted from a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<Url>,
    pub site_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub keywords: Vec<String>,
    /// Author as declared by the page (meta tags or JSON-LD).
    pub author: Author,
    pub structured: Structured,
}

/// Parses `html` served from `base`.
pub fn extract(html: &str, base: &Url) -> PageMetadata {
    let doc = Html::parse_document(html);
    let meta = |names: &[&str]| -> Option<String> {
        names.iter().find_map(|name| {
            let selector = Selector::parse(&format!(
                r#"meta[property="{name}"], meta[name="{name}"], meta[itemprop="{name}"]"#
            ))
            .ok()?;
            doc.select(&selector)
                .filter_map(|e| e.value().attr("content"))
                .map(str::trim)
                .find(|c| !c.is_empty())
                .map(str::to_owned)
        })
    };
    let text_of = |css: &str| -> Option<String> {
        let selector = Selector::parse(css).ok()?;
        doc.select(&selector)
            .map(|e| e.text().collect::<String>().trim().to_owned())
            .find(|t| !t.is_empty())
    };

    let structured = structured_data(&doc);
    PageMetadata {
        title: meta(&["og:title", "twitter:title"])
            .or_else(|| structured.name.clone())
            .or_else(|| text_of("title"))
            .or_else(|| text_of("h1")),
        description: meta(&["og:description", "description", "twitter:description"])
            .or_else(|| structured.description.clone()),
        image: meta(&["og:image", "og:image:url", "twitter:image"])
            .and_then(|src| base.join(&src).ok())
            .filter(|u| u.scheme() == "https"),
        site_name: meta(&["og:site_name", "application-name"]),
        published_at: meta(&["article:published_time", "datePublished", "date"])
            .and_then(|d| parse_date(&d)),
        keywords: meta(&["keywords"])
            .map(|k| {
                k.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .take(10)
                    .collect()
            })
            .unwrap_or_default(),
        author: author(
            &doc,
            meta(&["author", "article:author"]),
            meta(&["twitter:creator"]),
        ),
        structured,
    }
}

/// Builds the author from what the page declares. `article:author` is often a
/// profile URL rather than a name, so URLs go to `profile_url`.
fn author(doc: &Html, declared: Option<String>, twitter: Option<String>) -> Author {
    let mut author = json_ld_author(doc).unwrap_or_default();
    if let Some(value) = declared {
        if value.starts_with("https://") || value.starts_with("http://") {
            author.profile_url.get_or_insert(value);
        } else {
            author.name.get_or_insert(value);
        }
    }
    if let Some(handle) = twitter.filter(|h| h.starts_with('@')) {
        author.handle.get_or_insert(handle);
    }
    author
}

fn json_ld_author(doc: &Html) -> Option<Author> {
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    doc.select(&selector)
        .take(MAX_JSON_LD_BLOCKS)
        .filter_map(|e| serde_json::from_str::<Value>(&e.text().collect::<String>()).ok())
        .flat_map(flatten)
        .find_map(|node| {
            let author = first(node.get("author")?);
            let name = author
                .as_str()
                .or_else(|| str_at(author, "name"))
                .map(str::to_owned);
            let url = str_at(author, "url").map(str::to_owned);
            let image = author
                .get("image")
                .and_then(|i| i.as_str().or_else(|| str_at(i, "url")))
                .map(str::to_owned);
            (name.is_some() || url.is_some()).then(|| Author {
                name,
                handle: str_at(author, "alternateName").map(str::to_owned),
                profile_url: url,
                avatar_url: image,
            })
        })
}

/// Visible text of an HTML fragment (tags removed, entities decoded).
/// Scripts and styles are dropped; whitespace is normalized later by
/// `sentri_core::text::clean`.
pub fn html_to_text(html: &str) -> String {
    if !html.contains('<') && !html.contains('&') {
        return html.to_owned();
    }
    let fragment = Html::parse_fragment(html);
    let mut out = String::with_capacity(html.len());
    for node in fragment.root_element().descendants() {
        let Some(text) = node.value().as_text() else {
            continue;
        };
        let hidden = node.ancestors().any(|a| {
            a.value()
                .as_element()
                .is_some_and(|e| matches!(e.name(), "script" | "style"))
        });
        if !hidden {
            out.push_str(text);
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Accepts RFC 3339 timestamps and plain `YYYY-MM-DD` dates.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.to_utc())
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(0, 0, 0)
                .map(|d| d.and_utc())
        })
}

fn structured_data(doc: &Html) -> Structured {
    let Ok(selector) = Selector::parse(r#"script[type="application/ld+json"]"#) else {
        return Structured::default();
    };
    doc.select(&selector)
        .take(MAX_JSON_LD_BLOCKS)
        .filter_map(|e| serde_json::from_str::<Value>(&e.text().collect::<String>()).ok())
        .flat_map(flatten)
        .find_map(|node| from_node(&node))
        .unwrap_or_default()
}

/// JSON-LD may be an object, an array, or an object with `@graph`.
fn flatten(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.into_iter().flat_map(flatten).collect(),
        Value::Object(mut map) => match map.remove("@graph") {
            Some(graph) => flatten(graph),
            None => vec![Value::Object(map)],
        },
        _ => Vec::new(),
    }
}

fn types(node: &Value) -> Vec<&str> {
    match node.get("@type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn str_at<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn from_node(node: &Value) -> Option<Structured> {
    let kind = types(node).into_iter().find_map(|t| match t {
        t if t.ends_with("Event") => Some(StructuredKind::Event),
        "JobPosting" => Some(StructuredKind::JobPosting),
        "Course" | "CourseInstance" => Some(StructuredKind::Course),
        _ => None,
    })?;
    let mut s = Structured {
        kind: Some(kind),
        name: str_at(node, "name")
            .or_else(|| str_at(node, "title"))
            .map(str::to_owned),
        description: str_at(node, "description").map(str::to_owned),
        starts_at: str_at(node, "startDate").and_then(parse_date),
        ends_at: str_at(node, "endDate").and_then(parse_date),
        deadline: str_at(node, "validThrough").and_then(parse_date),
        organization: node
            .get("hiringOrganization")
            .or_else(|| node.get("organizer"))
            .or_else(|| node.get("provider"))
            .and_then(|o| str_at(o, "name"))
            .map(str::to_owned),
        ..Structured::default()
    };
    s.modality = modality(node);
    let place = node.get("location").or_else(|| node.get("jobLocation"));
    if let Some(place) = place.map(first) {
        s.location = place_name(place);
        s.country = place
            .get("address")
            .and_then(|a| a.get("addressCountry"))
            .and_then(|c| c.as_str().or_else(|| str_at(c, "name")))
            .map(str::to_owned);
    }
    if let Some(offer) = node.get("offers").map(first) {
        let price = offer.get("price").and_then(|p| match p {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });
        s.free = price
            .as_deref()
            .and_then(|p| p.parse::<f64>().ok())
            .map(|p| p == 0.0);
        let currency = str_at(offer, "priceCurrency").unwrap_or_default();
        s.price = price
            .filter(|_| s.free == Some(false))
            .map(|p| format!("{currency} {p}").trim().to_owned());
    }
    Some(s)
}

fn first(value: &Value) -> &Value {
    match value {
        Value::Array(items) => items.first().unwrap_or(value),
        other => other,
    }
}

fn place_name(place: &Value) -> Option<String> {
    if let Some(name) = place.as_str() {
        return Some(name.to_owned());
    }
    let address = place.get("address");
    let city = address.and_then(|a| a.as_str().or_else(|| str_at(a, "addressLocality")));
    let name = str_at(place, "name");
    match (name, city) {
        (Some(n), Some(c)) if !n.contains(c) => Some(format!("{n}, {c}")),
        (Some(n), _) => Some(n.to_owned()),
        (None, Some(c)) => Some(c.to_owned()),
        (None, None) => None,
    }
}

fn modality(node: &Value) -> Option<Modality> {
    if str_at(node, "jobLocationType") == Some("TELECOMMUTE") {
        return Some(Modality::Online);
    }
    let mode = str_at(node, "eventAttendanceMode")?;
    if mode.contains("Online") {
        Some(Modality::Online)
    } else if mode.contains("Offline") {
        Some(Modality::InPerson)
    } else if mode.contains("Mixed") {
        Some(Modality::Hybrid)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://events.example/e/1").unwrap()
    }

    #[test]
    fn reads_open_graph_with_fallbacks() {
        let html = r#"<html><head>
            <title>Fallback</title>
            <meta property="og:title" content=" DevFest Cusco ">
            <meta name="description" content="Annual GDG event">
            <meta property="og:image" content="/img/cover.png">
            <meta property="og:site_name" content="GDG">
            <meta name="keywords" content="gdg, android, , ai">
            <meta property="article:published_time" content="2026-10-01T10:00:00-05:00">
        </head></html>"#;
        let m = extract(html, &base());
        assert_eq!(m.title.as_deref(), Some("DevFest Cusco"));
        assert_eq!(m.description.as_deref(), Some("Annual GDG event"));
        assert_eq!(
            m.image.unwrap().as_str(),
            "https://events.example/img/cover.png"
        );
        assert_eq!(m.site_name.as_deref(), Some("GDG"));
        assert_eq!(m.keywords, ["gdg", "android", "ai"]);
        assert_eq!(
            m.published_at.unwrap().to_rfc3339(),
            "2026-10-01T15:00:00+00:00"
        );
    }

    #[test]
    fn converts_html_to_text() {
        assert_eq!(
            html_to_text("<p>Hola <b>mundo</b> &amp; más</p><script>evil()</script>"),
            "Hola mundo & más"
        );
        assert_eq!(html_to_text("plain"), "plain");
    }

    #[test]
    fn reads_declared_authors_only() {
        let html = r#"<meta name="author" content="Ana Quispe">
            <meta name="twitter:creator" content="@anaq">
            <script type="application/ld+json">{"@type":"Article","author":{"@type":"Person",
              "name":"Ana Q.","url":"https://blog.example/ana","image":"https://blog.example/ana.png"}}</script>"#;
        let a = extract(html, &base()).author;
        assert_eq!(a.name.as_deref(), Some("Ana Q."), "JSON-LD wins");
        assert_eq!(a.handle.as_deref(), Some("@anaq"));
        assert_eq!(a.profile_url.as_deref(), Some("https://blog.example/ana"));
        assert_eq!(
            a.avatar_url.as_deref(),
            Some("https://blog.example/ana.png")
        );
        assert!(extract("<title>x</title>", &base()).author.is_empty());
    }

    #[test]
    fn falls_back_to_title_tag() {
        let m = extract("<title> Only title </title>", &base());
        assert_eq!(m.title.as_deref(), Some("Only title"));
        assert_eq!(m.image, None);
    }

    #[test]
    fn reads_event_json_ld() {
        let html = r#"<script type="application/ld+json">{"@context":"https://schema.org",
          "@graph":[{"@type":"Organization","name":"X"},{"@type":"Event","name":"AI Hack",
          "startDate":"2026-12-05T09:00:00-05:00","endDate":"2026-12-06",
          "eventAttendanceMode":"https://schema.org/OfflineEventAttendanceMode",
          "location":{"@type":"Place","name":"Universidad","address":{"addressLocality":"Cusco","addressCountry":"PE"}},
          "offers":{"price":"0","priceCurrency":"PEN"}}]}</script>"#;
        let s = extract(html, &base()).structured;
        assert_eq!(s.kind, Some(StructuredKind::Event));
        assert_eq!(s.name.as_deref(), Some("AI Hack"));
        assert_eq!(s.modality, Some(Modality::InPerson));
        assert_eq!(s.location.as_deref(), Some("Universidad, Cusco"));
        assert_eq!(s.country.as_deref(), Some("PE"));
        assert_eq!(s.free, Some(true));
        assert_eq!(s.price, None);
        assert!(s.starts_at.is_some() && s.ends_at.is_some());
    }

    #[test]
    fn reads_job_posting_json_ld() {
        let html = r#"<script type="application/ld+json">{"@type":"JobPosting","title":"Backend Intern",
          "datePosted":"2026-11-01","validThrough":"2026-12-01T00:00:00Z","jobLocationType":"TELECOMMUTE",
          "hiringOrganization":{"name":"ACME"}}</script>"#;
        let s = extract(html, &base()).structured;
        assert_eq!(s.kind, Some(StructuredKind::JobPosting));
        assert_eq!(s.name.as_deref(), Some("Backend Intern"));
        assert_eq!(s.modality, Some(Modality::Online));
        assert_eq!(s.organization.as_deref(), Some("ACME"));
        assert!(s.deadline.is_some());
    }

    #[test]
    fn ignores_malformed_json_ld() {
        let m = extract(
            r#"<script type="application/ld+json">{nope</script>"#,
            &base(),
        );
        assert_eq!(m.structured, Structured::default());
    }
}
