use serde::{Deserialize, Serialize};
pub use yss_tracing::{LogFields, LogLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOrigin {
    Rust,
    Frontend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogDomain {
    Application,
    Execution,
    System,
    Graph,
    Data,
    Ui,
}

impl LogDomain {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "application" => Some(Self::Application),
            "execution" => Some(Self::Execution),
            "system" => Some(Self::System),
            "graph" => Some(Self::Graph),
            "data" => Some(Self::Data),
            "ui" => Some(Self::Ui),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecordDto {
    pub stream_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub level: LogLevel,
    pub origin: LogOrigin,
    pub domain: LogDomain,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub fields: LogFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBatchDto {
    pub stream_id: String,
    pub entries: Vec<LogRecordDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSubscriptionDto {
    pub subscription_id: String,
    pub stream_id: String,
    pub entries: Vec<LogRecordDto>,
    pub latest_sequence: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendLogEntryDto {
    pub level: LogLevel,
    pub domain: LogDomain,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub fields: LogFields,
}
