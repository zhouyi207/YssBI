//! Demand-driven graph reads. Full edit facts remain owned by the graph contract.
use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphInspectionView {
    Summary,
    #[default]
    Overview,
    Nodes,
    Ports,
    Connections,
    Diagnostics,
    Constants,
    Full,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectGraphRequest {
    pub graph: GraphResourceRef,
    /// Overview lists node identities only. Read other views only as needed.
    #[serde(default)]
    pub view: GraphInspectionView,
    /// Restrict nodes, ports or incident connections to these exact node IDs.
    #[serde(default)]
    pub node_ids: Vec<String>,
    /// Restrict the ports view to exact addresses obtained from receipts or prior reads.
    #[serde(default)]
    pub port_addresses: Vec<GraphEditPortRef>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_graph_page_limit")]
    pub limit: usize,
    /// Only the ports view can include resolved column schemas.
    #[serde(default)]
    pub include_schema: bool,
    /// Only the nodes view can include current parameter choices and editor hints.
    #[serde(default)]
    pub include_options: bool,
    /// Observation hash from the same query/page; a match returns unchanged without items.
    #[serde(default)]
    pub if_unchanged: Option<String>,
}

fn default_graph_page_limit() -> usize {
    50
}

impl InspectGraphRequest {
    pub fn overview(graph_path: impl Into<String>) -> Self {
        Self {
            graph: GraphResourceRef::for_path(graph_path),
            view: GraphInspectionView::Overview,
            node_ids: vec![],
            port_addresses: vec![],
            offset: 0,
            limit: default_graph_page_limit(),
            include_schema: false,
            include_options: false,
            if_unchanged: None,
        }
    }

    pub fn summary(graph: GraphResourceRef) -> Self {
        Self {
            graph: graph.clone(),
            view: GraphInspectionView::Summary,
            ..Self::overview(graph.id)
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("graph", &self.graph.id)?;
        if self.limit == 0 {
            return Err(CapabilityContractError::InvalidField("limit"));
        }
        for id in &self.node_ids {
            validate_resource_id("nodeIds", id)?;
        }
        for address in &self.port_addresses {
            super::validate_graph_edit_port(address)?;
        }
        if !self.node_ids.is_empty()
            && !matches!(
                self.view,
                GraphInspectionView::Overview
                    | GraphInspectionView::Nodes
                    | GraphInspectionView::Ports
                    | GraphInspectionView::Connections
            )
        {
            return Err(CapabilityContractError::InvalidField("nodeIds"));
        }
        if !self.port_addresses.is_empty() && self.view != GraphInspectionView::Ports {
            return Err(CapabilityContractError::InvalidField("portAddresses"));
        }
        if self.include_schema && self.view != GraphInspectionView::Ports {
            return Err(CapabilityContractError::InvalidField("includeSchema"));
        }
        if self.include_options && self.view != GraphInspectionView::Nodes {
            return Err(CapabilityContractError::InvalidField("includeOptions"));
        }
        if let Some(hash) = &self.if_unchanged
            && (hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(CapabilityContractError::InvalidField("ifUnchanged"));
        }
        if self.view == GraphInspectionView::Full
            && (self.offset != 0 || self.if_unchanged.is_some())
        {
            return Err(CapabilityContractError::InvalidField("view"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphInspectionCounts {
    pub nodes: usize,
    pub ports: usize,
    pub connections: usize,
    pub constants: usize,
    pub diagnostics: usize,
    pub blocking_diagnostics: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphInspectionPagination {
    pub offset: usize,
    pub total: usize,
    /// None means all matching items have been returned by this and preceding pages.
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNodeSummary {
    pub node_id: String,
    pub node_type_id: String,
    pub user_label: Option<String>,
    pub title: String,
    pub port_count: usize,
    /// Present in the nodes view; absent in overview does not mean no parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<BTreeMap<String, Option<serde_json::Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_options: Option<Vec<GraphParameterInspection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_templates: Option<Vec<GraphPortTemplateInspection>>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphPortDetail {
    pub address: GraphEditPortRef,
    pub label: String,
    pub direction: String,
    pub data_type: String,
    pub accepted_type: String,
    pub orphan: bool,
    pub maximum_connections: Option<u32>,
    pub connection_count: u32,
    /// Absent means not requested; false means no schema has been resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_known: Option<bool>,
    /// Only interpret an empty object as an empty schema when schemaKnown is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_page: Option<crate::InspectionPage>,
    pub literal: Option<serde_json::Value>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "items", rename_all = "snake_case")]
pub enum GraphInspectionItems {
    Summary,
    Unchanged,
    Nodes(Vec<GraphNodeSummary>),
    NodeDetails(Vec<GraphNodeDetails>),
    Ports(Vec<GraphPortDetail>),
    Connections(Vec<GraphConnectionInspection>),
    Diagnostics(Vec<GraphDiagnosticInspection>),
    Constants(BTreeMap<String, serde_json::Value>),
    ConstantSummaries(Vec<ConstantSummary>),
    ConstantDetails(Vec<ConstantInspection>),
}

/// How much of the captured graph fits in the bounded initial overview.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphOverviewDetail {
    /// All nodes, effective parameters, literal overrides and connections.
    Configuration,
    /// All nodes and connections; parameters and literals are omitted.
    Topology,
    /// Structure omitted; use the enclosing counts and targeted graph queries.
    Counts,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOverviewLiteral {
    pub address: GraphEditPortRef,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOverview {
    pub detail: GraphOverviewDetail,
    pub nodes: Vec<GraphNodeSummary>,
    pub connections: Vec<GraphConnectionInspection>,
    pub literals: Vec<GraphOverviewLiteral>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphInspectionPage {
    pub graph_path: String,
    pub version: crate::ResourceVersion,
    pub graph_hash: String,
    pub semantic_input_hash: String,
    pub ready: bool,
    pub counts: GraphInspectionCounts,
    pub view: GraphInspectionView,
    /// Binds the graph's document/edit/semantic identity and all query options, including page.
    pub observation_hash: String,
    pub page: Option<GraphInspectionPagination>,
    pub content: GraphInspectionItems,
    pub runs: Option<Vec<GraphRunInspection>>,
    /// Only summary reads include a size-bounded initial graph context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<GraphOverview>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphRunInspection {
    pub run_id: u64,
    pub status: String,
    pub timing: Option<super::GraphRunTiming>,
    /// Whether this run used the inspected graph's current semantic inputs.
    pub current_inputs: bool,
}
