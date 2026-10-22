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
    ChartInspect(InspectChartInput),
    ChartUpdate(UpdateChartInput),
    ProjectInspectResource(InspectResourceInput),
    ProjectCreateResource(CreateResourceInput),
    DataInspect(InspectDatabaseInput),
    DataInspectSchema(InspectDatabaseSchemaInput),
    DataInspectProfile(ProfileDatabaseInput),
    DataReadRows(ReadDatabaseRowsInput),
    DocumentInspect(InspectDocumentInput),
    DocumentRead(ReadDocumentInput),
    DocumentSearch(SearchDocumentInput),
    DocumentReplaceText(ReplaceDocumentTextInput),
    DocumentAppendText(AppendDocumentInput),
    DocumentWriteText(WriteDocumentInput),

    MindInspect(InspectMindInput),
    MindSearchTopics(FindTopicsInput),
    MindInspectTopics(InspectTopicsInput),
    MindCreateTopics(CreateTopicsInput),
    MindUpdateTopics(UpdateTopicsInput),
    MindMoveTopics(MoveTopicsInput),
    MindDeleteTopics(DeleteTopicsInput),
    MindDuplicateTopics(DuplicateTopicsInput),

    DataImport(ImportDatabaseInput),
    DataCreateRows(InsertRowsInput),
    DataUpdateCells(UpdateCellsInput),
    DataDeleteRows(DeleteRowsInput),
    DataCreateColumns(CreateColumnsInput),
    DataRenameColumns(RenameColumnsInput),
    DataDeleteColumns(DeleteColumnsInput),
    DataCastColumns(CastColumnsInput),
    DataUpdateColumnSemantics(SetColumnSemanticsInput),
    ProjectRenameResource(RenameResourceInput),
    ProjectDuplicateResource(DuplicateResourceInput),
    ProjectDeleteResource(ResourceTargetInput),
    ProjectSaveResource(ResourceTargetInput),
    ProjectUndoResource(ResourceTargetInput),
    ProjectRedoResource(ResourceTargetInput),
    GraphUpdateFunctionSignature(EditResourceInput),
    DataExport(ExportDatabaseInput),
    UiInspectReceipt(yss_ui_contract::InspectUiIntentRequest),
    UiOpen(RequestUiIntentInput),
    GraphInspect(InspectGraphInput),
    CatalogSearchNodes(BrowseNodesRequest),
    CatalogInspectNodes(InspectNodeTypeRequest),
    GraphSearchNodes(FindNodesRequest),
    GraphSearchConstants(FindConstantsRequest),
    GraphInspectConstants(InspectConstantsRequest),
    GraphCreateConstants(CreateConstantsInput),
    GraphUpdateConstants(UpdateConstantsInput),
    GraphDeleteConstants(DeleteConstantsInput),

    GraphInspectNodes(InspectNodesRequest),
    GraphSearchConnections(FindConnectionsRequest),
    ResultInspect(InspectResultRequest),
    ResultReadTable(ReadResultTableRequest),
    ProjectListResources(ListResourcesRequest),
    #[serde(skip_deserializing)]
    GraphApplyEdit(ApplyGraphEditInput),
    GraphCreateNodes(CreateNodesInput),
    GraphUpdateNodes(UpdateNodesInput),
    GraphDeleteNodes(DeleteNodesInput),
    GraphDuplicateNodes(DuplicateNodesInput),
    GraphMoveNodes(MoveNodesInput),
    GraphCreateConnections(CreateConnectionsInput),
    GraphUpdateConnections(UpdateConnectionsInput),
    GraphDeleteConnections(DeleteConnectionsInput),
    GraphValidate(ValidateGraphInput),
    GraphExecute(ExecuteGraphInput),
    #[serde(skip_deserializing)]
    GraphSave(GraphTargetInput),
    ResultList(ListGraphResultsRequest),
}

