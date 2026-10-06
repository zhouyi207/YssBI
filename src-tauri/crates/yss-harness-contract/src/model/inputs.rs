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
    InspectChart(InspectChartInput),
    UpdateChart(UpdateChartInput),
    InspectResource(InspectResourceInput),
    CreateResource(CreateResourceInput),
    InspectDatabase(InspectDatabaseInput),
    InspectDatabaseSchema(InspectDatabaseSchemaInput),
    ProfileDatabase(ProfileDatabaseInput),
    ReadDatabaseRows(ReadDatabaseRowsInput),
    InspectDocument(InspectDocumentInput),
    ReadDocument(ReadDocumentInput),
    SearchDocument(SearchDocumentInput),
    ReplaceDocumentText(ReplaceDocumentTextInput),
    AppendDocument(AppendDocumentInput),
    WriteDocument(WriteDocumentInput),

    InspectMind(InspectMindInput),
    FindTopics(FindTopicsInput),
    InspectTopics(InspectTopicsInput),
    CreateTopics(CreateTopicsInput),
    UpdateTopics(UpdateTopicsInput),
    MoveTopics(MoveTopicsInput),
    DeleteTopics(DeleteTopicsInput),
    DuplicateTopics(DuplicateTopicsInput),

    ImportDatabase(ImportDatabaseInput),
    InsertRows(InsertRowsInput),
    UpdateCells(UpdateCellsInput),
    DeleteRows(DeleteRowsInput),
    CreateColumns(CreateColumnsInput),
    RenameColumns(RenameColumnsInput),
    DeleteColumns(DeleteColumnsInput),
    CastColumns(CastColumnsInput),
    SetColumnSemantics(SetColumnSemanticsInput),
    RenameResource(RenameResourceInput),
    DuplicateResource(DuplicateResourceInput),
    DeleteResource(ResourceTargetInput),
    SaveResource(ResourceTargetInput),
    UndoResource(ResourceTargetInput),
    RedoResource(ResourceTargetInput),
    EditResource(EditResourceInput),
    ExportDatabase(ExportDatabaseInput),
    InspectUiIntent(yss_ui_contract::InspectUiIntentRequest),
    RequestUiIntent(RequestUiIntentInput),
    InspectGraph(InspectGraphInput),
    BrowseNodes(BrowseNodesRequest),
    InspectNodeType(InspectNodeTypeRequest),
    FindNodes(FindNodesRequest),
    FindConstants(FindConstantsRequest),
    InspectConstants(InspectConstantsRequest),
    CreateConstants(CreateConstantsInput),
    UpdateConstants(UpdateConstantsInput),
    DeleteConstants(DeleteConstantsInput),

    InspectNodes(InspectNodesRequest),
    FindConnections(FindConnectionsRequest),
    SearchKnowledge(SearchKnowledgeRequest),
    ReadKnowledge(ReadKnowledgeRequest),
    #[serde(skip_deserializing)]
    InspectDatasetSchema(InspectDatasetSchemaRequest),
    #[serde(skip_deserializing)]
    InspectDatasetProfile(InspectDatasetProfileRequest),
    InspectResult(InspectResultRequest),
    ReadResultTable(ReadResultTableRequest),
    ListResources(ListResourcesRequest),
    #[serde(skip_deserializing)]
    ApplyGraphEdit(ApplyGraphEditInput),
    CreateNodes(CreateNodesInput),
    UpdateNodes(UpdateNodesInput),
    DeleteNodes(DeleteNodesInput),
    DuplicateNodes(DuplicateNodesInput),
    MoveNodes(MoveNodesInput),
    CreateConnections(CreateConnectionsInput),
    UpdateConnections(UpdateConnectionsInput),
    DeleteConnections(DeleteConnectionsInput),
    ValidateGraph(ValidateGraphInput),
    ExecuteGraph(ExecuteGraphInput),
    #[serde(skip_deserializing)]
    SaveGraph(GraphTargetInput),
    ListGraphResults(ListGraphResultsRequest),
}

impl CapabilityInput {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::InspectChart(_) => CapabilityId::InspectChart,
            Self::UpdateChart(_) => CapabilityId::UpdateChart,
            Self::InspectResource(_) => CapabilityId::InspectResource,
            Self::CreateResource(_) => CapabilityId::CreateResource,
            Self::InspectDatabase(_) => CapabilityId::InspectDatabase,
            Self::InspectDatabaseSchema(_) => CapabilityId::InspectDatabaseSchema,
            Self::ProfileDatabase(_) => CapabilityId::ProfileDatabase,
            Self::ReadDatabaseRows(_) => CapabilityId::ReadDatabaseRows,
            Self::InspectDocument(_) => CapabilityId::InspectDocument,
            Self::ReadDocument(_) => CapabilityId::ReadDocument,
            Self::SearchDocument(_) => CapabilityId::SearchDocument,
            Self::ReplaceDocumentText(_) => CapabilityId::ReplaceDocumentText,
            Self::AppendDocument(_) => CapabilityId::AppendDocument,
            Self::WriteDocument(_) => CapabilityId::WriteDocument,

