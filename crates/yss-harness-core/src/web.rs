//! Web search and page reading executed by the Harness through the host's network port.

use serde_json::Value;
use yss_harness_contract::{
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, WEB_FETCH_TOOL, WEB_SEARCH_TOOL,
    WebAccessFailure, WebAccessPort, WebFetchRequest, WebPage, WebSearchRequest, WebSearchResult,
};

pub(crate) fn is_web_tool(name: &str) -> bool {
    matches!(name, WEB_SEARCH_TOOL | WEB_FETCH_TOOL)
}

pub(crate) async fn invoke(
    web: &dyn WebAccessPort,
    tool: &str,
    request: &Value,
    control: &CapabilityControl,
) -> Result<Value, CapabilityFailure> {
    let invalid = |field: &str| {
        CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
            .with_detail("capabilityId", tool)
            .with_detail("field", field)
    };
    control.check()?;
    let work = async {
        let value = match tool {
            WEB_SEARCH_TOOL => {
                let request: WebSearchRequest =
                    serde_json::from_value(request.clone()).map_err(|_| invalid("query"))?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "limit"))?;
                let results = web
                    .search(request.query.trim(), usize::from(request.limit))
                    .await
                    .map_err(failure)?;
                serde_json::to_value(WebSearchResult { results })
            }
            WEB_FETCH_TOOL => {
                let request: WebFetchRequest =
                    serde_json::from_value(request.clone()).map_err(|_| invalid("url"))?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "url"))?;
                let page = web.fetch(request.url.trim()).await.map_err(failure)?;
                serde_json::to_value(WebPage::window(
                    page.url,
                    page.title,
                    &page.text,
                    request.offset,
                ))
            }
            _ => return Err(invalid("capabilityId")),
        };
        value.map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))
    };
    tokio::select! {
        biased;
        _ = control.cancellation().cancelled() => {
            Err(control.check().err().unwrap_or_else(|| CapabilityFailure::new(CapabilityFailureCode::CANCELLED)))
        }
        _ = tokio::time::sleep_until(control.deadline().into()) => {
            Err(CapabilityFailure::new(CapabilityFailureCode::DEADLINE_ELAPSED))
        }
        result = work => result,
    }
}

fn failure(error: WebAccessFailure) -> CapabilityFailure {
    let (code, reason, next_step) = match error {
        WebAccessFailure::NotConfigured => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "web_search_not_configured",
            "Tell the user web search is not configured in Settings and continue without it.",
        ),
        WebAccessFailure::AddressNotAllowed => (
            CapabilityFailureCode::INVALID_REQUEST,
            "web_address_not_allowed",
            "Only public http and https addresses can be read.",
        ),
        WebAccessFailure::UnsupportedContent => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "web_content_unsupported",
            "Choose another source that serves HTML or text.",
        ),
        WebAccessFailure::TooLarge => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "web_content_too_large",
            "Choose a smaller page or another source.",
        ),
        WebAccessFailure::Rejected => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "web_request_rejected",
            "Try another query or source.",
        ),
        WebAccessFailure::Unavailable => (
            CapabilityFailureCode::RESULT_UNAVAILABLE,
            "web_service_unavailable",
            "Retry later or continue without web sources.",
        ),
    };
    CapabilityFailure::new(code)
        .with_detail("reason", reason)
        .with_detail("nextStep", next_step)
}
