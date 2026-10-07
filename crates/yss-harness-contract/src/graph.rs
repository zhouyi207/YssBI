//! Graph inspection, edits, validation and execution receipts.

use crate::{
    CapabilityContractError, MAX_RESOURCE_ID_BYTES, ResultCategoryInspection, validate_resource_id,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod inspection;
pub use inspection::*;
mod queries;
pub use queries::*;
mod constants;
pub use constants::*;

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
    pub graph: GraphResourceRef,
    pub graph_hash: String,
    pub node_ids: Vec<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteGraphRequest {
    pub graph: GraphResourceRef,
    pub graph_hash: String,
    pub demand: GraphExecutionDemand,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GraphExecutionDemand {
    Default,
    Node {
        node_id: String,
        mode: NodeExecutionMode,
    },
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeExecutionMode {
    CurrentInputs,
    Dependencies,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveGraphRequest {
    pub graph_path: String,
    pub graph_hash: String,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphParameterInspection {
    pub key: String,
    pub title: String,
    pub editor: String,
    pub value: Option<serde_json::Value>,
    /// None means choices are not known; an empty list is a known empty candidate set.
    pub options: Option<Vec<String>>,
    pub context_hint: Option<String>,
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
    pub node_ids: Vec<String>,
    pub scope_node_count: usize,
    pub page: crate::InspectionPage,
    pub diagnostics: Vec<GraphDiagnosticInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphExecution {
    pub graph_path: String,
    pub graph_hash: String,
    pub run_id: Option<u64>,
    pub status: String,
    /// Absent if rejected before run admission or its metadata is no longer retained.
    pub timing: Option<GraphRunTiming>,
    pub failure_code: Option<String>,
    pub failure_location: Option<String>,
    /// Number of results published by a successful run; unknown when execution failed.
    pub result_count: Option<usize>,
    pub results_complete: bool,
    pub results: Vec<GraphResultReference>,
}

/// Monotonic wall time from run admission, independent of tool-call/queue duration.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphRunTiming {
    pub elapsed_ms: u64,
    pub admission_ms: u64,
    /// Includes scheduling, resource preparation and result materialization, not just kernels.
    pub running_ms: u64,
    pub finalization_ms: u64,
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
    pub result_ref: crate::ResultRef,
    pub validity: crate::ResultValidity,
    pub run_id: u64,
    pub output: String,
    pub category: ResultCategoryInspection,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphResults {
    pub graph: GraphResourceRef,
    pub run_status: Option<String>,
    pub run_timing: Option<GraphRunTiming>,
    pub page: crate::InspectionPage,
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

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditPosition {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum GraphEditPortRef {
    Declared {
        /// Existing node UUID, or "$clientId" for a node created in this same batch. A bare clientId is not a node ID.
        node_id: String,
        port_key: String,
    },
    Instance {
        /// Existing node UUID, or "$clientId" for a node created in this same batch. A bare clientId is not a node ID.
        node_id: String,
        template_key: String,
        instance_id: String,
    },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum GraphEditOperation {
    CreateTypedConstant {
        declaration: ConstantDeclaration,
    },
    UpdateTypedConstant {
        update: ConstantUpdate,
    },
    SetNodeLabel {
        node_id: String,
        label: Option<String>,
    },
    SetPortCounts {
        node_id: String,
        counts: BTreeMap<String, u16>,
    },
    UpdateConnections {
        connections: Vec<GraphConnectionUpdate>,
    },
    CreateConstant {
        name: String,
        value: GraphConstantLiteral,
        x: f64,
        y: f64,
        client_id: Option<String>,
    },
    CreateNode {
        client_id: Option<String>,
        node_type_id: String,
        resource_path: Option<String>,
        parameters: BTreeMap<String, serde_json::Value>,
        /// Initial totals per user-created pin template. Fixed and derived pins cannot be overridden.
        /// With clientId, each initial pin is addressable by instanceId "$clientId.template[0]"
        /// (zero-based creation order) in the same batch; receipts return stable instance IDs.
        port_counts: BTreeMap<String, u16>,
        x: f64,
        y: f64,
        user_label: Option<String>,
    },
    MoveNodes {
        positions: Vec<GraphEditPosition>,
    },
    DeleteNodes {
        node_ids: Vec<String>,
    },
    Connect {
        output: GraphEditPortRef,
        input: GraphEditPortRef,
        order: Option<String>,
    },
    DisconnectConnections {
        connection_ids: Vec<String>,
    },
    SetParameters {
        node_id: String,
        parameters: BTreeMap<String, serde_json::Value>,
    },
    SetLiteral {
        address: GraphEditPortRef,
        literal: Option<serde_json::Value>,
    },
    AddPortInstance {
        node_id: String,
        template_key: String,
        client_id: Option<String>,
    },
    RemovePortInstance {
        address: GraphEditPortRef,
    },
    DisconnectPort {
        address: GraphEditPortRef,
    },
    DisconnectNode {
        node_id: String,
    },
    MoveConnections {
        source: GraphEditPortRef,
        target: GraphEditPortRef,
    },
    DuplicateNodes {
        node_ids: Vec<String>,
        offset_x: f64,
        offset_y: f64,
    },
    SetConstant {
        id: String,
        constant: Option<serde_json::Value>,
    },
    InsertConstantReference {
        id: String,
        x: f64,
        y: f64,
        client_id: Option<String>,
    },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyGraphEditRequest {
    pub graph_path: String,
    pub base_revision: u64,
    pub graph_hash: String,
    pub client_key: String,
    pub locale: String,
    pub operations: Vec<GraphEditOperation>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphMutationRequest {
    pub input: crate::model::GraphMutationInput,
    pub base_revision: u64,
    pub graph_hash: String,
    pub client_key: String,
}

impl GraphMutationRequest {
    pub fn edit_request(&self) -> ApplyGraphEditRequest {
        ApplyGraphEditRequest {
            graph_path: self.input.graph().id.clone(),
            base_revision: self.base_revision,
            graph_hash: self.graph_hash.clone(),
            client_key: self.client_key.clone(),
            locale: "en-US".into(),
            operations: self.input.operations(),
        }
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphConnectionUpdate {
    pub connection_id: String,
    pub output: GraphEditPortRef,
    pub input: GraphEditPortRef,
    /// Omit to retain ordering; null clears it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_optional"
    )]
    pub order: Option<Option<String>>,
}

pub(crate) fn present_optional<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

pub(crate) fn validate_graph_edit_operation(
    operation: &GraphEditOperation,
) -> Result<(), CapabilityContractError> {
    match operation {
        GraphEditOperation::CreateTypedConstant { declaration } => {
            validate_resource_id("clientId", &declaration.client_id)?;
            validate_resource_id("name", &declaration.name)?;
            if let Some(node) = &declaration.reference_node
                && (!node.position.x.is_finite() || !node.position.y.is_finite())
            {
                return Err(CapabilityContractError::InvalidField("position"));
            }
        }
        GraphEditOperation::UpdateTypedConstant { update } => {
            validate_resource_id("constantId", &update.constant_id)?;
            if let Some(name) = &update.name {
                validate_resource_id("name", name)?;
            }
        }
        GraphEditOperation::SetNodeLabel { node_id, label } => {
            validate_resource_id("nodeId", node_id)?;
            if label.as_ref().is_some_and(|label| label.len() > 1024) {
                return Err(CapabilityContractError::InvalidField("label"));
            }
        }
        GraphEditOperation::SetPortCounts { node_id, counts } => {
            validate_resource_id("nodeId", node_id)?;
            for key in counts.keys() {
                validate_resource_id("portCounts", key)?;
            }
        }
        GraphEditOperation::UpdateConnections { connections } => {
            if connections.is_empty() || connections.len() > 200 {
                return Err(CapabilityContractError::InvalidField("connections"));
            }
            for connection in connections {
                validate_resource_id("connectionId", &connection.connection_id)?;
                validate_graph_edit_port(&connection.output)?;
                validate_graph_edit_port(&connection.input)?;
                if connection
                    .order
                    .as_ref()
                    .and_then(Option::as_ref)
                    .is_some_and(|order| order.len() > 1024)
                {
                    return Err(CapabilityContractError::InvalidField("order"));
                }
            }
        }
        GraphEditOperation::CreateConstant {
            name, value, x, y, ..
        } => {
            validate_resource_id("name", name)?;
            validate_graph_json(value)?;
            if !x.is_finite()
                || !y.is_finite()
                || matches!(value, GraphConstantLiteral::Decimal(value) if !value.is_finite())
                || matches!(value, GraphConstantLiteral::Integer(value) if value.unsigned_abs() > 9_007_199_254_740_991)
            {
                return Err(CapabilityContractError::InvalidField("value"));
            }
        }
        GraphEditOperation::CreateNode {
            client_id: _,
            node_type_id,
            resource_path,
            parameters,
            port_counts,
            x,
            y,
            user_label,
        } => {
            validate_resource_id("nodeTypeId", node_type_id)?;
            validate_graph_json(parameters)?;
            validate_graph_json(port_counts)?;
            for key in port_counts.keys() {
                validate_resource_id("portCounts", key)?;
            }
            if resource_path
                .as_ref()
                .is_some_and(|path| path.trim().is_empty() || path.len() > MAX_RESOURCE_ID_BYTES)
                || user_label.as_ref().is_some_and(|label| label.len() > 1_024)
                || !x.is_finite()
                || !y.is_finite()
            {
                return Err(CapabilityContractError::InvalidField("operation"));
            }
        }
        GraphEditOperation::MoveNodes { positions } => {
            if positions.is_empty()
                || positions.len() > 200
                || positions.iter().any(|position| {
                    position.node_id.trim().is_empty()
                        || !position.x.is_finite()
                        || !position.y.is_finite()
                })
            {
                return Err(CapabilityContractError::InvalidField("positions"));
            }
        }
        GraphEditOperation::DeleteNodes { node_ids } => {
            if node_ids.is_empty()
                || node_ids.len() > 200
                || node_ids.iter().any(|id| id.trim().is_empty())
            {
                return Err(CapabilityContractError::InvalidField("nodeIds"));
            }
        }
        GraphEditOperation::Connect {
            output,
            input,
            order,
        } => {
            validate_graph_edit_port(output)?;
            validate_graph_edit_port(input)?;
            if order.as_ref().is_some_and(|order| order.len() > 1_024) {
                return Err(CapabilityContractError::InvalidField("order"));
            }
        }
        GraphEditOperation::DisconnectConnections { connection_ids } => {
            if connection_ids.is_empty()
                || connection_ids.len() > 200
                || connection_ids.iter().any(|id| id.trim().is_empty())
            {
                return Err(CapabilityContractError::InvalidField("connectionIds"));
            }
        }
        GraphEditOperation::SetParameters {
            node_id,
            parameters,
        } => {
            validate_resource_id("nodeId", node_id)?;
            validate_graph_json(parameters)?;
        }
        GraphEditOperation::SetLiteral { address, literal } => {
            validate_graph_edit_port(address)?;
            validate_graph_json(literal)?;
        }
        GraphEditOperation::AddPortInstance {
            node_id,
            template_key,
            ..
        } => {
            validate_resource_id("nodeId", node_id)?;
            validate_resource_id("templateKey", template_key)?;
        }
        GraphEditOperation::RemovePortInstance { address }
        | GraphEditOperation::DisconnectPort { address } => validate_graph_edit_port(address)?,
        GraphEditOperation::DisconnectNode { node_id } => validate_resource_id("nodeId", node_id)?,
        GraphEditOperation::MoveConnections { source, target } => {
            validate_graph_edit_port(source)?;
            validate_graph_edit_port(target)?;
        }
        GraphEditOperation::DuplicateNodes {
            node_ids,
            offset_x,
            offset_y,
        } => {
            if node_ids.is_empty()
                || node_ids.len() > 200
                || !offset_x.is_finite()
                || !offset_y.is_finite()
            {
                return Err(CapabilityContractError::InvalidField("nodeIds"));
            }
        }
        GraphEditOperation::SetConstant { id, constant } => {
            validate_resource_id("constantId", id)?;
            validate_graph_json(constant)?;
        }
        GraphEditOperation::InsertConstantReference { id, x, y, .. } => {
            validate_resource_id("constantId", id)?;
            if !x.is_finite() || !y.is_finite() {
                return Err(CapabilityContractError::InvalidField("position"));
            }
        }
    }
    Ok(())
}

fn validate_graph_edit_port(port: &GraphEditPortRef) -> Result<(), CapabilityContractError> {
    let invalid = match port {
        GraphEditPortRef::Declared { node_id, port_key } => {
            node_id.trim().is_empty() || port_key.trim().is_empty()
        }
        GraphEditPortRef::Instance {
            node_id,
            template_key,
            instance_id,
        } => {
            node_id.trim().is_empty()
                || template_key.trim().is_empty()
                || instance_id.trim().is_empty()
        }
    };
    if invalid {
        Err(CapabilityContractError::InvalidField("port"))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNodeInspection {
    pub node_id: String,
    pub node_type_id: String,
    pub user_label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub title: String,
    pub parameters: Vec<GraphParameterInspection>,
    pub ports: Vec<GraphPortFacts>,
    pub port_templates: Vec<GraphPortTemplateInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GraphPortInspection {
    Declared {
        node_id: String,
        port_key: String,
    },
    Instance {
        node_id: String,
        template_key: String,
        instance_id: String,
    },
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphConnectionInspection {
    pub connection_id: String,
    pub output: GraphPortInspection,
    pub input: GraphPortInspection,
    pub order: Option<String>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphInspection {
    pub version: crate::ResourceVersion,
    pub graph_path: String,
    pub semantic_input_hash: String,
    pub ready: bool,
    pub nodes: Vec<GraphNodeInspection>,
    pub connections: Vec<GraphConnectionInspection>,
    pub revision: u64,
    pub graph_hash: String,
    pub constants: BTreeMap<String, serde_json::Value>,
    pub diagnostics: Vec<GraphDiagnosticInspection>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditReceipt {
    pub created_constants: BTreeMap<String, String>,
    pub graph_path: String,
    pub from_revision: u64,
    pub to_revision: u64,
    pub client_key: String,
    pub graph_hash: String,
    pub created_nodes: BTreeMap<String, String>,
    pub created_ports: BTreeMap<String, GraphEditPortRef>,
    pub changes: GraphEditChanges,
}