            Self::InspectMind(_) => CapabilityId::InspectMind,
            Self::FindTopics(_) => CapabilityId::FindTopics,
            Self::InspectTopics(_) => CapabilityId::InspectTopics,
            Self::CreateTopics(_) => CapabilityId::CreateTopics,
            Self::UpdateTopics(_) => CapabilityId::UpdateTopics,
            Self::MoveTopics(_) => CapabilityId::MoveTopics,
            Self::DeleteTopics(_) => CapabilityId::DeleteTopics,
            Self::DuplicateTopics(_) => CapabilityId::DuplicateTopics,

            Self::ImportDatabase(_) => CapabilityId::ImportDatabase,
            Self::InsertRows(_) => CapabilityId::InsertRows,
            Self::UpdateCells(_) => CapabilityId::UpdateCells,
            Self::DeleteRows(_) => CapabilityId::DeleteRows,
            Self::CreateColumns(_) => CapabilityId::CreateColumns,
            Self::RenameColumns(_) => CapabilityId::RenameColumns,
            Self::DeleteColumns(_) => CapabilityId::DeleteColumns,
            Self::CastColumns(_) => CapabilityId::CastColumns,
            Self::SetColumnSemantics(_) => CapabilityId::SetColumnSemantics,
            Self::RenameResource(_) => CapabilityId::RenameResource,
            Self::DuplicateResource(_) => CapabilityId::DuplicateResource,
            Self::DeleteResource(_) => CapabilityId::DeleteResource,
            Self::SaveResource(_) => CapabilityId::SaveResource,
            Self::UndoResource(_) => CapabilityId::UndoResource,
            Self::RedoResource(_) => CapabilityId::RedoResource,
            Self::EditResource(_) => CapabilityId::EditResource,
            Self::ExportDatabase(_) => CapabilityId::ExportDatabase,
            Self::InspectUiIntent(_) => CapabilityId::InspectUiIntent,
            Self::RequestUiIntent(_) => CapabilityId::RequestUiIntent,
            Self::InspectGraph(_) => CapabilityId::InspectGraph,
            Self::BrowseNodes(_) => CapabilityId::BrowseNodes,
            Self::InspectNodeType(_) => CapabilityId::InspectNodeType,
            Self::FindNodes(_) => CapabilityId::FindNodes,
            Self::FindConstants(_) => CapabilityId::FindConstants,
            Self::InspectConstants(_) => CapabilityId::InspectConstants,
            Self::CreateConstants(_) => CapabilityId::CreateConstants,
            Self::UpdateConstants(_) => CapabilityId::UpdateConstants,
            Self::DeleteConstants(_) => CapabilityId::DeleteConstants,

