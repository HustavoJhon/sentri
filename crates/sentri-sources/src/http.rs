//! HTTP client hardened for untrusted URLs.
//!
//! Protections:
//! - only `http`/`https`, no embedded credentials;
//! - every resolved address (including after redirects) must be public, which
//!   also defeats DNS rebinding because the vetted addresses are the ones used
//!   to connect;
//! - IP literals are checked before connecting;
//! - system proxies are ignored so the checks cannot be bypassed;
//! - bounded redirects, timeouts, body size and accepted content types.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::redirect;
use thiserror::Error;
use url::{Host, Url};

use crate::net::is_public;

/// Identifies SENTRI without revealing anything about the user.
pub const USER_AGENT: &str = concat!("sentri/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("URL not allowed: {0}")]
    Forbidden(String),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("server answered HTTP {0}")]
    Status(u16),
    #[error("response larger than {0} bytes")]
    TooLarge(u64),
    #[error("unsupported content type {0:?}")]
    ContentType(String),
}

/// Limits applied to every request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpLimits {
    pub timeout: Duration,
    pub max_body_bytes: u64,
    pub max_redirects: u8,
    /// Only for tests against local mock servers. Never enabled by config.
    pub allow_private_networks: bool,
}

impl Default for HttpLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            max_body_bytes: 3_000_000,
            max_redirects: 5,
            allow_private_networks: false,
        }
    }
}

/// A downloaded document.
#[derive(Debug, Clone)]
pub struct Fetched {
    /// URL after redirects.
    pub url: Url,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Fetched {
    /// Body decoded as UTF-8, replacing invalid sequences.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn is_html(&self) -> bool {
        self.content_type.contains("html")
    }
}

/// Content types SENTRI is willing to download.
const ACCEPTED_TYPES: &[&str] = &[
    "text/html",
    "application/xhtml+xml",
    "application/rss+xml",
    "application/atom+xml",
    "application/feed+json",
    "application/xml",
    "text/xml",
    "application/json",
    "application/ld+json",
    "text/calendar",
    "text/plain",
];

const DOCUMENT_ACCEPT: &str = "text/html,application/xhtml+xml,application/rss+xml,\
    application/atom+xml,application/xml;q=0.9,application/json;q=0.9,*/*;q=0.1";

/// Image types SENTRI decodes. SVG is excluded on purpose (it is a document
/// that can reference external resources).
const IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Shared, cheaply clonable HTTP client.
#[derive(Debug, Clone)]
pub struct SafeClient {
    /// Follows validated redirects; used for anonymous requests.
    client: reqwest::Client,
    /// Never follows redirects; used when credentials are attached so they
    /// cannot be forwarded to another origin.
    authenticated: reqwest::Client,
    limits: HttpLimits,
}

impl SafeClient {
    pub fn new(limits: HttpLimits) -> Result<Self, FetchError> {
        let allow_private = limits.allow_private_networks;
        let max_redirects = usize::from(limits.max_redirects);
        let policy = redirect::Policy::custom(move |attempt| {
            let downgrade = attempt.url().scheme() == "http"
                && attempt.previous().iter().any(|u| u.scheme() == "https");
            if attempt.previous().len() >= max_redirects {
                attempt.error("too many redirects")
            } else if downgrade {
                attempt.error("redirect from https to http refused")
            } else if let Err(reason) = check_url(attempt.url(), allow_private) {
                attempt.error(reason)
            } else {
                attempt.follow()
            }
        });
        let build = |policy| {
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(limits.timeout)
                .connect_timeout(limits.timeout.min(Duration::from_secs(10)))
                .redirect(policy)
                .no_proxy()
                .dns_resolver(Arc::new(PublicOnlyResolver { allow_private }))
                .build()
        };
        Ok(Self {
            client: build(policy)?,
            authenticated: build(redirect::Policy::none())?,
            limits,
        })
    }

