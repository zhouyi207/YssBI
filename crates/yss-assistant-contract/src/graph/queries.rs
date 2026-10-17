//! Business graph queries. Resource kinds are constrained before owner admission.
use super::*;
use crate::{InspectionPage, ProjectResourceKind, ProjectResourceRef};

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphResourceKind {
    EventGraph,
    FunctionGraph,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphResourceRef {
    pub kind: GraphResourceKind,
    pub id: String,
}

impl GraphResourceRef {
    /// Internal callers already hold an owner-validated path; new public calls carry both fields.
    pub fn for_path(path: impl Into<String>) -> Self {
        let id = path.into();
        let kind = if id.starts_with("functions/") {
            GraphResourceKind::FunctionGraph
        } else {
            GraphResourceKind::EventGraph
        };
        Self { kind, id }
    }
    pub fn resource(&self) -> ProjectResourceRef {
        ProjectResourceRef {
            kind: match self.kind {
                GraphResourceKind::EventGraph => ProjectResourceKind::EventGraph,
                GraphResourceKind::FunctionGraph => ProjectResourceKind::FunctionGraph,
            },
            id: self.id.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindNodesRequest {
    pub graph: GraphResourceRef,
    /// Case-insensitive substring of the node's name, title or ID.
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub type_ids: Vec<String>,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_query_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeInspectionField {
    Parameters,
    Ports,
    Schema,
    Options,
}

fn default_node_fields() -> Vec<NodeInspectionField> {
    vec![NodeInspectionField::Parameters, NodeInspectionField::Ports]
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectNodesRequest {
    pub graph: GraphResourceRef,
    #[schemars(length(min = 1, max = 100))]
    pub node_ids: Vec<String>,
    #[serde(default = "default_node_fields")]
    pub fields: Vec<NodeInspectionField>,
    /// Applied separately to each selected node's ports.
    #[serde(default)]
    pub port_offset: usize,
    #[serde(default = "default_query_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub port_limit: usize,
    /// Applied separately to each selected port's known schema.
    #[serde(default)]
    pub column_offset: usize,
    #[serde(default = "default_query_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub column_limit: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindConnectionsRequest {
    pub graph: GraphResourceRef,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub ports: Vec<GraphEditPortRef>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_query_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

fn default_query_limit() -> usize {
    50
}

pub(crate) fn validate_query_limit(limit: usize) -> Result<(), CapabilityContractError> {
    if (1..=100).contains(&limit) {
        Ok(())
    } else {
        Err(CapabilityContractError::InvalidLimit { maximum: 100 })
    }
}

impl FindNodesRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        validate_query_limit(self.limit)?;
        for id in self.node_ids.iter().chain(&self.type_ids) {
            validate_resource_id("nodeIds/typeIds", id)?;
        }
        if self
            .query
            .as_ref()
            .is_some_and(|q| q.len() > crate::MAX_CATALOG_QUERY_BYTES)
        {
            return Err(CapabilityContractError::InvalidField("query"));
        }
        Ok(())
    }
}

impl InspectNodesRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        if self.node_ids.is_empty() || self.node_ids.len() > 100 {
            return Err(CapabilityContractError::InvalidField("nodeIds"));
        }
        for id in &self.node_ids {
            validate_resource_id("nodeIds", id)?;
        }
        validate_query_limit(self.port_limit)?;
        validate_query_limit(self.column_limit)
    }
}

impl FindConnectionsRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        validate_query_limit(self.limit)?;
        for id in &self.node_ids {
            validate_resource_id("nodeIds", id)?;
        }
        for port in &self.ports {
            super::validate_graph_edit_port(port)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNodeDetails {
    pub node: GraphNodeSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<GraphPortDetail>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_page: Option<InspectionPage>,
}