impl CapabilityInput {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::ChartInspect(_) => CapabilityId::ChartInspect,
            Self::ChartUpdate(_) => CapabilityId::ChartUpdate,
            Self::ProjectInspectResource(_) => CapabilityId::ProjectInspectResource,
            Self::ProjectCreateResource(_) => CapabilityId::ProjectCreateResource,
            Self::DataInspect(_) => CapabilityId::DataInspect,
            Self::DataInspectSchema(_) => CapabilityId::DataInspectSchema,
            Self::DataInspectProfile(_) => CapabilityId::DataInspectProfile,
            Self::DataReadRows(_) => CapabilityId::DataReadRows,
            Self::DocumentInspect(_) => CapabilityId::DocumentInspect,
            Self::DocumentRead(_) => CapabilityId::DocumentRead,
            Self::DocumentSearch(_) => CapabilityId::DocumentSearch,
            Self::DocumentReplaceText(_) => CapabilityId::DocumentReplaceText,
            Self::DocumentAppendText(_) => CapabilityId::DocumentAppendText,
            Self::DocumentWriteText(_) => CapabilityId::DocumentWriteText,

            Self::MindInspect(_) => CapabilityId::MindInspect,
            Self::MindSearchTopics(_) => CapabilityId::MindSearchTopics,
            Self::MindInspectTopics(_) => CapabilityId::MindInspectTopics,
            Self::MindCreateTopics(_) => CapabilityId::MindCreateTopics,
            Self::MindUpdateTopics(_) => CapabilityId::MindUpdateTopics,
            Self::MindMoveTopics(_) => CapabilityId::MindMoveTopics,
            Self::MindDeleteTopics(_) => CapabilityId::MindDeleteTopics,
            Self::MindDuplicateTopics(_) => CapabilityId::MindDuplicateTopics,

            Self::DataImport(_) => CapabilityId::DataImport,
            Self::DataCreateRows(_) => CapabilityId::DataCreateRows,
            Self::DataUpdateCells(_) => CapabilityId::DataUpdateCells,
            Self::DataDeleteRows(_) => CapabilityId::DataDeleteRows,
            Self::DataCreateColumns(_) => CapabilityId::DataCreateColumns,
            Self::DataRenameColumns(_) => CapabilityId::DataRenameColumns,
            Self::DataDeleteColumns(_) => CapabilityId::DataDeleteColumns,
            Self::DataCastColumns(_) => CapabilityId::DataCastColumns,
            Self::DataUpdateColumnSemantics(_) => CapabilityId::DataUpdateColumnSemantics,
            Self::ProjectRenameResource(_) => CapabilityId::ProjectRenameResource,
            Self::ProjectDuplicateResource(_) => CapabilityId::ProjectDuplicateResource,
            Self::ProjectDeleteResource(_) => CapabilityId::ProjectDeleteResource,
            Self::ProjectSaveResource(_) => CapabilityId::ProjectSaveResource,
            Self::ProjectUndoResource(_) => CapabilityId::ProjectUndoResource,
            Self::ProjectRedoResource(_) => CapabilityId::ProjectRedoResource,
            Self::GraphUpdateFunctionSignature(_) => CapabilityId::GraphUpdateFunctionSignature,
            Self::DataExport(_) => CapabilityId::DataExport,
            Self::UiInspectReceipt(_) => CapabilityId::UiInspectReceipt,
            Self::UiOpen(_) => CapabilityId::UiOpen,
            Self::GraphInspect(_) => CapabilityId::GraphInspect,
            Self::CatalogSearchNodes(_) => CapabilityId::CatalogSearchNodes,
            Self::CatalogInspectNodes(_) => CapabilityId::CatalogInspectNodes,
            Self::GraphSearchNodes(_) => CapabilityId::GraphSearchNodes,
            Self::GraphSearchConstants(_) => CapabilityId::GraphSearchConstants,
            Self::GraphInspectConstants(_) => CapabilityId::GraphInspectConstants,
            Self::GraphCreateConstants(_) => CapabilityId::GraphCreateConstants,
            Self::GraphUpdateConstants(_) => CapabilityId::GraphUpdateConstants,
            Self::GraphDeleteConstants(_) => CapabilityId::GraphDeleteConstants,

