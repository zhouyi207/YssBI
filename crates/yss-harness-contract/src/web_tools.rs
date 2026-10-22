//! Model-visible web search and page reading, and the port that reaches the network.

use std::{future::Future, pin::Pin};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::CapabilityContractError;

pub const WEB_SEARCH_TOOL: &str = "web_search";
pub const WEB_FETCH_TOOL: &str = "web_fetch";

pub const MAX_WEB_RESULTS: u16 = 10;
/// Characters of page text returned by one `web_fetch` call.
pub const WEB_PAGE_WINDOW: usize = 12_000;
const MAX_WEB_QUERY_CHARS: usize = 400;
const MAX_WEB_URL_BYTES: usize = 2_048;

/// Tools the Harness executes itself rather than an extension.
pub fn is_harness_tool(name: &str) -> bool {
    matches!(
        name,
        crate::SEARCH_KNOWLEDGE_TOOL
            | crate::READ_KNOWLEDGE_TOOL
            | WEB_SEARCH_TOOL
            | WEB_FETCH_TOOL
    ) || crate::is_python_tool(name)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 10))]
    pub limit: u16,
}

fn default_limit() -> u16 {
    5
}

impl WebSearchRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        let query = self.query.trim();
        if query.is_empty() || query.chars().count() > MAX_WEB_QUERY_CHARS {
            return Err(CapabilityContractError::InvalidField("query"));
        }
        if self.limit == 0 || self.limit > MAX_WEB_RESULTS {
            return Err(CapabilityContractError::InvalidLimit {
                maximum: MAX_WEB_RESULTS,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSearchResult {
    pub results: Vec<WebSearchHit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebFetchRequest {
    /// An absolute http or https URL, usually one returned by web_search.
    pub url: String,
    /// Character offset into the page text; use nextOffset from a previous call.
    #[serde(default)]
    pub offset: usize,
}

impl WebFetchRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        let url = self.url.trim();
        let scheme = url.split_once("://").map(|(scheme, _)| scheme);
        if url.len() > MAX_WEB_URL_BYTES
            || !scheme.is_some_and(|scheme| {
                scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
            })
        {
            return Err(CapabilityContractError::InvalidField("url"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPage {
    /// The final URL after redirects; cite this one.
    pub url: String,
    pub title: Option<String>,
    pub text: String,
    pub offset: usize,
    pub total_characters: usize,
    pub next_offset: Option<usize>,
}

impl WebPage {
    /// The window of `text` starting at `offset` characters.
    pub fn window(url: String, title: Option<String>, text: &str, offset: usize) -> Self {
        let total_characters = text.chars().count();
        let start = offset.min(total_characters);
        let end = start.saturating_add(WEB_PAGE_WINDOW).min(total_characters);
        Self {
            url,
            title,
            text: text.chars().skip(start).take(end - start).collect(),
            offset: start,
            total_characters,
            next_offset: (end < total_characters).then_some(end),
        }
    }
}

pub struct FetchedPage {
    pub url: String,
    pub title: Option<String>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WebAccessFailure {
    #[error("web search is not configured")]
    NotConfigured,
    #[error("the address is not allowed")]
    AddressNotAllowed,
    #[error("the content type cannot be read as text")]
    UnsupportedContent,
    #[error("the response is too large")]
    TooLarge,
    #[error("the service rejected the request")]
    Rejected,
    #[error("the service is unavailable")]
    Unavailable,
}

pub type WebAccessFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, WebAccessFailure>> + Send + 'a>>;

pub trait WebAccessPort: Send + Sync {
    fn search(&self, query: &str, limit: usize) -> WebAccessFuture<'_, Vec<WebSearchHit>>;
    /// Fetches public http(s) content as plain text. Implementations refuse
    /// private, loopback and link-local addresses, including after redirects.
    fn fetch(&self, url: &str) -> WebAccessFuture<'_, FetchedPage>;
}

/// A port for hosts without network access configured.
pub struct DisabledWebAccess;

impl WebAccessPort for DisabledWebAccess {
    fn search(&self, _: &str, _: usize) -> WebAccessFuture<'_, Vec<WebSearchHit>> {
        Box::pin(async { Err(WebAccessFailure::NotConfigured) })
    }

    fn fetch(&self, _: &str) -> WebAccessFuture<'_, FetchedPage> {
        Box::pin(async { Err(WebAccessFailure::NotConfigured) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_windows_split_on_characters_and_report_the_next_offset() {
        let text = "数据".repeat(WEB_PAGE_WINDOW);
        let first = WebPage::window("https://a.example".into(), None, &text, 0);
        assert_eq!(first.text.chars().count(), WEB_PAGE_WINDOW);
        assert_eq!(first.next_offset, Some(WEB_PAGE_WINDOW));
        let last = WebPage::window(
            "https://a.example".into(),
            None,
            &text,
            2 * WEB_PAGE_WINDOW - 1,
        );
        assert_eq!(last.text, "据");
        assert_eq!(last.next_offset, None);
    }

    #[test]
    fn fetch_requests_accept_only_http_urls() {
        for (url, valid) in [
            ("https://example.com/a", true),
            ("HTTP://example.com", true),
            ("file:///etc/passwd", false),
            ("example.com", false),
        ] {
            let request = WebFetchRequest {
                url: url.into(),
                offset: 0,
            };
            assert_eq!(request.validate().is_ok(), valid, "{url}");
        }
    }
}
