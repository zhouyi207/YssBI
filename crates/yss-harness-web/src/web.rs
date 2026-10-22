//! Public web search and page reading behind the Harness `web_*` tools.

use std::error::Error as _;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use serde::Deserialize;
use url::{Host, Url};
use yss_harness_contract::{
    FetchedPage, WebAccessFailure, WebAccessFuture, WebAccessPort, WebSearchHit,
};

const MAX_PAGE_BYTES: usize = 3 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const MAX_SNIPPET_CHARS: usize = 600;
const TEXT_WIDTH: usize = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchBackend {
    Brave {
        api_key: String,
    },
    Tavily {
        api_key: String,
    },
    /// A SearXNG instance the user runs or trusts; it may be on the local network.
    Searxng {
        base_url: Url,
    },
}

pub struct WebAccess {
    backend: RwLock<Option<SearchBackend>>,
    search_client: reqwest::Client,
    fetch_client: reqwest::Client,
}

impl WebAccess {
    pub fn new(backend: Option<SearchBackend>) -> Result<Self, reqwest::Error> {
        let user_agent = concat!("YssBI/", env!("CARGO_PKG_VERSION"));
        let search_client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        // Page addresses come from the model and from untrusted pages, so every hop is
        // checked before connecting: the literal host here, resolved addresses in the resolver.
        let fetch_client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .dns_resolver(Arc::new(PublicResolver))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= MAX_REDIRECTS {
                    attempt.error("too many redirects")
                } else if !host_allowed(attempt.url()) {
                    attempt.error(BlockedAddress)
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        Ok(Self {
            backend: RwLock::new(backend),
            search_client,
            fetch_client,
        })
    }

    /// Applies to searches that start after this call.
    pub fn set_backend(&self, backend: Option<SearchBackend>) {
        *self.backend.write().unwrap_or_else(|e| e.into_inner()) = backend;
    }

    async fn search_hits(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WebSearchHit>, WebAccessFailure> {
        let backend = self
            .backend
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or(WebAccessFailure::NotConfigured)?;
        let request = match &backend {
            SearchBackend::Brave { api_key } => self
                .search_client
                .get(
                    Url::parse_with_params(
                        "https://api.search.brave.com/res/v1/web/search",
                        [("q", query), ("count", &limit.to_string())],
                    )
                    .map_err(|_| WebAccessFailure::Unavailable)?,
                )
                .header("X-Subscription-Token", api_key)
                .header("Accept", "application/json"),
            SearchBackend::Tavily { api_key } => self
                .search_client
                .post("https://api.tavily.com/search")
                .bearer_auth(api_key)
                .json(&serde_json::json!({"query": query, "max_results": limit})),
            SearchBackend::Searxng { base_url } => {
                let mut endpoint = base_url
                    .join("search")
                    .map_err(|_| WebAccessFailure::NotConfigured)?;
                endpoint
                    .query_pairs_mut()
                    .append_pair("q", query)
                    .append_pair("format", "json");
                self.search_client.get(endpoint)
            }
        };
        let response = request.send().await.map_err(request_failure)?;
        let body = checked(response)?.bytes().await.map_err(request_failure)?;
        let mut hits = parse_hits(&backend, &body)?;
        hits.truncate(limit);
        Ok(hits)
    }

    async fn fetch_page(&self, address: &str) -> Result<FetchedPage, WebAccessFailure> {
        let url = Url::parse(address).map_err(|_| WebAccessFailure::AddressNotAllowed)?;
        if !matches!(url.scheme(), "http" | "https") || !host_allowed(&url) {
            return Err(WebAccessFailure::AddressNotAllowed);
        }
        let mut response = checked(
            self.fetch_client
                .get(url)
                .send()
                .await
                .map_err(request_failure)?,
        )?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("text/html")
            .to_ascii_lowercase();
        let kind = ContentKind::of(&content_type).ok_or(WebAccessFailure::UnsupportedContent)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PAGE_BYTES as u64)
        {
            return Err(WebAccessFailure::TooLarge);
        }
        let final_url = response.url().to_string();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(request_failure)? {
            if body.len() + chunk.len() > MAX_PAGE_BYTES {
                return Err(WebAccessFailure::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let decoded = decode(&body, &content_type);
        Ok(match kind {
            ContentKind::Html => FetchedPage {
                url: final_url,
                title: html_title(&decoded),
                text: html_to_text(&decoded)?,
            },
            ContentKind::Text => FetchedPage {
                url: final_url,
                title: None,
                text: decoded,
            },
        })
    }
}

impl WebAccessPort for WebAccess {
    fn search(&self, query: &str, limit: usize) -> WebAccessFuture<'_, Vec<WebSearchHit>> {
        let query = query.to_owned();
        Box::pin(async move { self.search_hits(&query, limit).await })
    }

    fn fetch(&self, url: &str) -> WebAccessFuture<'_, FetchedPage> {
        let url = url.to_owned();
        Box::pin(async move { self.fetch_page(&url).await })
    }
}

enum ContentKind {
    Html,
    Text,
}

impl ContentKind {
    fn of(content_type: &str) -> Option<Self> {
        let essence = content_type.split(';').next().unwrap_or_default().trim();
        match essence {
            "text/html" | "application/xhtml+xml" => Some(Self::Html),
            "application/json" | "application/xml" | "text/xml" => Some(Self::Text),
            essence if essence.starts_with("text/") => Some(Self::Text),
            _ => None,
        }
    }
}

fn checked(response: reqwest::Response) -> Result<reqwest::Response, WebAccessFailure> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else if status.is_client_error() {
        Err(WebAccessFailure::Rejected)
    } else {
        Err(WebAccessFailure::Unavailable)
    }
}

fn request_failure(error: reqwest::Error) -> WebAccessFailure {
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.is::<BlockedAddress>() {
            return WebAccessFailure::AddressNotAllowed;
        }
        source = cause.source();
    }
    if error.is_redirect() {
        WebAccessFailure::Rejected
    } else {
        WebAccessFailure::Unavailable
    }
}

