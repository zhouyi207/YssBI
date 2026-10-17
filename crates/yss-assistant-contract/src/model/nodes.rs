//! Explicit graph-authoring intentions, translated to the existing atomic edit contract.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodePositionInput {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeDeclaration {
    /// Optional unique batch alias, without "$". Refer to this node as "$clientId" in this batch's connections; use createdNodes' actual UUID in later calls.
    #[serde(default)]
    pub client_id: Option<String>,
    pub type_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub port_counts: BTreeMap<String, u16>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub position: NodePositionInput,
    /// Exact resourcePath returned by browse_nodes for a bound type.
    #[serde(default)]
    pub resource_path: Option<String>,
    /// Existing constant identity; only valid for yssbi.constant.get.
    #[serde(default)]
    pub constant_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionDeclaration {
    pub output: GraphEditPortRef,
    pub input: GraphEditPortRef,
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNodesInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub nodes: Vec<NodeDeclaration>,
    #[serde(default)]
    pub connections: Vec<ConnectionDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeLiteralUpdate {
    pub address: GraphEditPortRef,
    /// JSON null clears the literal override.
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeUpdate {
    pub node_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub port_counts: BTreeMap<String, u16>,
    /// Remove exact configurable pins, retaining other pin identities. Do not combine with portCounts.
    #[serde(default)]
    pub remove_pins: Vec<GraphEditPortRef>,
    /// Omit to retain the label; null clears it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::graph::present_optional"
    )]
    pub label: Option<Option<String>>,
    #[serde(default)]
    pub literals: Vec<NodeLiteralUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateNodesInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub nodes: Vec<NodeUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteNodesInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateNodesInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub node_ids: Vec<String>,
    #[serde(default = "default_duplicate_offset")]
    pub offset: NodePositionInput,
}

fn default_duplicate_offset() -> NodePositionInput {
    NodePositionInput { x: 40., y: 40. }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveNodesInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub positions: Vec<GraphEditPosition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConnectionsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub connections: Vec<ConnectionDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateConnectionsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub connections: Vec<GraphConnectionUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteConnectionsInput {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub connection_ids: Vec<String>,
}

/// Kept internally in the existing ledger so replay retains the original public action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum GraphMutationInput {
    CreateNodes(CreateNodesInput),
    CreateConstants(CreateConstantsInput),
    UpdateConstants(UpdateConstantsInput),
    DeleteConstants(DeleteConstantsInput),

    UpdateNodes(UpdateNodesInput),
    DeleteNodes(DeleteNodesInput),
    DuplicateNodes(DuplicateNodesInput),
    MoveNodes(MoveNodesInput),
    CreateConnections(CreateConnectionsInput),
    UpdateConnections(UpdateConnectionsInput),
    DeleteConnections(DeleteConnectionsInput),
}

impl GraphMutationInput {
    /// Business items for display, without expanding or copying large constant values.
    pub fn item_count(&self) -> usize {
        match self {
            Self::CreateNodes(value) => value.nodes.len() + value.connections.len(),
            Self::UpdateNodes(value) => value.nodes.len(),
            Self::DeleteNodes(value) => value.node_ids.len(),
            Self::DuplicateNodes(value) => value.node_ids.len(),
            Self::MoveNodes(value) => value.positions.len(),
            Self::CreateConnections(value) => value.connections.len(),
            Self::UpdateConnections(value) => value.connections.len(),
            Self::DeleteConnections(value) => value.connection_ids.len(),
            Self::CreateConstants(value) => value.constants.len(),
            Self::UpdateConstants(value) => value.constants.len(),
            Self::DeleteConstants(value) => value.constant_ids.len(),
        }
    }

    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::CreateNodes(_) => CapabilityId::CreateNodes,
            Self::CreateConstants(_) => CapabilityId::CreateConstants,
            Self::UpdateConstants(_) => CapabilityId::UpdateConstants,
            Self::DeleteConstants(_) => CapabilityId::DeleteConstants,

