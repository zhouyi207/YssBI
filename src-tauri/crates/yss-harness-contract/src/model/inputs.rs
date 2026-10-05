//! Public capability arguments shared by schemas, live calls and history replay.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CapabilityInput {
    InspectResource(InspectResourceRequest),
    ManageResource(ManageResourceInput),
    EditResource(EditResourceInput),
    ExportDataset(ExportDatasetInput),
    InspectUiIntent(yss_ui_contract::InspectUiIntentRequest),
    RequestUiIntent(RequestUiIntentInput),
    InspectGraph(InspectGraphInput),
    SearchNodeCatalog(SearchNodeCatalogRequest),
    SearchKnowledge(SearchKnowledgeRequest),
    ReadKnowledge(ReadKnowledgeRequest),
    InspectDatasetSchema(InspectDatasetSchemaRequest),
    InspectDatasetProfile(InspectDatasetProfileRequest),
    InspectResult(InspectResultRequest),
    InspectProject(InspectProjectRequest),
    ApplyGraphEdit(ApplyGraphEditInput),
    ValidateGraph(GraphTargetInput),
    ExecuteGraph(ExecuteGraphInput),
    SaveGraph(GraphTargetInput),
    ListGraphResults(ListGraphResultsRequest),
}

impl CapabilityInput {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::InspectResource(_) => CapabilityId::InspectResource,
            Self::ManageResource(_) => CapabilityId::ManageResource,
            Self::EditResource(_) => CapabilityId::EditResource,
            Self::ExportDataset(_) => CapabilityId::ExportDataset,
            Self::InspectUiIntent(_) => CapabilityId::InspectUiIntent,
            Self::RequestUiIntent(_) => CapabilityId::RequestUiIntent,
            Self::InspectGraph(_) => CapabilityId::InspectGraph,
            Self::SearchNodeCatalog(_) => CapabilityId::SearchNodeCatalog,
            Self::SearchKnowledge(_) => CapabilityId::SearchKnowledge,
            Self::ReadKnowledge(_) => CapabilityId::ReadKnowledge,
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
}

impl From<&AutomationCapabilityRequest> for CapabilityInput {
    fn from(value: &AutomationCapabilityRequest) -> Self {
        match value {
            AutomationCapabilityRequest::InspectResource(value) => {
                Self::InspectResource(value.clone())
            }
            AutomationCapabilityRequest::ManageResource(value) => {
                Self::ManageResource(value.into())
            }
            AutomationCapabilityRequest::EditResource(value) => {
                Self::EditResource(EditResourceInput {
                    resource: value.resource.clone(),
                    edit: (&value.edit).into(),
                })
            }
            AutomationCapabilityRequest::ExportDataset(value) => {
                Self::ExportDataset(ExportDatasetInput {
                    resource: value.resource.clone(),
                    path: value.path.clone(),
                    format: value.format.clone(),
                })
            }
            AutomationCapabilityRequest::InspectUiIntent(value) => {
                Self::InspectUiIntent(value.clone())
            }
            AutomationCapabilityRequest::RequestUiIntent(value) => {
                Self::RequestUiIntent(RequestUiIntentInput {
                    intent: value.intent.clone(),
                })
            }
            AutomationCapabilityRequest::InspectGraph(value) => Self::InspectGraph(value.into()),
            AutomationCapabilityRequest::SearchNodeCatalog(value) => {
                Self::SearchNodeCatalog(value.clone())
            }
            AutomationCapabilityRequest::SearchKnowledge(value) => {
                Self::SearchKnowledge(value.clone())
            }
            AutomationCapabilityRequest::ReadKnowledge(value) => Self::ReadKnowledge(value.clone()),
            AutomationCapabilityRequest::InspectDatasetSchema(value) => {
                Self::InspectDatasetSchema(value.clone())
            }
            AutomationCapabilityRequest::InspectDatasetProfile(value) => {
                Self::InspectDatasetProfile(value.clone())
            }
            AutomationCapabilityRequest::InspectResult(value) => Self::InspectResult(value.clone()),
            AutomationCapabilityRequest::InspectProject(value) => {
                Self::InspectProject(value.clone())
            }
            AutomationCapabilityRequest::ApplyGraphEdit(value) => {
                Self::ApplyGraphEdit(ApplyGraphEditInput {
                    graph_path: value.graph_path.clone(),
                    locale: value.locale.clone(),
                    operations: value.operations.clone(),
                })
            }
            AutomationCapabilityRequest::ValidateGraph(value) => {
                Self::ValidateGraph(GraphTargetInput {
                    graph_path: value.graph_path.clone(),
                })
            }
            AutomationCapabilityRequest::ExecuteGraph(value) => {
                Self::ExecuteGraph(ExecuteGraphInput {
                    graph_path: value.graph_path.clone(),
                    demand: value.demand.clone(),
                })
            }
            AutomationCapabilityRequest::SaveGraph(value) => Self::SaveGraph(GraphTargetInput {
                graph_path: value.graph_path.clone(),
            }),
            AutomationCapabilityRequest::ListGraphResults(value) => {
                Self::ListGraphResults(value.clone())
            }
        }
    }
}

impl From<AutomationCapabilityRequest> for CapabilityInput {
    fn from(value: AutomationCapabilityRequest) -> Self {
        Self::from(&value)
    }
}