    pub const fn limits(&self) -> HttpLimits {
        self.limits
    }

    /// Downloads `url` honoring every limit.
    pub async fn get(&self, url: &Url) -> Result<Fetched, FetchError> {
        self.get_with(url, &[]).await
    }

    /// Like [`get`](Self::get) with extra request headers (e.g. API auth).
    /// Redirects are not followed when headers are given, so credentials are
    /// only ever sent to the requested origin.
    pub async fn get_with(
        &self,
        url: &Url,
        headers: &[(&str, String)],
    ) -> Result<Fetched, FetchError> {
        self.fetch(
            url,
            headers,
            ACCEPTED_TYPES,
            DOCUMENT_ACCEPT,
            self.limits.max_body_bytes,
        )
        .await
    }

    /// Downloads an image (PNG, JPEG, WebP or GIF) of at most `max_bytes`.
    /// The bytes are untrusted: decode them with dimension limits.
    pub async fn get_image(&self, url: &Url, max_bytes: u64) -> Result<Fetched, FetchError> {
        self.fetch(
            url,
            &[],
            IMAGE_TYPES,
            "image/png,image/jpeg,image/webp,image/gif",
            max_bytes,
        )
        .await
    }

    async fn fetch(
        &self,
        url: &Url,
        headers: &[(&str, String)],
        accepted: &[&str],
        accept_header: &str,
        max: u64,
    ) -> Result<Fetched, FetchError> {
        check_url(url, self.limits.allow_private_networks).map_err(FetchError::Forbidden)?;
        let client = if headers.is_empty() {
            &self.client
        } else {
            &self.authenticated
        };
        let mut request = client.get(url.clone()).header(ACCEPT, accept_header);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let mut response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::Status(status.as_u16()));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_ascii_lowercase();
        let essence = content_type.split(';').next().unwrap_or_default().trim();
        if !accepted.contains(&essence) {
            return Err(FetchError::ContentType(essence.to_owned()));
        }
        if response.content_length().is_some_and(|len| len > max) {
            return Err(FetchError::TooLarge(max));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() as u64 + chunk.len() as u64 > max {
                return Err(FetchError::TooLarge(max));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Fetched {
            url: response.url().clone(),
            content_type,
            body,
        })
    }
}

/// Validates scheme, credentials and IP-literal hosts.
pub fn check_url(url: &Url, allow_private: bool) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("scheme {:?} is not allowed", url.scheme()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("embedded credentials are not allowed".into());
    }
    let ip = match url.host() {
        None => return Err("missing host".into()),
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            if !allow_private && (domain == "localhost" || domain.ends_with(".localhost")) {
                return Err("localhost is not allowed".into());
            }
            return Ok(());
        }
    };
    if allow_private || is_public(ip) {
        Ok(())
    } else {
        Err(format!("address {ip} is not public"))
    }
}

/// DNS resolver that drops non-public addresses.
#[derive(Debug)]
struct PublicOnlyResolver {
    allow_private: bool,
}

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_private = self.allow_private;
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let resolved: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|addr| allow_private || is_public(addr.ip()))
                .collect();
            if resolved.is_empty() {
                return Err(format!("{host} does not resolve to a public address").into());
            }
            let addrs: Addrs = Box::new(resolved.into_iter());
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(s: &str) -> Result<(), String> {
        check_url(&Url::parse(s).unwrap(), false)
    }

    #[test]
    fn rejects_unsafe_urls() {
        assert!(check("ftp://example.com/").is_err());
        assert!(check("http://user:pw@example.com/").is_err());
        assert!(check("http://127.0.0.1/").is_err());
        assert!(check("http://[::1]:8080/").is_err());
        assert!(check("http://169.254.169.254/latest/meta-data").is_err());
        assert!(check("http://localhost:3000/").is_err());
        assert!(check("http://0x7f000001/").is_err());
        assert!(check("https://example.com/").is_ok());
    }
}