#[derive(Debug)]
struct BlockedAddress;

impl std::fmt::Display for BlockedAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("address is not public")
    }
}

impl std::error::Error for BlockedAddress {}

struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|address| is_public(address.ip()))
                .collect();
            if addresses.is_empty() {
                return Err(Box::new(BlockedAddress) as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

fn host_allowed(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(address)) => is_public(IpAddr::V4(address)),
        Some(Host::Ipv6(address)) => is_public(IpAddr::V6(address)),
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            !(domain == "localhost"
                || domain.ends_with(".localhost")
                || domain.ends_with(".local")
                || domain.ends_with(".internal"))
        }
        None => false,
    }
}

fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [first, second, ..] = address.octets();
            !(address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_documentation()
                || address.is_unspecified()
                || first == 0
                || first >= 224
                // Shared address space (RFC 6598) and benchmarking (RFC 2544).
                || (first == 100 && second & 0xc0 == 64)
                || (first == 198 && second & 0xfe == 18))
        }
        IpAddr::V6(address) => {
            if let Some(mapped) = address.to_ipv4_mapped() {
                return is_public(IpAddr::V4(mapped));
            }
            let first = address.segments()[0];
            !(address.is_loopback()
                || address.is_unspecified()
                || address.is_multicast()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80)
        }
    }
}

fn decode(body: &[u8], content_type: &str) -> String {
    let label = content_type
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("charset="))
        .next()
        .map(|label| label.trim_matches('"'));
    match label.and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes())) {
        Some(encoding) => encoding.decode(body).0.into_owned(),
        None => String::from_utf8_lossy(body).into_owned(),
    }
}

fn html_to_text(html: &str) -> Result<String, WebAccessFailure> {
    html2text::config::plain()
        .string_from_read(html.as_bytes(), TEXT_WIDTH)
        .map_err(|_| WebAccessFailure::UnsupportedContent)
}

/// Inline markup such as titles and snippets, without emphasis marks or link lists.
fn inline_text(html: &str) -> String {
    html2text::config::plain_no_decorate()
        .string_from_read(format!("<p>{html}</p>").as_bytes(), usize::MAX / 2)
        .unwrap_or_else(|_| html.to_owned())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let title = inline_text(html.get(start..end)?);
    (!title.is_empty()).then_some(title)
}

fn snippet(text: &str) -> String {
    inline_text(text).chars().take(MAX_SNIPPET_CHARS).collect()
}

#[derive(Deserialize)]
struct BraveResponse {
    #[serde(default)]
    web: Option<BraveResults>,
}

#[derive(Deserialize)]
struct BraveResults {
    #[serde(default)]
    results: Vec<BraveHit>,
}

#[derive(Deserialize)]
struct BraveHit {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
}

/// Tavily and SearXNG share this shape.
#[derive(Deserialize)]
struct ContentResponse {
    #[serde(default)]
    results: Vec<ContentHit>,
}

#[derive(Deserialize)]
struct ContentHit {
    #[serde(default)]
    title: String,
    url: String,
    #[serde(default)]
    content: String,
}