            Self::GraphInspectNodes(_) => CapabilityId::GraphInspectNodes,
            Self::GraphSearchConnections(_) => CapabilityId::GraphSearchConnections,
            Self::ResultInspect(_) => CapabilityId::ResultInspect,
            Self::ResultReadTable(_) => CapabilityId::ResultReadTable,
            Self::ProjectListResources(_) => CapabilityId::ProjectListResources,
            Self::GraphApplyEdit(_) => CapabilityId::GraphApplyEdit,
            Self::GraphCreateNodes(_) => CapabilityId::GraphCreateNodes,
            Self::GraphUpdateNodes(_) => CapabilityId::GraphUpdateNodes,
            Self::GraphDeleteNodes(_) => CapabilityId::GraphDeleteNodes,
            Self::GraphDuplicateNodes(_) => CapabilityId::GraphDuplicateNodes,
            Self::GraphMoveNodes(_) => CapabilityId::GraphMoveNodes,
            Self::GraphCreateConnections(_) => CapabilityId::GraphCreateConnections,
            Self::GraphUpdateConnections(_) => CapabilityId::GraphUpdateConnections,
            Self::GraphDeleteConnections(_) => CapabilityId::GraphDeleteConnections,
            Self::GraphValidate(_) => CapabilityId::GraphValidate,
            Self::GraphExecute(_) => CapabilityId::GraphExecute,
            Self::GraphSave(_) => CapabilityId::GraphSave,
            Self::ResultList(_) => CapabilityId::ResultList,
        }
    }
}