            Self::UpdateNodes(_) => CapabilityId::UpdateNodes,
            Self::DeleteNodes(_) => CapabilityId::DeleteNodes,
            Self::DuplicateNodes(_) => CapabilityId::DuplicateNodes,
            Self::MoveNodes(_) => CapabilityId::MoveNodes,
            Self::CreateConnections(_) => CapabilityId::CreateConnections,
            Self::UpdateConnections(_) => CapabilityId::UpdateConnections,
            Self::DeleteConnections(_) => CapabilityId::DeleteConnections,
        }
    }

    pub fn graph(&self) -> &GraphResourceRef {
        match self {
            Self::CreateNodes(value) => &value.graph,
            Self::CreateConstants(value) => &value.graph,
            Self::UpdateConstants(value) => &value.graph,
            Self::DeleteConstants(value) => &value.graph,

            Self::UpdateNodes(value) => &value.graph,
            Self::DeleteNodes(value) => &value.graph,
            Self::DuplicateNodes(value) => &value.graph,
            Self::MoveNodes(value) => &value.graph,
            Self::CreateConnections(value) => &value.graph,
            Self::UpdateConnections(value) => &value.graph,
            Self::DeleteConnections(value) => &value.graph,
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph().id)?;
        match self {
            Self::CreateConstants(value) => {
                let mut seen = BTreeSet::new();
                for constant in &value.constants {
                    if !seen.insert(&constant.client_id) || constant.client_id.len() > 64 {
                        return Err(CapabilityContractError::InvalidField("clientId"));
                    }
                }
            }
            Self::UpdateConstants(value) => {
                let mut seen = BTreeSet::new();
                for constant in &value.constants {
                    if !seen.insert(&constant.constant_id) {
                        return Err(CapabilityContractError::InvalidField("constantId"));
                    }
                }
            }
            Self::DeleteConstants(value)
                if value.constant_ids.iter().collect::<BTreeSet<_>>().len()
                    != value.constant_ids.len() =>
            {
                return Err(CapabilityContractError::InvalidField("constantIds"));
            }
            Self::CreateNodes(value) => {
                if value.nodes.is_empty() {
                    return Err(CapabilityContractError::InvalidField("nodes"));
                }
                for node in &value.nodes {
                    if node.constant_id.is_some()
                        && (node.type_id != "yssbi.constant.get" || node.resource_path.is_some())
                    {
                        return Err(CapabilityContractError::InvalidField("constantId"));
                    }
                    if let Some(id) = &node.constant_id
                        && node
                            .parameters
                            .get("constant")
                            .is_some_and(|value| value.as_str() != Some(id))
                    {
                        return Err(CapabilityContractError::InvalidField("parameters.constant"));
                    }
                }
            }
            Self::UpdateNodes(value) => {
                let mut seen = BTreeSet::new();
                for node in &value.nodes {
                    if !seen.insert(&node.node_id) {
                        return Err(CapabilityContractError::InvalidField("nodeId"));
                    }
                    if !node.remove_pins.is_empty() && !node.port_counts.is_empty() {
                        return Err(CapabilityContractError::InvalidField("removePins"));
                    }
                    for pin in &node.remove_pins {
                        if !matches!(pin, GraphEditPortRef::Instance { node_id, .. } if node_id == &node.node_id)
                        {
                            return Err(CapabilityContractError::InvalidField("removePins"));
                        }
                    }
                    for literal in &node.literals {
                        let (GraphEditPortRef::Declared { node_id, .. }
                        | GraphEditPortRef::Instance { node_id, .. }) = &literal.address;
                        if node_id != &node.node_id {
                            return Err(CapabilityContractError::InvalidField("literals"));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Pure DTO conversion. All semantic validation and allocation stays in Graph.
    pub fn operations(&self) -> Vec<GraphEditOperation> {
        match self {
            Self::CreateNodes(value) => create_operations(value),
            Self::CreateConstants(value) => value
                .constants
                .iter()
                .cloned()
                .map(|declaration| GraphEditOperation::CreateConstant { declaration })
                .collect(),
            Self::UpdateConstants(value) => value
                .constants
                .iter()
                .cloned()
                .map(|update| GraphEditOperation::UpdateConstant { update })
                .collect(),
            Self::DeleteConstants(value) => value
                .constant_ids
                .iter()
                .cloned()
                .map(|constant_id| GraphEditOperation::DeleteConstant { constant_id })
                .collect(),
            Self::UpdateNodes(value) => value
                .nodes
                .iter()
                .flat_map(|node| {
                    let mut operations = Vec::new();
                    if !node.parameters.is_empty() {
                        operations.push(GraphEditOperation::SetParameters {
                            node_id: node.node_id.clone(),
                            parameters: node.parameters.clone(),
                        });
                    }
                    if !node.port_counts.is_empty() {
                        operations.push(GraphEditOperation::SetPortCounts {
                            node_id: node.node_id.clone(),
                            counts: node.port_counts.clone(),
                        });
                    }
                    operations.extend(
                        node.remove_pins
                            .iter()
                            .cloned()
                            .map(|address| GraphEditOperation::RemovePortInstance { address }),
                    );
                    if let Some(label) = &node.label {
                        operations.push(GraphEditOperation::SetNodeLabel {
                            node_id: node.node_id.clone(),
                            label: label.clone(),
                        });
                    }
                    operations.extend(node.literals.iter().map(|literal| {
                        GraphEditOperation::SetLiteral {
                            address: literal.address.clone(),
                            literal: (!literal.value.is_null()).then(|| literal.value.clone()),
                        }
                    }));
                    operations
                })
                .collect(),
            Self::DeleteNodes(value) => vec![GraphEditOperation::DeleteNodes {
                node_ids: value.node_ids.clone(),
            }],
            Self::DuplicateNodes(value) => vec![GraphEditOperation::DuplicateNodes {
                node_ids: value.node_ids.clone(),
                offset_x: value.offset.x,
                offset_y: value.offset.y,
            }],
            Self::MoveNodes(value) => vec![GraphEditOperation::MoveNodes {
                positions: value.positions.clone(),
            }],
            Self::CreateConnections(value) => {
                value.connections.iter().map(connection_operation).collect()
            }
            Self::UpdateConnections(value) => vec![GraphEditOperation::UpdateConnections {
                connections: value.connections.clone(),
            }],
            Self::DeleteConnections(value) => vec![GraphEditOperation::DisconnectConnections {
                connection_ids: value.connection_ids.clone(),
            }],
        }
    }
}

fn connection_operation(value: &ConnectionDeclaration) -> GraphEditOperation {
    GraphEditOperation::Connect {
        output: value.output.clone(),
        input: value.input.clone(),
        order: value.order.clone(),
    }
}

fn create_operations(value: &CreateNodesInput) -> Vec<GraphEditOperation> {
    let mut aliases = value
        .nodes
        .iter()
        .filter_map(|node| node.client_id.clone())
        .collect::<BTreeSet<_>>();
    let mut operations = Vec::new();
    for (index, node) in value.nodes.iter().enumerate() {
        let alias = node.client_id.clone().unwrap_or_else(|| {
            let mut index = index;
            loop {
                let candidate = format!("node_{index}");
                if aliases.insert(candidate.clone()) {
                    break candidate;
                }
                index += 1;
            }
        });
        if let Some(id) = &node.constant_id {
            operations.push(GraphEditOperation::InsertConstantReference {
                id: id.clone(),
                x: node.position.x,
                y: node.position.y,
                client_id: Some(alias.clone()),
            });
            if !node.parameters.is_empty() {
                operations.push(GraphEditOperation::SetParameters {
                    node_id: format!("${alias}"),
                    parameters: node.parameters.clone(),
                });
            }
            if !node.port_counts.is_empty() {
                operations.push(GraphEditOperation::SetPortCounts {
                    node_id: format!("${alias}"),
                    counts: node.port_counts.clone(),
                });
            }
            if node.label.is_some() {
                operations.push(GraphEditOperation::SetNodeLabel {
                    node_id: format!("${alias}"),
                    label: node.label.clone(),
                });
            }
        } else {
            operations.push(GraphEditOperation::CreateNode {
                client_id: Some(alias),
                node_type_id: node.type_id.clone(),
                resource_path: node.resource_path.clone(),
                parameters: node.parameters.clone(),
                port_counts: node.port_counts.clone(),
                x: node.position.x,
                y: node.position.y,
                user_label: node.label.clone(),
            });
        }
    }
    operations.extend(value.connections.iter().map(connection_operation));
    operations
}

impl From<GraphMutationInput> for super::CapabilityInput {
    fn from(value: GraphMutationInput) -> Self {
        match value {
            GraphMutationInput::CreateNodes(value) => Self::CreateNodes(value),
            GraphMutationInput::CreateConstants(value) => Self::CreateConstants(value),
            GraphMutationInput::UpdateConstants(value) => Self::UpdateConstants(value),
            GraphMutationInput::DeleteConstants(value) => Self::DeleteConstants(value),

            GraphMutationInput::UpdateNodes(value) => Self::UpdateNodes(value),
            GraphMutationInput::DeleteNodes(value) => Self::DeleteNodes(value),
            GraphMutationInput::DuplicateNodes(value) => Self::DuplicateNodes(value),
            GraphMutationInput::MoveNodes(value) => Self::MoveNodes(value),
            GraphMutationInput::CreateConnections(value) => Self::CreateConnections(value),
            GraphMutationInput::UpdateConnections(value) => Self::UpdateConnections(value),
            GraphMutationInput::DeleteConnections(value) => Self::DeleteConnections(value),
        }
    }
}
