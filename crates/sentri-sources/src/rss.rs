//! RSS 2.0 / Atom / JSON Feed connector (via `feed-rs`).

use chrono::{DateTime, Utc};
use sentri_core::{CanonicalUrl, Opportunity, SourceKind};
use url::Url;

use crate::SafeClient;
use crate::connector::{SourceError, SourceSpec};
use crate::metadata::html_to_text;

/// A configured feed.
#[derive(Debug, Clone)]
pub struct Feed {
    pub(crate) spec: SourceSpec,
    url: Url,
}

impl Feed {
    pub fn new(spec: SourceSpec) -> Result<Self, SourceError> {
        let url = spec
            .url
            .as_deref()
            .ok_or_else(|| SourceError::Config(format!("{}: `url` is required", spec.name)))?;
        let url = Url::parse(url).map_err(|e| SourceError::Config(e.to_string()))?;
        Ok(Self { spec, url })
    }

    pub async fn fetch(
        &self,
        http: &SafeClient,
        now: DateTime<Utc>,
    ) -> Result<Vec<Opportunity>, SourceError> {
        let page = http.get(&self.url).await?;
        let mut items = parse(&page.body, &self.spec.name, now)?;
        // Keep the platform badge (Mastodon, YouTube…) for feed-based sources.
        for item in &mut items {
            item.source = self.spec.kind;
        }
        Ok(items)
    }
}

/// Parses a feed document into opportunities. Entries without a usable
/// http(s) link are skipped.
pub fn parse(
    body: &[u8],
    source_name: &str,
    now: DateTime<Utc>,
) -> Result<Vec<Opportunity>, SourceError> {
    let feed = match feed_rs::parser::parse(body) {
        Ok(feed) => feed,
        // Some WordPress feeds (e.g. blogs.nvidia.com) emit empty
        // `<media:description></media:description>`, which feed-rs rejects.
        // Retry once without empty media elements; nothing else changes.
        Err(first) => {
            let cleaned = strip_empty_media(&String::from_utf8_lossy(body));
            feed_rs::parser::parse(cleaned.as_bytes())
                .map_err(|_| SourceError::Parse(first.to_string()))?
        }
    };
    Ok(feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let link = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
                .or_else(|| entry.links.first())?
                .href
                .clone();
            let canonical = CanonicalUrl::parse(&link).ok()?;
            let title = entry
                .title
                .as_ref()
                .map_or(link.as_str(), |t| t.content.as_str());
            let mut o = Opportunity::new(
                SourceKind::Rss,
                source_name,
                &link,
                &canonical,
                &html_to_text(title),
                now,
            );
            o.external_id = Some(entry.id.clone()).filter(|id| !id.is_empty());
            o.summary = entry
                .summary
                .as_ref()
                .map(|t| t.content.clone())
                .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
                .map(|html| html_to_text(&html));
            o.dates.published_at = entry.published.or(entry.updated);
            o.tags = entry.categories.iter().map(|c| c.term.clone()).collect();
            if let Some(person) = entry.authors.first() {
                o.author = sentri_core::Author {
                    name: person.name.clone().filter(|n| !n.trim().is_empty()),
                    handle: None,
                    profile_url: person.uri.clone(),
                    avatar_url: None,
                };
            }
            o.image_url = entry
                .media
                .iter()
                .flat_map(|m| m.thumbnails.iter())
                .map(|t| t.image.uri.clone())
                .find(|u| u.starts_with("https://"));
            Some(o)
        })
        .collect())
}

/// Removes empty `<media:title>`, `<media:description>` and `<media:text>`
/// elements (`<media:x ...></media:x>` or `<media:x ... />`).
fn strip_empty_media(xml: &str) -> String {
    let mut out = xml.to_owned();
    for name in ["media:description", "media:title", "media:text"] {
        let open = format!("<{name}");
        let close = format!("</{name}>");
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(start) = rest.find(&open) {
            let Some(tag_end) = rest[start..].find('>').map(|i| start + i) else {
                break;
            };
            let self_closing = rest[..tag_end].ends_with('/');
            let after = &rest[tag_end + 1..];
            let empty_pair = after.trim_start().starts_with(&close);
            result.push_str(&rest[..start]);
            if self_closing {
                rest = after;
            } else if empty_pair {
                let close_at = after.find(&close).unwrap_or(0);
                rest = &after[close_at + close.len()..];
            } else {
                result.push_str(&rest[start..=tag_end]);
                rest = after;
            }
        }
        result.push_str(rest);
        out = result;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Becas</title>
<item><title>Beca &lt;b&gt;completa&lt;/b&gt; en Japón</title>
  <link>https://becas.example/japon?utm_source=rss</link>
  <guid>b-1</guid>
  <description>&lt;p&gt;Fully funded, flights included&lt;/p&gt;</description>
  <category>Asia</category>
  <pubDate>Wed, 11 Nov 2026 10:00:00 GMT</pubDate></item>
<item><title>Bad link</title><link>javascript:alert(1)</link></item>
</channel></rss>"#;

    const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"><title>Dev</title><id>x</id><updated>2026-11-01T00:00:00Z</updated>
<entry><title>Rust workshop</title><id>tag:1</id><updated>2026-11-02T00:00:00Z</updated>
<author><name>Ferris</name><uri>https://dev.example/ferris</uri></author>
<link rel="alternate" href="https://dev.example/rust"/><summary>Online workshop</summary></entry>
</feed>"#;

    #[test]
    fn parses_rss_items_and_skips_unsafe_links() {
        let items = parse(RSS.as_bytes(), "becas", Utc::now()).unwrap();
        assert_eq!(items.len(), 1);
        let o = &items[0];
        assert_eq!(o.title, "Beca completa en Japón");
        assert_eq!(o.canonical_url, "https://becas.example/japon");
        assert_eq!(o.external_id.as_deref(), Some("b-1"));
        assert_eq!(o.summary.as_deref(), Some("Fully funded, flights included"));
        assert_eq!(o.tags, ["Asia"]);
        assert!(o.dates.published_at.is_some());
    }

    #[test]
    fn parses_atom_entries() {
        let items = parse(ATOM.as_bytes(), "dev", Utc::now()).unwrap();
        assert_eq!(items[0].original_url, "https://dev.example/rust");
        assert_eq!(items[0].summary.as_deref(), Some("Online workshop"));
        assert_eq!(items[0].author.name.as_deref(), Some("Ferris"));
        assert_eq!(
            items[0].author.profile_url.as_deref(),
            Some("https://dev.example/ferris")
        );
    }

    #[test]
    fn tolerates_empty_media_elements() {
        let rss = r#"<?xml version="1.0"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
<channel><title>NVIDIA</title><item><title>AI Day Seoul</title><link>https://blogs.nvidia.com/blog/a/</link>
<media:content url="https://blogs.nvidia.com/a.jpg" type="image/jpeg"><media:description type="html"></media:description>
<media:title>Kept</media:title></media:content></item></channel></rss>"#;
        let items = parse(rss.as_bytes(), "nvidia", Utc::now()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "AI Day Seoul");
        assert!(strip_empty_media("<media:title>Kept</media:title>").contains("Kept"));
        assert_eq!(
            strip_empty_media("<a/><media:text type=\"x\"/><b/>"),
            "<a/><b/>"
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"<html>not a feed</html>", "x", Utc::now()).is_err());
    }
}