impl From<&AutomationCapabilityRequest> for CapabilityInput {
    fn from(value: &AutomationCapabilityRequest) -> Self {
        match value {
            AutomationCapabilityRequest::InspectChart(value) => {
                Self::ChartInspect(InspectChartInput {
                    chart: value.chart.clone(),
                })
            }
            AutomationCapabilityRequest::InspectResource(value) => {
                Self::ProjectInspectResource(value.into())
            }
            AutomationCapabilityRequest::ReadDocument(value) => match &value.input {
                DocumentReadInput::Outline(value) => Self::DocumentInspect(value.clone()),
                DocumentReadInput::Text(value) => Self::DocumentRead(value.clone()),
                DocumentReadInput::Search(value) => Self::DocumentSearch(value.clone()),
            },
            AutomationCapabilityRequest::ReadMind(value) => match &value.input {
                MindReadInput::Outline(value) => Self::MindInspect(value.clone()),
                MindReadInput::Find(value) => Self::MindSearchTopics(value.clone()),
                MindReadInput::Topics(value) => Self::MindInspectTopics(value.clone()),
            },
            AutomationCapabilityRequest::ReadDatabase(value) => match &value.input {
                DatabaseReadInput::Overview(value) => Self::DataInspect(value.clone()),
                DatabaseReadInput::Schema(value) => Self::DataInspectSchema(value.clone()),
                DatabaseReadInput::Profile(value) => Self::DataInspectProfile(value.clone()),
                DatabaseReadInput::Rows(value) => Self::DataReadRows(value.clone()),
            },
            AutomationCapabilityRequest::ManageResource(value) => match value {
                ManageResourceRequest::Create { specification } => match specification {
                    ResourceCreation::EventGraph { name } => {
                        Self::ProjectCreateResource(CreateResourceInput::EventGraph {
                            name: name.clone(),
                        })
                    }
                    ResourceCreation::FunctionGraph { name } => {
                        Self::ProjectCreateResource(CreateResourceInput::FunctionGraph {
                            name: name.clone(),
                        })
                    }
                    ResourceCreation::Chart { name } => {
                        Self::ProjectCreateResource(CreateResourceInput::Chart {
                            name: name.clone(),
                        })
                    }
                    ResourceCreation::Mind { name } => {
                        Self::ProjectCreateResource(CreateResourceInput::Mind {
                            name: name.clone(),
                        })
                    }
                    ResourceCreation::Doc { name } => {
                        Self::ProjectCreateResource(CreateResourceInput::Doc { name: name.clone() })
                    }
                    ResourceCreation::Database { source, name } => {
                        Self::DataImport(ImportDatabaseInput {
                            source: source.clone(),
                            name: name.clone(),
                        })
                    }
                },
                ManageResourceRequest::Rename { resource, name, .. } => {
                    Self::ProjectRenameResource(RenameResourceInput {
                        resource: resource.clone(),
                        name: name.clone(),
                    })
                }
                ManageResourceRequest::Duplicate { resource, name, .. } => {
                    Self::ProjectDuplicateResource(DuplicateResourceInput {
                        resource: resource.clone(),
                        name: name.clone(),
                    })
                }
                ManageResourceRequest::Delete { resource, .. } => {
                    Self::ProjectDeleteResource(ResourceTargetInput {
                        resource: resource.clone(),
                    })
                }
                ManageResourceRequest::Save { resource, .. } => {
                    Self::ProjectSaveResource(ResourceTargetInput {
                        resource: resource.clone(),
                    })
                }
            },
            AutomationCapabilityRequest::EditResource(value) => value.into(),
            AutomationCapabilityRequest::ExportDatabase(value) => {
                Self::DataExport(ExportDatabaseInput {
                    database: DatabaseResourceRef::new(value.resource.id.clone()),
                    path: value.path.clone(),
                    format: value.format.clone(),
                })
            }
            AutomationCapabilityRequest::InspectUiIntent(value) => {
                Self::UiInspectReceipt(value.clone())
            }
            AutomationCapabilityRequest::RequestUiIntent(value) => {
                Self::UiOpen(value.input.clone())
            }
            AutomationCapabilityRequest::InspectGraph(value) => Self::GraphInspect(value.into()),
            AutomationCapabilityRequest::BrowseNodes(value) => {
                Self::CatalogSearchNodes(value.clone())
            }
            AutomationCapabilityRequest::InspectNodeType(value) => {
                Self::CatalogInspectNodes(value.clone())
            }
            AutomationCapabilityRequest::FindNodes(value) => Self::GraphSearchNodes(value.clone()),
            AutomationCapabilityRequest::FindConstants(value) => {
                Self::GraphSearchConstants(value.clone())
            }
            AutomationCapabilityRequest::InspectConstants(value) => {
                Self::GraphInspectConstants(value.clone())
            }

            AutomationCapabilityRequest::InspectNodes(value) => {
                Self::GraphInspectNodes(value.clone())
            }
            AutomationCapabilityRequest::FindConnections(value) => {
                Self::GraphSearchConnections(value.clone())
            }
            AutomationCapabilityRequest::InspectResult(value) => Self::ResultInspect(value.clone()),
            AutomationCapabilityRequest::ReadResultTable(value) => {
                Self::ResultReadTable(value.clone())
            }
            AutomationCapabilityRequest::ListResources(value) => {
                Self::ProjectListResources(value.clone())
            }
            AutomationCapabilityRequest::GraphMutation(value) => value.input.clone().into(),
            AutomationCapabilityRequest::ApplyGraphEdit(value) => {
                Self::GraphApplyEdit(ApplyGraphEditInput {
                    graph_path: value.graph_path.clone(),
                    locale: value.locale.clone(),
                    operations: value.operations.clone(),
                })
            }
            AutomationCapabilityRequest::ValidateGraph(value) => {
                Self::GraphValidate(ValidateGraphInput {
                    graph: value.graph.clone(),
                    node_ids: value.node_ids.clone(),
                    offset: value.offset,
                    limit: value.limit,
                })
            }
            AutomationCapabilityRequest::ExecuteGraph(value) => {
                Self::GraphExecute(ExecuteGraphInput {
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
            AutomationCapabilityRequest::SaveGraph(value) => Self::GraphSave(GraphTargetInput {
                graph_path: value.graph_path.clone(),
            }),
            AutomationCapabilityRequest::ListGraphResults(value) => Self::ResultList(value.clone()),
        }
    }
}

impl From<AutomationCapabilityRequest> for CapabilityInput {
    fn from(value: AutomationCapabilityRequest) -> Self {
        Self::from(&value)
    }
}
