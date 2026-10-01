//! Closed capability registry, typed envelopes, failures and model-facing schemas.

use crate::graph::{validate_graph_hash, validate_graph_request};

use crate::{
    ApplyGraphEditRequest, DatasetExported, DatasetProfileInspection, DatasetSchemaInspection,
    EditResourceRequest, ExecuteGraphRequest, ExportDatasetRequest, GraphEditReceipt,
    GraphExecution, GraphInspection, GraphResults, GraphSaved, GraphValidation,
    InspectDatasetProfileRequest, InspectDatasetSchemaRequest, InspectGraphRequest,
    InspectProjectRequest, InspectResourceRequest, InspectResultRequest, ListGraphResultsRequest,
    MAX_CATALOG_QUERY_BYTES, MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES, ManageResourceRequest,
    NodeCatalogSearchResult, ProjectInspection, ResourceInspection, ResourceMutationReceipt,
    ResultInspection, ResultValueInspection, SaveGraphRequest, SearchNodeCatalogRequest,
    ValidateGraphRequest, validate_graph_edit_operation, validate_resource_id,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityId {
    InspectResource,
    ManageResource,
    EditResource,
    ExportDataset,
    InspectUi,
    UpdateUi,
    RequestUiIntent,
    InspectGraph,
    SearchNodeCatalog,
    InspectDatasetSchema,
    InspectDatasetProfile,
    InspectResult,
    InspectProject,
    ApplyGraphEdit,
    ValidateGraph,
    ExecuteGraph,
    SaveGraph,
    ListGraphResults,
}

impl CapabilityId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectResource => "inspect_resource",
            Self::ManageResource => "manage_resource",
            Self::EditResource => "edit_resource",
            Self::ExportDataset => "export_dataset",
            Self::InspectUi => "inspect_ui",
            Self::UpdateUi => "update_ui",
            Self::RequestUiIntent => "request_ui_intent",
            Self::InspectGraph => "inspect_graph",
            Self::SearchNodeCatalog => "search_node_catalog",
            Self::InspectDatasetSchema => "inspect_dataset_schema",
            Self::InspectDatasetProfile => "inspect_dataset_profile",
            Self::InspectResult => "inspect_result",
            Self::InspectProject => "inspect_project",
            Self::ApplyGraphEdit => "apply_graph_edit",
            Self::ValidateGraph => "validate_graph",
            Self::ExecuteGraph => "execute_graph",
            Self::SaveGraph => "save_graph",
            Self::ListGraphResults => "list_graph_results",
        }
    }

    pub const fn descriptor(self) -> &'static CapabilityDescriptor {
        match self {
            Self::InspectResource => &CAPABILITY_DESCRIPTORS[14],
            Self::ManageResource => &CAPABILITY_DESCRIPTORS[15],
            Self::EditResource => &CAPABILITY_DESCRIPTORS[16],
            Self::ExportDataset => &CAPABILITY_DESCRIPTORS[17],
            Self::InspectUi => &CAPABILITY_DESCRIPTORS[11],
            Self::UpdateUi => &CAPABILITY_DESCRIPTORS[12],
            Self::RequestUiIntent => &CAPABILITY_DESCRIPTORS[13],
            Self::InspectGraph => &CAPABILITY_DESCRIPTORS[0],
            Self::SearchNodeCatalog => &CAPABILITY_DESCRIPTORS[1],
            Self::InspectDatasetSchema => &CAPABILITY_DESCRIPTORS[2],
            Self::InspectDatasetProfile => &CAPABILITY_DESCRIPTORS[3],
            Self::InspectResult => &CAPABILITY_DESCRIPTORS[4],
            Self::InspectProject => &CAPABILITY_DESCRIPTORS[5],
            Self::ApplyGraphEdit => &CAPABILITY_DESCRIPTORS[6],
            Self::ValidateGraph => &CAPABILITY_DESCRIPTORS[7],
            Self::ExecuteGraph => &CAPABILITY_DESCRIPTORS[8],
            Self::SaveGraph => &CAPABILITY_DESCRIPTORS[9],
            Self::ListGraphResults => &CAPABILITY_DESCRIPTORS[10],
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptor {
    pub id: CapabilityId,
    pub effect: ToolEffect,
    pub maximum_results: u16,
}

pub const MAX_CAPABILITY_RESULT_BYTES: usize = 1_048_576;

impl CapabilityDescriptor {
    pub const fn timeout_ms(&self) -> u64 {
        if matches!(self.effect, ToolEffect::Compute) {
            60_000
        } else {
            30_000
        }
    }
}

pub const CAPABILITY_DESCRIPTORS: [CapabilityDescriptor; 18] = [
    CapabilityDescriptor {
        id: CapabilityId::InspectGraph,
        effect: ToolEffect::Inspect,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::SearchNodeCatalog,
        effect: ToolEffect::Inspect,
        maximum_results: MAX_CATALOG_RESULTS,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatasetSchema,
        effect: ToolEffect::Inspect,
        maximum_results: 4_096,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatasetProfile,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectResult,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectProject,
        effect: ToolEffect::Inspect,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::ApplyGraphEdit,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::ValidateGraph,
        effect: ToolEffect::Inspect,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::ExecuteGraph,
        effect: ToolEffect::Compute,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::SaveGraph,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::ListGraphResults,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectUi,
        effect: ToolEffect::Inspect,
        maximum_results: 128,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateUi,
        effect: ToolEffect::Mutate,
        maximum_results: 128,
    },
    CapabilityDescriptor {
        id: CapabilityId::RequestUiIntent,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectResource,
        effect: ToolEffect::Inspect,
        maximum_results: 500,
    },
    CapabilityDescriptor {
        id: CapabilityId::ManageResource,
        effect: ToolEffect::Mutate,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::EditResource,
        effect: ToolEffect::Mutate,
        maximum_results: 500,
    },
    CapabilityDescriptor {
        id: CapabilityId::ExportDataset,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
];

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityRequest {
    InspectResource(InspectResourceRequest),
    ManageResource(ManageResourceRequest),
    EditResource(EditResourceRequest),
    ExportDataset(ExportDatasetRequest),
    InspectUi(yss_ui_contract::InspectUiRequest),
    UpdateUi(yss_ui_contract::UpdateUiRequest),
    RequestUiIntent(yss_ui_contract::RequestUiIntent),
    InspectGraph(InspectGraphRequest),
    SearchNodeCatalog(SearchNodeCatalogRequest),
    InspectDatasetSchema(InspectDatasetSchemaRequest),
    InspectDatasetProfile(InspectDatasetProfileRequest),
    InspectResult(InspectResultRequest),
    InspectProject(InspectProjectRequest),
    ApplyGraphEdit(ApplyGraphEditRequest),
    ValidateGraph(ValidateGraphRequest),
    ExecuteGraph(ExecuteGraphRequest),
    SaveGraph(SaveGraphRequest),
    ListGraphResults(ListGraphResultsRequest),
}

impl AutomationCapabilityRequest {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::InspectResource(_) => CapabilityId::InspectResource,
            Self::ManageResource(_) => CapabilityId::ManageResource,
            Self::EditResource(_) => CapabilityId::EditResource,
            Self::ExportDataset(_) => CapabilityId::ExportDataset,
            Self::InspectUi(_) => CapabilityId::InspectUi,
            Self::UpdateUi(_) => CapabilityId::UpdateUi,
            Self::RequestUiIntent(_) => CapabilityId::RequestUiIntent,
            Self::InspectGraph(_) => CapabilityId::InspectGraph,
            Self::SearchNodeCatalog(_) => CapabilityId::SearchNodeCatalog,
            Self::InspectDatasetSchema(_) => CapabilityId::InspectDatasetSchema,
            Self::InspectDatasetProfile(_) => CapabilityId::InspectDatasetProfile,
            Self::InspectResult(_) => CapabilityId::InspectResult,
            Self::InspectProject(_) => CapabilityId::InspectProject,
            Self::ApplyGraphEdit(_) => CapabilityId::ApplyGraphEdit,
            Self::ValidateGraph(_) => CapabilityId::ValidateGraph,
            Self::ExecuteGraph(_) => CapabilityId::ExecuteGraph,
            Self::SaveGraph(_) => CapabilityId::SaveGraph,
            Self::ListGraphResults(_) => CapabilityId::ListGraphResults,
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        match self {
            Self::InspectResource(request) => request.validate(),
            Self::ManageResource(request) => request.validate(),
            Self::EditResource(request) => request.validate(),
            Self::ExportDataset(request) => request.validate(),
            Self::InspectUi(request) => match request {
                yss_ui_contract::InspectUiRequest::Page { source } => source
                    .validate()
                    .map_err(|_| CapabilityContractError::InvalidField("source")),
                yss_ui_contract::InspectUiRequest::Intent { id } => validate_resource_id("id", id),
                yss_ui_contract::InspectUiRequest::Catalog => Ok(()),
            },
            Self::UpdateUi(request) => request
                .validate()
                .map_err(|_| CapabilityContractError::InvalidField("page")),
            Self::RequestUiIntent(request) => request
                .validate()
                .map_err(|_| CapabilityContractError::InvalidField("intent")),
            Self::InspectGraph(request) => validate_resource_id("graphPath", &request.graph_path),
            Self::InspectDatasetSchema(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
            Self::InspectDatasetProfile(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
            Self::InspectResult(request) => {
                validate_resource_id("executionSessionId", &request.execution_session_id)?;
                if request.limit == 0 || request.limit > 50 {
                    Err(CapabilityContractError::InvalidLimit { maximum: 50 })
                } else {
                    Ok(())
                }
            }
            Self::InspectProject(_) => Ok(()),
            Self::ValidateGraph(request) => {
                validate_graph_request(&request.graph_path, &request.graph_hash)
            }
            Self::SaveGraph(request) => {
                validate_graph_request(&request.graph_path, &request.graph_hash)
            }
            Self::ExecuteGraph(request) => {
                validate_graph_request(&request.graph_path, &request.graph_hash)
            }
            Self::ListGraphResults(request) => {
                validate_resource_id("graphPath", &request.graph_path)
            }
            Self::ApplyGraphEdit(request) => {
                validate_resource_id("graphPath", &request.graph_path)?;
                validate_graph_hash(&request.graph_hash)?;
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
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityResult {
    ResourceInspection(ResourceInspection),
    ResourceManaged(ResourceMutationReceipt),
    ResourceEdited(ResourceMutationReceipt),
    DatasetExported(DatasetExported),
    UiInspection(yss_ui_contract::UiInspection),
    UiUpdate(yss_ui_contract::UiUpdate),
    UiIntentReceipt(yss_ui_contract::UiIntentReceipt),
    GraphInspection(GraphInspection),
    NodeCatalogSearch(NodeCatalogSearchResult),
    DatasetSchemaInspection(DatasetSchemaInspection),
    DatasetProfileInspection(DatasetProfileInspection),
    ResultInspection(ResultInspection),
    ProjectInspection(ProjectInspection),
    GraphEditReceipt(GraphEditReceipt),
    GraphValidation(GraphValidation),
    GraphExecution(GraphExecution),
    GraphSaved(GraphSaved),
    GraphResults(GraphResults),
}

impl AutomationCapabilityResult {
    pub fn validate_budget(&self, maximum_bytes: usize) -> Result<(), CapabilityFailure> {
        // Result JSON is the complete shared projection. Tabular data is separately paged.
        if matches!(
            self,
            Self::ResultInspection(ResultInspection {
                value: ResultValueInspection::Json(_),
                ..
            })
        ) {
            return Ok(());
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?;
        if bytes.len() > maximum_bytes {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::ResultTooLarge,
            ));
        }
        Ok(())
    }
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::ResourceInspection(_) => CapabilityId::InspectResource,
            Self::ResourceManaged(_) => CapabilityId::ManageResource,
            Self::ResourceEdited(_) => CapabilityId::EditResource,
            Self::DatasetExported(_) => CapabilityId::ExportDataset,
            Self::GraphInspection(_) => CapabilityId::InspectGraph,
            Self::NodeCatalogSearch(_) => CapabilityId::SearchNodeCatalog,
            Self::DatasetSchemaInspection(_) => CapabilityId::InspectDatasetSchema,
            Self::DatasetProfileInspection(_) => CapabilityId::InspectDatasetProfile,
            Self::ResultInspection(_) => CapabilityId::InspectResult,
            Self::ProjectInspection(_) => CapabilityId::InspectProject,
            Self::GraphEditReceipt(_) => CapabilityId::ApplyGraphEdit,
            Self::GraphValidation(_) => CapabilityId::ValidateGraph,
            Self::GraphExecution(_) => CapabilityId::ExecuteGraph,
            Self::GraphSaved(_) => CapabilityId::SaveGraph,
            Self::GraphResults(_) => CapabilityId::ListGraphResults,
            Self::UiInspection(_) => CapabilityId::InspectUi,
            Self::UiUpdate(_) => CapabilityId::UpdateUi,
            Self::UiIntentReceipt(_) => CapabilityId::RequestUiIntent,
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
    #[error("resource_unavailable")]
    ResourceUnavailable,
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
    #[error("graph_draft_changed")]
    GraphDraftChanged,
    #[error("outcome_unknown")]
    OutcomeUnknown,
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

pub fn capability_input_schema(capability_id: CapabilityId) -> schemars::Schema {
    let mut schema = match capability_id {
        CapabilityId::InspectResource => schemars::schema_for!(InspectResourceRequest),
        CapabilityId::ManageResource => schemars::schema_for!(ManageResourceRequest),
        CapabilityId::EditResource => schemars::schema_for!(EditResourceRequest),
        CapabilityId::ExportDataset => schemars::schema_for!(ExportDatasetRequest),
        CapabilityId::InspectUi => schemars::schema_for!(yss_ui_contract::InspectUiRequest),
        CapabilityId::UpdateUi => schemars::schema_for!(yss_ui_contract::UpdateUiRequest),
        CapabilityId::RequestUiIntent => schemars::schema_for!(yss_ui_contract::RequestUiIntent),
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
        CapabilityId::ValidateGraph => schemars::schema_for!(ValidateGraphRequest),
        CapabilityId::ExecuteGraph => schemars::schema_for!(ExecuteGraphRequest),
        CapabilityId::SaveGraph => schemars::schema_for!(SaveGraphRequest),
        CapabilityId::ListGraphResults => schemars::schema_for!(ListGraphResultsRequest),
    };
    // All capability arguments are objects, including tagged enums whose schemas
    // otherwise express this constraint only inside oneOf branches.
    schema.insert("type".into(), serde_json::json!("object"));
    schema
}