            Self::InspectNodes(_) => CapabilityId::InspectNodes,
            Self::FindConnections(_) => CapabilityId::FindConnections,
            Self::SearchKnowledge(_) => CapabilityId::SearchKnowledge,
            Self::ReadKnowledge(_) => CapabilityId::ReadKnowledge,
            Self::InspectDatasetSchema(_) => CapabilityId::InspectDatasetSchema,
            Self::InspectDatasetProfile(_) => CapabilityId::InspectDatasetProfile,
            Self::InspectResult(_) => CapabilityId::InspectResult,
            Self::ReadResultTable(_) => CapabilityId::ReadResultTable,
            Self::ListResources(_) => CapabilityId::ListResources,
            Self::ApplyGraphEdit(_) => CapabilityId::ApplyGraphEdit,
            Self::CreateNodes(_) => CapabilityId::CreateNodes,
            Self::UpdateNodes(_) => CapabilityId::UpdateNodes,
            Self::DeleteNodes(_) => CapabilityId::DeleteNodes,
            Self::DuplicateNodes(_) => CapabilityId::DuplicateNodes,
            Self::MoveNodes(_) => CapabilityId::MoveNodes,
            Self::CreateConnections(_) => CapabilityId::CreateConnections,
            Self::UpdateConnections(_) => CapabilityId::UpdateConnections,
            Self::DeleteConnections(_) => CapabilityId::DeleteConnections,
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
            AutomationCapabilityRequest::InspectChart(value) => {
                Self::InspectChart(InspectChartInput {
                    chart: value.chart.clone(),
                })
            }
            AutomationCapabilityRequest::InspectResource(value) => {
                Self::InspectResource(value.into())
            }
            AutomationCapabilityRequest::ReadDocument(value) => match &value.input {
                DocumentReadInput::Outline(value) => Self::InspectDocument(value.clone()),
                DocumentReadInput::Text(value) => Self::ReadDocument(value.clone()),
                DocumentReadInput::Search(value) => Self::SearchDocument(value.clone()),
            },
            AutomationCapabilityRequest::ReadMind(value) => match &value.input {
                MindReadInput::Outline(value) => Self::InspectMind(value.clone()),
                MindReadInput::Find(value) => Self::FindTopics(value.clone()),
                MindReadInput::Topics(value) => Self::InspectTopics(value.clone()),
            },
            AutomationCapabilityRequest::ReadDatabase(value) => match &value.input {
                DatabaseReadInput::Overview(value) => Self::InspectDatabase(value.clone()),
                DatabaseReadInput::Schema(value) => Self::InspectDatabaseSchema(value.clone()),
                DatabaseReadInput::Profile(value) => Self::ProfileDatabase(value.clone()),
                DatabaseReadInput::Rows(value) => Self::ReadDatabaseRows(value.clone()),
            },
            AutomationCapabilityRequest::ManageResource(value) => match value {
                ManageResourceRequest::Create { specification } => match specification {
                    ResourceCreation::EventGraph { name } => {
                        Self::CreateResource(CreateResourceInput::EventGraph { name: name.clone() })
                    }
                    ResourceCreation::FunctionGraph { name } => {
                        Self::CreateResource(CreateResourceInput::FunctionGraph {
                            name: name.clone(),
                        })
                    }
                    ResourceCreation::Chart { name } => {
                        Self::CreateResource(CreateResourceInput::Chart { name: name.clone() })
                    }
                    ResourceCreation::Mind { name } => {
                        Self::CreateResource(CreateResourceInput::Mind { name: name.clone() })
                    }
                    ResourceCreation::Doc { name } => {
                        Self::CreateResource(CreateResourceInput::Doc { name: name.clone() })
                    }
                    ResourceCreation::Database { source, name } => {
                        Self::ImportDatabase(ImportDatabaseInput {
                            source: source.clone(),
                            name: name.clone(),
                        })
                    }
                },
                ManageResourceRequest::Rename { resource, name, .. } => {
                    Self::RenameResource(RenameResourceInput {
                        resource: resource.clone(),
                        name: name.clone(),
                    })
                }
                ManageResourceRequest::Duplicate { resource, name, .. } => {
                    Self::DuplicateResource(DuplicateResourceInput {
                        resource: resource.clone(),
                        name: name.clone(),
                    })
                }
                ManageResourceRequest::Delete { resource, .. } => {
                    Self::DeleteResource(ResourceTargetInput {
                        resource: resource.clone(),
                    })
                }
                ManageResourceRequest::Save { resource, .. } => {
                    Self::SaveResource(ResourceTargetInput {
                        resource: resource.clone(),
                    })
                }
            },
            AutomationCapabilityRequest::EditResource(value) => value.into(),
            AutomationCapabilityRequest::ExportDatabase(value) => {
                Self::ExportDatabase(ExportDatabaseInput {
                    database: DatabaseResourceRef::new(value.resource.id.clone()),
                    path: value.path.clone(),
                    format: value.format.clone(),
                })
            }
            AutomationCapabilityRequest::InspectUiIntent(value) => {
                Self::InspectUiIntent(value.clone())
            }
            AutomationCapabilityRequest::RequestUiIntent(value) => {
                Self::RequestUiIntent(value.input.clone())
            }
            AutomationCapabilityRequest::InspectGraph(value) => Self::InspectGraph(value.into()),
            AutomationCapabilityRequest::BrowseNodes(value) => Self::BrowseNodes(value.clone()),
            AutomationCapabilityRequest::InspectNodeType(value) => {
                Self::InspectNodeType(value.clone())
            }
            AutomationCapabilityRequest::FindNodes(value) => Self::FindNodes(value.clone()),
            AutomationCapabilityRequest::FindConstants(value) => Self::FindConstants(value.clone()),
            AutomationCapabilityRequest::InspectConstants(value) => {
                Self::InspectConstants(value.clone())
            }

            AutomationCapabilityRequest::InspectNodes(value) => Self::InspectNodes(value.clone()),
            AutomationCapabilityRequest::FindConnections(value) => {
                Self::FindConnections(value.clone())
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
            AutomationCapabilityRequest::ReadResultTable(value) => {
                Self::ReadResultTable(value.clone())
            }
            AutomationCapabilityRequest::ListResources(value) => Self::ListResources(value.clone()),
            AutomationCapabilityRequest::GraphMutation(value) => value.input.clone().into(),
            AutomationCapabilityRequest::ApplyGraphEdit(value) => {
                Self::ApplyGraphEdit(ApplyGraphEditInput {
                    graph_path: value.graph_path.clone(),
                    locale: value.locale.clone(),
                    operations: value.operations.clone(),
                })
            }
            AutomationCapabilityRequest::ValidateGraph(value) => {
                Self::ValidateGraph(ValidateGraphInput {
                    graph: value.graph.clone(),
                    node_ids: value.node_ids.clone(),
                    offset: value.offset,
                    limit: value.limit,
                })
            }
            AutomationCapabilityRequest::ExecuteGraph(value) => {
                Self::ExecuteGraph(ExecuteGraphInput {
                    graph: value.graph.clone(),
                    node_id: match &value.demand {
                        GraphExecutionDemand::Default => None,
                        GraphExecutionDemand::Node { node_id, .. } => Some(node_id.clone()),
                    },
                    mode: match &value.demand {
                        GraphExecutionDemand::Default => None,
                        GraphExecutionDemand::Node { mode, .. } => Some(mode.clone()),
                    },
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
