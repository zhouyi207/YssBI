//! Provider-neutral contracts shared by YssBI automation hosts and adapters.

#![forbid(unsafe_code)]

mod harness;
mod knowledge_memory;
mod persistence;
mod statistics;

pub use harness::*;
pub use knowledge_memory::*;
pub use persistence::*;
pub use statistics::*;

use std::{collections::BTreeMap, future::Future, pin::Pin};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

pub const MAX_RESOURCE_ID_BYTES: usize = 1_024;
pub const MAX_CATALOG_QUERY_BYTES: usize = 256;
pub const MAX_LOCALE_BYTES: usize = 32;
pub const MAX_CATALOG_RESULTS: u16 = 100;

macro_rules! string_identity {
    ($name:ident, $label:literal) => {
        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, AutomationIdentityError> {
                let value = value.into();
                if value.trim().is_empty() || value.len() > 128 {
                    return Err(AutomationIdentityError::Invalid($label));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct PrincipalId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct HarnessSessionId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct CapabilityInvocationId(String);

string_identity!(PrincipalId, "principal id");
string_identity!(HarnessSessionId, "harness session id");
string_identity!(CapabilityInvocationId, "capability invocation id");

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AutomationIdentityError {
    #[error("invalid {0}")]
    Invalid(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSessionBinding {
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
}

impl ProjectSessionBinding {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
    ) -> Self {
        Self {
            project_instance_id,
            project_session_id,
        }
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.project_session_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityInvocationContext {
    principal_id: PrincipalId,
    harness_session_id: HarnessSessionId,
    invocation_id: CapabilityInvocationId,
    project: ProjectSessionBinding,
    approval_grant_id: Option<ApprovalGrantId>,
}

impl CapabilityInvocationContext {
    pub fn new(
        principal_id: PrincipalId,
        harness_session_id: HarnessSessionId,
        invocation_id: CapabilityInvocationId,
        project: ProjectSessionBinding,
    ) -> Self {
        Self {
            principal_id,
            harness_session_id,
            invocation_id,
            project,
            approval_grant_id: None,
        }
    }

    pub fn principal_id(&self) -> &PrincipalId {
        &self.principal_id
    }

    pub fn harness_session_id(&self) -> &HarnessSessionId {
        &self.harness_session_id
    }

    pub fn invocation_id(&self) -> &CapabilityInvocationId {
        &self.invocation_id
    }

    pub fn project(&self) -> &ProjectSessionBinding {
        &self.project
    }

    pub fn with_approval(mut self, approval_grant_id: ApprovalGrantId) -> Self {
        self.approval_grant_id = Some(approval_grant_id);
        self
    }

    pub fn approval_grant_id(&self) -> Option<&ApprovalGrantId> {
        self.approval_grant_id.as_ref()
    }
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityId {
    InspectGraph,
    SearchNodeCatalog,
    InspectDatasetSchema,
    InspectDatasetProfile,
    InspectResult,
    InspectProject,
    ApplyGraphEdit,
}

impl CapabilityId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectGraph => "inspect_graph",
            Self::SearchNodeCatalog => "search_node_catalog",
            Self::InspectDatasetSchema => "inspect_dataset_schema",
            Self::InspectDatasetProfile => "inspect_dataset_profile",
            Self::InspectResult => "inspect_result",
            Self::InspectProject => "inspect_project",
            Self::ApplyGraphEdit => "apply_graph_edit",
        }
    }

    pub const fn descriptor(self) -> &'static CapabilityDescriptor {
        match self {
            Self::InspectGraph => &CAPABILITY_DESCRIPTORS[0],
            Self::SearchNodeCatalog => &CAPABILITY_DESCRIPTORS[1],
            Self::InspectDatasetSchema => &CAPABILITY_DESCRIPTORS[2],
            Self::InspectDatasetProfile => &CAPABILITY_DESCRIPTORS[3],
            Self::InspectResult => &CAPABILITY_DESCRIPTORS[4],
            Self::InspectProject => &CAPABILITY_DESCRIPTORS[5],
            Self::ApplyGraphEdit => &CAPABILITY_DESCRIPTORS[6],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolEffect {
    Inspect,
    Compute,
    Mutate,
    Destructive,
    External,
}

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalPolicy {
    Automatic,
    Configurable,
    Required,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptor {
    pub id: CapabilityId,
    pub effect: ToolEffect,
    pub approval: ApprovalPolicy,
    pub maximum_results: u16,
}

pub const CAPABILITY_DESCRIPTORS: [CapabilityDescriptor; 7] = [
    CapabilityDescriptor {
        id: CapabilityId::InspectGraph,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::SearchNodeCatalog,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: MAX_CATALOG_RESULTS,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatasetSchema,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: 4_096,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatasetProfile,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectResult,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectProject,
        effect: ToolEffect::Inspect,
        approval: ApprovalPolicy::Automatic,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::ApplyGraphEdit,
        effect: ToolEffect::Mutate,
        approval: ApprovalPolicy::Required,
        maximum_results: 200,
    },
];

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectGraphRequest {
    pub graph_path: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchNodeCatalogRequest {
    pub query: String,
    pub locale: String,
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatasetSchemaRequest {
    pub database_id: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatasetProfileRequest {
    pub database_id: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectResultRequest {
    pub result_id: u64,
}

#[derive(Clone, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectProjectRequest {}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditPosition {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphEditPortRef {
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

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum GraphEditOperation {
    CreateNode {
        node_type_id: String,
        resource_path: Option<String>,
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
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyGraphEditRequest {
    pub graph_path: String,
    pub base_revision: u64,
    pub client_key: String,
    pub locale: String,
    pub operations: Vec<GraphEditOperation>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityRequest {
    InspectGraph(InspectGraphRequest),
    SearchNodeCatalog(SearchNodeCatalogRequest),
    InspectDatasetSchema(InspectDatasetSchemaRequest),
    InspectDatasetProfile(InspectDatasetProfileRequest),
    InspectResult(InspectResultRequest),
    InspectProject(InspectProjectRequest),
    ApplyGraphEdit(ApplyGraphEditRequest),
}

impl AutomationCapabilityRequest {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::InspectGraph(_) => CapabilityId::InspectGraph,
            Self::SearchNodeCatalog(_) => CapabilityId::SearchNodeCatalog,
            Self::InspectDatasetSchema(_) => CapabilityId::InspectDatasetSchema,
            Self::InspectDatasetProfile(_) => CapabilityId::InspectDatasetProfile,
            Self::InspectResult(_) => CapabilityId::InspectResult,
            Self::InspectProject(_) => CapabilityId::InspectProject,
            Self::ApplyGraphEdit(_) => CapabilityId::ApplyGraphEdit,
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        match self {
            Self::InspectGraph(request) => validate_resource_id("graphPath", &request.graph_path),
            Self::InspectDatasetSchema(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
            Self::InspectDatasetProfile(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
            Self::InspectResult(_) => Ok(()),
            Self::InspectProject(_) => Ok(()),
            Self::ApplyGraphEdit(request) => {
                validate_resource_id("graphPath", &request.graph_path)?;
                if request.client_key.trim().is_empty() || request.client_key.len() > 128 {
                    return Err(CapabilityContractError::InvalidField("clientKey"));
                }
                if request.locale.trim().is_empty() || request.locale.len() > MAX_LOCALE_BYTES {
                    return Err(CapabilityContractError::InvalidField("locale"));
                }
                if request.operations.is_empty() || request.operations.len() > 200 {
                    return Err(CapabilityContractError::InvalidLimit { maximum: 200 });
                }
                for operation in &request.operations {
                    validate_graph_edit_operation(operation)?;
                }
                Ok(())
            }
            Self::SearchNodeCatalog(request) => {
                if request.query.len() > MAX_CATALOG_QUERY_BYTES {
                    return Err(CapabilityContractError::FieldTooLong {
                        field: "query",
                        maximum: MAX_CATALOG_QUERY_BYTES,
                    });
                }
                if request.locale.trim().is_empty() || request.locale.len() > MAX_LOCALE_BYTES {
                    return Err(CapabilityContractError::InvalidField("locale"));
                }
                if request.limit == 0 || request.limit > MAX_CATALOG_RESULTS {
                    return Err(CapabilityContractError::InvalidLimit {
                        maximum: MAX_CATALOG_RESULTS,
                    });
                }
                Ok(())
            }
        }
    }
}

fn validate_graph_edit_operation(
    operation: &GraphEditOperation,
) -> Result<(), CapabilityContractError> {
    match operation {
        GraphEditOperation::CreateNode {
            node_type_id,
            resource_path,
            x,
            y,
            user_label,
        } => {
            validate_resource_id("nodeTypeId", node_type_id)?;
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

fn validate_resource_id(field: &'static str, value: &str) -> Result<(), CapabilityContractError> {
    if value.trim().is_empty() {
        return Err(CapabilityContractError::InvalidField(field));
    }
    if value.len() > MAX_RESOURCE_ID_BYTES {
        return Err(CapabilityContractError::FieldTooLong {
            field,
            maximum: MAX_RESOURCE_ID_BYTES,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CapabilityContractError {
    #[error("invalid field '{0}'")]
    InvalidField(&'static str),
    #[error("field '{field}' exceeds {maximum} bytes")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("result limit must be between 1 and {maximum}")]
    InvalidLimit { maximum: u16 },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphNodeInspection {
    pub node_id: String,
    pub node_type_id: String,
    pub user_label: Option<String>,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphInspection {
    pub graph_path: String,
    pub nodes: Vec<GraphNodeInspection>,
    pub connections: Vec<GraphConnectionInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogMatch {
    pub node_type_id: String,
    pub title: String,
    pub category_id: String,
    pub style_id: String,
    pub resource_path: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeCatalogSearchResult {
    pub locale: String,
    pub matches: Vec<NodeCatalogMatch>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetSchemaInspection {
    pub database_id: String,
    pub runtime_revision: u64,
    pub schema_revision: u64,
    pub columns: Vec<DatasetColumnSchema>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetProfileInspection {
    pub database_id: String,
    pub runtime_revision: u64,
    pub schema_revision: u64,
    pub row_count: usize,
    pub column_count: usize,
    pub estimated_memory_bytes: Option<usize>,
    pub duplicated_rows: Option<usize>,
    pub numeric_columns: usize,
    pub categorical_columns: usize,
    pub string_columns: usize,
    pub temporal_columns: usize,
    pub boolean_columns: usize,
    pub total_nulls: usize,
    pub null_ratio: f64,
    pub columns_with_nulls: usize,
    pub rows_with_nulls: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultCategoryInspection {
    Value,
    PlotData { plot_kind: String },
    StatisticalReport { report_kind: String },
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ResultValueInspection {
    Null,
    Boolean(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(f64),
    String {
        value: String,
        truncated: bool,
    },
    List {
        items: Vec<ResultValueInspection>,
        total_count: usize,
        truncated: bool,
    },
    Record {
        entries: BTreeMap<String, ResultValueInspection>,
        total_count: usize,
        truncated: bool,
    },
    Resource {
        resource_id: String,
    },
    Empty,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultInspection {
    pub result_id: u64,
    pub category: ResultCategoryInspection,
    pub value: ResultValueInspection,
}

#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectResourceKindInspection {
    Graph,
    Database,
    Chart,
    Variable,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectResourceInspection {
    pub kind: ProjectResourceKindInspection,
    pub resource_id: String,
    pub display_name: String,
    pub revision: Option<u64>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectInspection {
    pub project_name: String,
    pub resources: Vec<ProjectResourceInspection>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphEditReceipt {
    pub graph_path: String,
    pub from_revision: u64,
    pub to_revision: u64,
    pub operation_id: String,
    pub client_key: String,
    pub can_undo: bool,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityResult {
    GraphInspection(GraphInspection),
    NodeCatalogSearch(NodeCatalogSearchResult),
    DatasetSchemaInspection(DatasetSchemaInspection),
    DatasetProfileInspection(DatasetProfileInspection),
    ResultInspection(ResultInspection),
    ProjectInspection(ProjectInspection),
    GraphEditReceipt(GraphEditReceipt),
}

impl AutomationCapabilityResult {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::GraphInspection(_) => CapabilityId::InspectGraph,
            Self::NodeCatalogSearch(_) => CapabilityId::SearchNodeCatalog,
            Self::DatasetSchemaInspection(_) => CapabilityId::InspectDatasetSchema,
            Self::DatasetProfileInspection(_) => CapabilityId::InspectDatasetProfile,
            Self::ResultInspection(_) => CapabilityId::InspectResult,
            Self::ProjectInspection(_) => CapabilityId::InspectProject,
            Self::GraphEditReceipt(_) => CapabilityId::ApplyGraphEdit,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize, thiserror::Error,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityFailureCode {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("project_session_unavailable")]
    ProjectSessionUnavailable,
    #[error("project_session_mismatch")]
    ProjectSessionMismatch,
    #[error("project_session_changed")]
    ProjectSessionChanged,
    #[error("graph_unavailable")]
    GraphUnavailable,
    #[error("database_unavailable")]
    DatabaseUnavailable,
    #[error("catalog_unavailable")]
    CatalogUnavailable,
    #[error("result_unavailable")]
    ResultUnavailable,
    #[error("approval_required")]
    ApprovalRequired,
    #[error("revision_conflict")]
    RevisionConflict,
    #[error("mutation_rejected")]
    MutationRejected,
    #[error("result_too_large")]
    ResultTooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error("deadline_elapsed")]
    DeadlineElapsed,
    #[error("invocation_conflict")]
    InvocationConflict,
    #[error("persistence_unavailable")]
    PersistenceUnavailable,
    #[error("internal_failure")]
    InternalFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityFailure {
    pub code: CapabilityFailureCode,
    pub details: BTreeMap<String, String>,
}

impl CapabilityFailure {
    pub fn new(code: CapabilityFailureCode) -> Self {
        Self {
            code,
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

pub type CapabilityFuture<'a> = Pin<
    Box<dyn Future<Output = Result<AutomationCapabilityResult, CapabilityFailure>> + Send + 'a>,
>;

pub trait CapabilityGatewayPort: Send + Sync {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
    ) -> CapabilityFuture<'a>;
}

pub fn capability_input_schema(capability_id: CapabilityId) -> schemars::Schema {
    match capability_id {
        CapabilityId::InspectGraph => schemars::schema_for!(InspectGraphRequest),
        CapabilityId::SearchNodeCatalog => schemars::schema_for!(SearchNodeCatalogRequest),
        CapabilityId::InspectDatasetSchema => {
            schemars::schema_for!(InspectDatasetSchemaRequest)
        }
        CapabilityId::InspectDatasetProfile => {
            schemars::schema_for!(InspectDatasetProfileRequest)
        }
        CapabilityId::InspectResult => schemars::schema_for!(InspectResultRequest),
        CapabilityId::InspectProject => schemars::schema_for!(InspectProjectRequest),
        CapabilityId::ApplyGraphEdit => schemars::schema_for!(ApplyGraphEditRequest),
    }
}

pub fn capability_output_schema(capability_id: CapabilityId) -> schemars::Schema {
    match capability_id {
        CapabilityId::InspectGraph => schemars::schema_for!(GraphInspection),
        CapabilityId::SearchNodeCatalog => schemars::schema_for!(NodeCatalogSearchResult),
        CapabilityId::InspectDatasetSchema => schemars::schema_for!(DatasetSchemaInspection),
        CapabilityId::InspectDatasetProfile => schemars::schema_for!(DatasetProfileInspection),
        CapabilityId::InspectResult => schemars::schema_for!(ResultInspection),
        CapabilityId::InspectProject => schemars::schema_for!(ProjectInspection),
        CapabilityId::ApplyGraphEdit => schemars::schema_for!(GraphEditReceipt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_and_requests_reject_ambiguous_or_unbounded_input() {
        assert!(PrincipalId::try_new(" ").is_err());
        assert!(HarnessSessionId::try_new("session-1").is_ok());

        let request = AutomationCapabilityRequest::SearchNodeCatalog(SearchNodeCatalogRequest {
            query: "regression".to_owned(),
            locale: "en-US".to_owned(),
            limit: MAX_CATALOG_RESULTS + 1,
        });
        assert_eq!(
            request.validate(),
            Err(CapabilityContractError::InvalidLimit {
                maximum: MAX_CATALOG_RESULTS,
            })
        );
    }

    #[test]
    fn capability_registry_is_closed_and_schema_generation_is_available() {
        assert_eq!(CAPABILITY_DESCRIPTORS.len(), 7);
        assert!(CAPABILITY_DESCRIPTORS[..6].iter().all(|descriptor| {
            descriptor.effect == ToolEffect::Inspect
                && descriptor.approval == ApprovalPolicy::Automatic
        }));
        assert_eq!(
            CapabilityId::ApplyGraphEdit.descriptor().approval,
            ApprovalPolicy::Required
        );
        assert_eq!(
            CapabilityId::InspectDatasetSchema.descriptor().id,
            CapabilityId::InspectDatasetSchema
        );
        let _request_schema = schemars::schema_for!(AutomationCapabilityRequest);
        let _result_schema = schemars::schema_for!(AutomationCapabilityResult);
    }
}