fn parse_hits(backend: &SearchBackend, body: &[u8]) -> Result<Vec<WebSearchHit>, WebAccessFailure> {
    let hits = match backend {
        SearchBackend::Brave { .. } => serde_json::from_slice::<BraveResponse>(body)
            .map_err(|_| WebAccessFailure::Unavailable)?
            .web
            .map(|web| web.results)
            .unwrap_or_default()
            .into_iter()
            .map(|hit| (hit.title, hit.url, hit.description))
            .collect::<Vec<_>>(),
        SearchBackend::Tavily { .. } | SearchBackend::Searxng { .. } => {
            serde_json::from_slice::<ContentResponse>(body)
                .map_err(|_| WebAccessFailure::Unavailable)?
                .results
                .into_iter()
                .map(|hit| (hit.title, hit.url, hit.content))
                .collect()
        }
    };
    Ok(hits
        .into_iter()
        .filter(|(_, url, _)| url.starts_with("http://") || url.starts_with("https://"))
        .map(|(title, url, text)| WebSearchHit {
            title: snippet(&title),
            url,
            snippet: snippet(&text),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_are_reachable() {
        for (address, public) in [
            ("8.8.8.8", true),
            ("2606:4700::1111", true),
            ("127.0.0.1", false),
            ("10.1.2.3", false),
            ("172.16.0.1", false),
            ("192.168.1.1", false),
            ("169.254.169.254", false),
            ("100.64.0.1", false),
            ("0.0.0.0", false),
            ("::1", false),
            ("fd00::1", false),
            ("fe80::1", false),
            ("::ffff:127.0.0.1", false),
        ] {
            assert_eq!(is_public(address.parse().unwrap()), public, "{address}");
        }
        for (url, allowed) in [
            ("https://example.com/a", true),
            ("http://localhost:8080", false),
            ("http://printer.local", false),
            ("http://[::1]/", false),
            ("http://127.0.0.1/", false),
        ] {
            assert_eq!(host_allowed(&Url::parse(url).unwrap()), allowed, "{url}");
        }
    }

    #[tokio::test]
    async fn private_pages_are_refused_before_connecting() {
        let access = WebAccess::new(None).unwrap();
        for url in [
            "http://127.0.0.1:9/",
            "http://localhost/",
            "file:///etc/passwd",
        ] {
            assert_eq!(
                access.fetch(url).await.err(),
                Some(WebAccessFailure::AddressNotAllowed),
                "{url}"
            );
        }
        assert_eq!(
            access.search("query", 3).await.err(),
            Some(WebAccessFailure::NotConfigured)
        );
    }

    #[test]
    fn backend_responses_become_plain_hits() {
        let brave = br#"{"web":{"results":[{"title":"GDP <strong>revision</strong>","url":"https://a.example/gdp","description":"Revised &amp; <strong>final</strong>"},{"title":"x","url":"javascript:alert(1)","description":""}]}}"#;
        let hits = parse_hits(
            &SearchBackend::Brave {
                api_key: "k".into(),
            },
            brave,
        )
        .unwrap();
        assert_eq!(
            hits,
            [WebSearchHit {
                title: "GDP revision".into(),
                url: "https://a.example/gdp".into(),
                snippet: "Revised & final".into(),
            }]
        );
        let searxng =
            br#"{"results":[{"title":"CPI","url":"https://b.example","content":"Index"}]}"#;
        let backend = SearchBackend::Searxng {
            base_url: Url::parse("http://127.0.0.1:8888/").unwrap(),
        };
        assert_eq!(parse_hits(&backend, searxng).unwrap()[0].snippet, "Index");
    }

    #[test]
    fn pages_become_text_with_titles() {
        let html = "<html><head><TITLE>Rate &amp; Policy</TITLE></head><body><h1>Rates</h1><p>Cut by 25bp.</p><script>x()</script></body></html>";
        assert_eq!(html_title(html).as_deref(), Some("Rate & Policy"));
        let text = html_to_text(html).unwrap();
        assert!(text.contains("Cut by 25bp."));
        assert!(!text.contains("x()"));
        assert_eq!(
            decode(&[0xc4, 0xe3, 0xba, 0xc3], "text/html; charset=GBK"),
            "你好"
        );
    }

    #[tokio::test]
    #[ignore = "reaches the public network"]
    async fn public_pages_are_read_through_redirects() {
        let access = WebAccess::new(None).unwrap();
        let page = access.fetch("http://example.com/").await.unwrap();
        assert_eq!(page.title.as_deref(), Some("Example Domain"));
        assert!(
            page.text.contains("documentation examples"),
            "{}",
            page.text
        );
    }
}
