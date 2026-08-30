use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type DiagnosticFields = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticOrigin {
    Rust,
    Frontend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticDomain {
    Application,
    Execution,
    System,
    Graph,
    Data,
    Ui,
}

impl DiagnosticDomain {
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
pub struct DiagnosticRecordDto {
    pub stream_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub level: DiagnosticLevel,
    pub origin: DiagnosticOrigin,
    pub domain: DiagnosticDomain,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub fields: DiagnosticFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBatchDto {
    pub stream_id: String,
    pub entries: Vec<DiagnosticRecordDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSubscriptionDto {
    pub subscription_id: String,
    pub stream_id: String,
    pub entries: Vec<DiagnosticRecordDto>,
    pub latest_sequence: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendDiagnosticEntryDto {
    pub level: DiagnosticLevel,
    pub domain: DiagnosticDomain,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub fields: DiagnosticFields,
}
