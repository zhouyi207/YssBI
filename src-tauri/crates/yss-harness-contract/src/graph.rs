use crate::{
    CapabilityContractError, GraphConnectionInspection, GraphEditPortRef, GraphNodeInspection,
    ResultCategoryInspection,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GraphConstantLiteral {
    Boolean(bool),
    Integer(i64),
    Decimal(f64),
    String(String),
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateGraphRequest {
    pub graph_path: String,
    pub graph_hash: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteGraphRequest {
    pub graph_path: String,
    pub graph_hash: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveGraphRequest {
    pub graph_path: String,
    pub graph_hash: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListGraphResultsRequest {
    pub graph_path: String,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphParameterInspection {
    pub key: String,
    pub title: String,
    pub editor: String,
    pub value: Option<serde_json::Value>,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphPortFacts {
    pub address: GraphEditPortRef,
    pub label: String,
    pub direction: String,
    pub data_type: String,
    pub accepted_type: String,
    pub orphan: bool,
    pub maximum_connections: Option<u32>,
    pub connection_count: u32,
    pub schema: BTreeMap<String, String>,
    pub literal: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphPortTemplateInspection {
    pub key: String,
    pub direction: String,
    pub can_add: bool,
}

#[derive(Clone, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphDiagnosticInspection {
    pub code: String,
    pub message_key: String,
    pub blocking: bool,
    pub severity: String,
    pub location: String,
    pub arguments: BTreeMap<String, String>,
}

/// Complete replacements for changed entities, relative to the receipt's fromRevision.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditChanges {
    pub base_semantic_input_hash: String,
    pub semantic_input_hash: String,
    pub nodes: Vec<GraphNodeInspection>,
    pub removed_node_ids: Vec<String>,
    pub connections: Vec<GraphConnectionInspection>,
    pub removed_connection_ids: Vec<String>,
    pub constants: BTreeMap<String, serde_json::Value>,
    pub removed_constant_ids: Vec<String>,
    pub ready: bool,
    /// The complete diagnostics at this commit, replacing the previous set.
    pub diagnostics: Vec<GraphDiagnosticInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphValidation {
    pub graph_path: String,
    pub graph_hash: String,
    pub ready: bool,
    pub diagnostics: Vec<GraphDiagnosticInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphExecution {
    pub graph_path: String,
    pub graph_hash: String,
    pub run_id: Option<u64>,
    pub status: String,
    pub failure_code: Option<String>,
    pub failure_location: Option<String>,
    /// Number of results published by a successful run; unknown when execution failed.
    pub result_count: Option<usize>,
    pub results_complete: bool,
    pub results: Vec<GraphResultReference>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphSaved {
    pub graph_path: String,
    pub graph_hash: String,
    pub from_revision: u64,
    pub resource_revision: u64,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphResultReference {
    pub execution_session_id: String,
    pub result_id: u64,
    pub run_id: u64,
    pub output: String,
    pub category: ResultCategoryInspection,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphResults {
    pub graph_path: String,
    pub results: Vec<GraphResultReference>,
}

pub(crate) fn validate_graph_hash(hash: &str) -> Result<(), CapabilityContractError> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CapabilityContractError::InvalidField("graphHash"));
    }
    Ok(())
}

pub(crate) fn validate_graph_request(
    path: &str,
    hash: &str,
) -> Result<(), CapabilityContractError> {
    crate::validate_resource_id("graphPath", path)?;
    validate_graph_hash(hash)
}

pub(crate) fn validate_graph_json(value: &impl Serialize) -> Result<(), CapabilityContractError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| CapabilityContractError::InvalidField("value"))?;
    if bytes.len() > 65_536 {
        return Err(CapabilityContractError::FieldTooLong {
            field: "value",
            maximum: 65_536,
        });
    }
    Ok(())
}
