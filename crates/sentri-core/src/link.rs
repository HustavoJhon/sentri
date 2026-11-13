//! URL canonicalization used to detect duplicates across sources.

use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

/// Query parameters that only track clicks and never change the content.
const TRACKING_PARAMS: &[&str] = &[
    "fbclid",
    "gclid",
    "dclid",
    "msclkid",
    "mc_cid",
    "mc_eid",
    "igshid",
    "ref",
    "ref_src",
    "ref_url",
    "trk",
    "trackingid",
    "refid",
    "si",
    "s",
    "_hsenc",
    "_hsmi",
    "share",
];

/// Error produced when a link cannot be used as an opportunity URL.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LinkError {
    #[error("invalid URL: {0}")]
    Invalid(String),
    #[error("unsupported URL scheme {0:?}; only http and https are allowed")]
    UnsupportedScheme(String),
    #[error("URL has no host")]
    MissingHost,
    #[error("URLs with embedded credentials are not allowed")]
    EmbeddedCredentials,
}

/// A validated, canonical http(s) URL plus its stable fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalUrl {
    url: Url,
    fingerprint: String,
}

impl CanonicalUrl {
    /// Parses and canonicalizes `input`.
    ///
    /// Canonicalization lowercases the host, drops `www.`, the fragment, default
    /// ports, tracking parameters and trailing slashes, and sorts the query, so
    /// that the same page shared from different places yields one fingerprint.
    pub fn parse(input: &str) -> Result<Self, LinkError> {
        let mut url = Url::parse(input.trim()).map_err(|e| LinkError::Invalid(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(LinkError::UnsupportedScheme(url.scheme().to_owned()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(LinkError::EmbeddedCredentials);
        }
        let host = url
            .host_str()
            .filter(|h| !h.is_empty())
            .ok_or(LinkError::MissingHost)?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        url.set_host(Some(&host))
            .map_err(|e| LinkError::Invalid(e.to_string()))?;
        url.set_fragment(None);
        if url.port() == url.port_or_known_default() && url.port().is_some() {
            let _ = url.set_port(None);
        }
        // Scheme is not part of identity: http and https point to the same page.
        let _ = url.set_scheme("https");

        let mut pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !is_tracking_param(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        pairs.sort();
        if pairs.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(pairs);
        }

        let path = url.path().trim_end_matches('/').to_owned();
        url.set_path(if path.is_empty() { "/" } else { &path });

        let fingerprint = hex(&Sha256::digest(url.as_str().as_bytes()));
        Ok(Self { url, fingerprint })
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Hex SHA-256 of the canonical form; stable across releases.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn host(&self) -> &str {
        self.url.host_str().unwrap_or_default()
    }
}

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_") || TRACKING_PARAMS.contains(&key.as_str())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(s: &str) -> String {
        CanonicalUrl::parse(s).unwrap().as_str().to_owned()
    }

    #[test]
    fn strips_noise() {
        assert_eq!(
            canon("HTTP://WWW.Example.com:80/a/b/?utm_source=x&b=2&a=1#frag"),
            "https://example.com/a/b?a=1&b=2"
        );
    }

    #[test]
    fn equivalent_urls_share_fingerprint() {
        let a = CanonicalUrl::parse("https://example.com/event?id=7").unwrap();
        let b = CanonicalUrl::parse("http://www.example.com/event/?fbclid=abc&id=7#x").unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint().len(), 64);
    }

    #[test]
    fn keeps_meaningful_query() {
        let a = CanonicalUrl::parse("https://example.com/?id=1").unwrap();
        let b = CanonicalUrl::parse("https://example.com/?id=2").unwrap();
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn rejects_unsafe_input() {
        assert_eq!(
            CanonicalUrl::parse("file:///etc/passwd"),
            Err(LinkError::UnsupportedScheme("file".into()))
        );
        assert_eq!(
            CanonicalUrl::parse("javascript:alert(1)"),
            Err(LinkError::UnsupportedScheme("javascript".into()))
        );
        assert_eq!(
            CanonicalUrl::parse("https://user:pw@example.com/"),
            Err(LinkError::EmbeddedCredentials)
        );
        assert!(matches!(
            CanonicalUrl::parse("not a url"),
            Err(LinkError::Invalid(_))
        ));
    }
}
