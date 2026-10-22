//! Closed YssBI capability registry, typed envelopes, business failure codes and model-facing schemas.

use crate::graph::{validate_graph_hash, validate_graph_request};

use crate::{
    ApplyGraphEditRequest, BrowseNodesRequest, CapabilityContractError, CapabilityFailure,
    CapabilityFailureCode, DatabaseExported, EditResourceRequest, ExecuteGraphRequest,
    ExportDatabaseRequest, GraphEditReceipt, GraphExecution, GraphInspection, GraphInspectionPage,
    GraphResults, GraphSaved, GraphValidation, InspectGraphRequest, InspectResourceRequest,
    InspectResultRequest, ListGraphResultsRequest, ListResourcesRequest, MAX_CATALOG_QUERY_BYTES,
    MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES, ManageResourceRequest, NodeCatalogPage,
    ProjectInspection, ResourceInspection, ResourceMutationReceipt, ResultInspection,
    ResultValueInspection, SaveGraphRequest, ToolEffect, ValidateGraphRequest,
    validate_graph_edit_operation, validate_resource_id,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityId {
    ChartInspect,
    ChartUpdate,
    ProjectInspectResource,
    ProjectCreateResource,
    DataImport,
    DataInspect,
    DataInspectSchema,
    DataInspectProfile,
    DataReadRows,
    DocumentInspect,
    DocumentRead,
    DocumentSearch,
    DocumentReplaceText,
    DocumentAppendText,
    DocumentWriteText,

    MindInspect,
    MindSearchTopics,
    MindInspectTopics,
    MindCreateTopics,
    MindUpdateTopics,
    MindMoveTopics,
    MindDeleteTopics,
    MindDuplicateTopics,

    DataCreateRows,
    DataUpdateCells,
    DataDeleteRows,
    DataCreateColumns,
    DataRenameColumns,
    DataDeleteColumns,
    DataCastColumns,
    DataUpdateColumnSemantics,
    ProjectRenameResource,
    ProjectDuplicateResource,
    ProjectDeleteResource,
    ProjectSaveResource,
    ProjectUndoResource,
    ProjectRedoResource,
    GraphUpdateFunctionSignature,
    DataExport,
    UiInspectReceipt,
    UiOpen,
    GraphInspect,
    CatalogSearchNodes,
    CatalogInspectNodes,
    GraphSearchNodes,
    GraphSearchConstants,
    GraphInspectConstants,
    GraphCreateConstants,
    GraphUpdateConstants,
    GraphDeleteConstants,

    GraphInspectNodes,
    GraphSearchConnections,
    GraphCreateNodes,
    GraphUpdateNodes,
    GraphDeleteNodes,
    GraphDuplicateNodes,
    GraphMoveNodes,
    GraphCreateConnections,
    GraphUpdateConnections,
    GraphDeleteConnections,
    KnowledgeSearch,
    KnowledgeRead,
    WebSearch,
    WebFetch,
    PythonInspectEnvironment,
    PythonReadScript,
    PythonWriteScript,
    PythonInstallPackages,
    PythonRunScript,
    ResultInspect,
    ResultReadTable,
    ProjectListResources,
    GraphApplyEdit,
    GraphValidate,
    GraphExecute,
    GraphSave,
    ResultList,
}

impl CapabilityId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChartInspect => "chart_inspect",
            Self::ChartUpdate => "chart_update",
            Self::ProjectInspectResource => "project_inspect_resource",
            Self::ProjectCreateResource => "project_create_resource",
            Self::DataInspect => "data_inspect",
            Self::DataInspectSchema => "data_inspect_schema",
            Self::DataInspectProfile => "data_inspect_profile",
            Self::DataReadRows => "data_read_rows",
            Self::DocumentInspect => "document_inspect",
            Self::DocumentRead => "document_read",
            Self::DocumentSearch => "document_search",
            Self::DocumentReplaceText => "document_replace_text",
            Self::DocumentAppendText => "document_append_text",
            Self::DocumentWriteText => "document_write_text",

            Self::MindInspect => "mind_inspect",
            Self::MindSearchTopics => "mind_search_topics",
            Self::MindInspectTopics => "mind_inspect_topics",
            Self::MindCreateTopics => "mind_create_topics",
            Self::MindUpdateTopics => "mind_update_topics",
            Self::MindMoveTopics => "mind_move_topics",
            Self::MindDeleteTopics => "mind_delete_topics",
            Self::MindDuplicateTopics => "mind_duplicate_topics",

            Self::DataImport => "data_import",
            Self::DataCreateRows => "data_create_rows",
            Self::DataUpdateCells => "data_update_cells",
            Self::DataDeleteRows => "data_delete_rows",
            Self::DataCreateColumns => "data_create_columns",
            Self::DataRenameColumns => "data_rename_columns",
            Self::DataDeleteColumns => "data_delete_columns",
            Self::DataCastColumns => "data_cast_columns",
            Self::DataUpdateColumnSemantics => "data_update_column_semantics",
            Self::ProjectRenameResource => "project_rename_resource",
            Self::ProjectDuplicateResource => "project_duplicate_resource",
            Self::ProjectDeleteResource => "project_delete_resource",
            Self::ProjectSaveResource => "project_save_resource",
            Self::ProjectUndoResource => "project_undo_resource",
            Self::ProjectRedoResource => "project_redo_resource",
            Self::GraphUpdateFunctionSignature => "graph_update_function_signature",
            Self::DataExport => "data_export",
            Self::UiInspectReceipt => "ui_inspect_receipt",
            Self::UiOpen => "ui_open",
            Self::GraphInspect => "graph_inspect",
            Self::CatalogSearchNodes => "catalog_search_nodes",
            Self::CatalogInspectNodes => "catalog_inspect_nodes",
            Self::GraphSearchNodes => "graph_search_nodes",
            Self::GraphSearchConstants => "graph_search_constants",
            Self::GraphInspectConstants => "graph_inspect_constants",
            Self::GraphCreateConstants => "graph_create_constants",
            Self::GraphUpdateConstants => "graph_update_constants",
            Self::GraphDeleteConstants => "graph_delete_constants",

            Self::GraphInspectNodes => "graph_inspect_nodes",
            Self::GraphSearchConnections => "graph_search_connections",
            Self::GraphCreateNodes => "graph_create_nodes",
            Self::GraphUpdateNodes => "graph_update_nodes",
            Self::GraphDeleteNodes => "graph_delete_nodes",
            Self::GraphDuplicateNodes => "graph_duplicate_nodes",
            Self::GraphMoveNodes => "graph_move_nodes",
            Self::GraphCreateConnections => "graph_create_connections",
            Self::GraphUpdateConnections => "graph_update_connections",
            Self::GraphDeleteConnections => "graph_delete_connections",
            Self::KnowledgeSearch => "knowledge_search",
            Self::KnowledgeRead => "knowledge_read",
            Self::WebSearch => "web_search",
            Self::WebFetch => "web_fetch",
            Self::PythonInspectEnvironment => "python_inspect_environment",
            Self::PythonReadScript => "python_read_script",
            Self::PythonWriteScript => "python_write_script",
            Self::PythonInstallPackages => "python_install_packages",
            Self::PythonRunScript => "python_run_script",
            Self::ResultInspect => "result_inspect",
            Self::ResultReadTable => "result_read_table",
            Self::ProjectListResources => "project_list_resources",
            Self::GraphApplyEdit => "graph_apply_edit",
            Self::GraphValidate => "graph_validate",
            Self::GraphExecute => "graph_execute",
            Self::GraphSave => "graph_save",
            Self::ResultList => "result_list",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityDescriptor {
    pub id: CapabilityId,
    pub effect: ToolEffect,
    pub maximum_results: u16,
}

impl CapabilityDescriptor {
    pub const fn timeout_ms(&self) -> u64 {
        self.effect.default_timeout_ms()
    }
}

macro_rules! capability_descriptors {
    (
        public { $($public:ident => $public_effect:ident, $public_maximum:expr;)* }
        internal { $($internal:ident => $internal_effect:ident, $internal_maximum:expr;)* }
    ) => {
        impl CapabilityId {
            pub const fn descriptor(self) -> &'static CapabilityDescriptor {
                match self {
                    $(Self::$public => &CapabilityDescriptor {
                        id: Self::$public,
                        effect: ToolEffect::$public_effect,
                        maximum_results: $public_maximum,
                    },)*
                    $(Self::$internal => &CapabilityDescriptor {
                        id: Self::$internal,
                        effect: ToolEffect::$internal_effect,
                        maximum_results: $internal_maximum,
                    },)*
                }
            }
        }

        pub const CAPABILITY_DESCRIPTORS: [CapabilityDescriptor;
            [$(stringify!($public)),*].len()
        ] = [$( *CapabilityId::$public.descriptor(), )*];
    };
}

capability_descriptors! {
    public {
        GraphInspect => Inspect, 2_000;
        CatalogSearchNodes => Inspect, MAX_CATALOG_RESULTS;
        DataInspectSchema => Inspect, 100;
        DataInspectProfile => Inspect, 1;
        ResultInspect => Inspect, 100;
        ProjectListResources => Inspect, 2_000;
        GraphValidate => Inspect, 200;
        GraphExecute => Compute, 100;
        ResultList => Inspect, 100;
        UiInspectReceipt => Inspect, 1;
        UiOpen => Mutate, 1;
        ProjectInspectResource => Inspect, 1;
        ProjectCreateResource => Mutate, 2_000;
        GraphUpdateFunctionSignature => Mutate, 500;
        DataExport => Mutate, 1;
        KnowledgeSearch => Inspect, crate::MAX_KNOWLEDGE_RESULTS;
        KnowledgeRead => Inspect, 1;
        WebSearch => Inspect, crate::MAX_WEB_RESULTS;
        WebFetch => Inspect, 1;
        DataImport => Mutate, 1;
        ProjectRenameResource => Mutate, 2_000;
        ProjectDuplicateResource => Mutate, 2_000;
        ProjectDeleteResource => Destructive, 2_000;
        ProjectSaveResource => Mutate, 2_000;
        CatalogInspectNodes => Inspect, MAX_CATALOG_RESULTS;
        GraphSearchNodes => Inspect, 100;
        GraphInspectNodes => Inspect, 100;
        GraphSearchConnections => Inspect, 100;
        GraphCreateNodes => Mutate, 200;
        GraphUpdateNodes => Mutate, 200;
        GraphDeleteNodes => Mutate, 200;
        GraphDuplicateNodes => Mutate, 200;
        GraphMoveNodes => Mutate, 200;
        GraphCreateConnections => Mutate, 200;
        GraphUpdateConnections => Mutate, 200;
        GraphDeleteConnections => Mutate, 200;
        GraphSearchConstants => Inspect, 200;
        GraphInspectConstants => Inspect, 200;
        GraphCreateConstants => Mutate, 200;
        GraphUpdateConstants => Mutate, 200;
        GraphDeleteConstants => Mutate, 200;
        ResultReadTable => Inspect, 1000;
        ProjectUndoResource => Mutate, 1;
        ProjectRedoResource => Mutate, 1;
        DataCreateRows => Mutate, 200;
        DataUpdateCells => Mutate, 200;
        DataDeleteRows => Mutate, 200;
        DataCreateColumns => Mutate, 200;
        DataRenameColumns => Mutate, 200;
        DataDeleteColumns => Mutate, 200;
        DataCastColumns => Mutate, 200;
        DataUpdateColumnSemantics => Mutate, 200;
        DataInspect => Inspect, 1;
        DataReadRows => Inspect, 1000;
        MindInspect => Inspect, 200;
        MindSearchTopics => Inspect, 100;
        MindInspectTopics => Inspect, 20;
        MindCreateTopics => Mutate, 200;
        MindUpdateTopics => Mutate, 200;
        MindMoveTopics => Mutate, 200;
        MindDeleteTopics => Mutate, 200;
        MindDuplicateTopics => Mutate, 200;
        DocumentInspect => Inspect, 100;
        DocumentRead => Inspect, 16384;
        DocumentSearch => Inspect, 100;
        DocumentReplaceText => Mutate, 200;
        DocumentAppendText => Mutate, 1;
        DocumentWriteText => Mutate, 1;
        ChartInspect => Inspect, 1;
        ChartUpdate => Mutate, 1;
        PythonInspectEnvironment => Inspect, 1;
        PythonReadScript => Inspect, 1;
        PythonWriteScript => Mutate, 1;
        PythonInstallPackages => Execute, 1;
        PythonRunScript => Execute, 1;
    }
    // Internal owner operations have metadata but are not model tools.
    internal {
        GraphApplyEdit => Mutate, 200;
        GraphSave => Mutate, 1;
    }
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityRequest {
    InspectChart(crate::InspectChartRequest),
    InspectResource(InspectResourceRequest),
    ReadDatabase(crate::DatabaseReadRequest),
    ReadMind(crate::MindReadRequest),
    ReadDocument(crate::DocumentReadRequest),
    ManageResource(ManageResourceRequest),
    EditResource(EditResourceRequest),
    ExportDatabase(ExportDatabaseRequest),
    InspectUiIntent(yss_ui_contract::InspectUiIntentRequest),
    RequestUiIntent(crate::RequestUiIntent),
    InspectGraph(InspectGraphRequest),
    BrowseNodes(BrowseNodesRequest),
    InspectNodeType(crate::InspectNodeTypeRequest),
    FindNodes(crate::FindNodesRequest),
    FindConstants(crate::FindConstantsRequest),
    InspectConstants(crate::InspectConstantsRequest),

    InspectNodes(crate::InspectNodesRequest),
    FindConnections(crate::FindConnectionsRequest),
    InspectResult(InspectResultRequest),
    ReadResultTable(crate::ReadResultTableRequest),
    ListResources(ListResourcesRequest),
    ApplyGraphEdit(ApplyGraphEditRequest),
    GraphMutation(crate::GraphMutationRequest),
    ValidateGraph(ValidateGraphRequest),
    ExecuteGraph(ExecuteGraphRequest),
    SaveGraph(SaveGraphRequest),
    ListGraphResults(ListGraphResultsRequest),
}

impl AutomationCapabilityRequest {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::InspectChart(_) => CapabilityId::ChartInspect,
            Self::InspectResource(_) => CapabilityId::ProjectInspectResource,
            Self::ReadDatabase(value) => value.input.capability_id(),
            Self::ReadMind(value) => value.input.capability_id(),
            Self::ReadDocument(value) => value.input.capability_id(),
            Self::ManageResource(value) => match value {
                ManageResourceRequest::Create {
                    specification: crate::ResourceCreation::Database { .. },
                } => CapabilityId::DataImport,
                ManageResourceRequest::Create { .. } => CapabilityId::ProjectCreateResource,
                ManageResourceRequest::Rename { .. } => CapabilityId::ProjectRenameResource,
                ManageResourceRequest::Duplicate { .. } => CapabilityId::ProjectDuplicateResource,
                ManageResourceRequest::Delete { .. } => CapabilityId::ProjectDeleteResource,
                ManageResourceRequest::Save { .. } => CapabilityId::ProjectSaveResource,
            },
            Self::EditResource(value) => value.capability_id(),
            Self::ExportDatabase(_) => CapabilityId::DataExport,
            Self::InspectUiIntent(_) => CapabilityId::UiInspectReceipt,
            Self::RequestUiIntent(_) => CapabilityId::UiOpen,
            Self::InspectGraph(_) => CapabilityId::GraphInspect,
            Self::BrowseNodes(_) => CapabilityId::CatalogSearchNodes,
            Self::InspectNodeType(_) => CapabilityId::CatalogInspectNodes,
            Self::FindNodes(_) => CapabilityId::GraphSearchNodes,
            Self::FindConstants(_) => CapabilityId::GraphSearchConstants,
            Self::InspectConstants(_) => CapabilityId::GraphInspectConstants,

            Self::InspectNodes(_) => CapabilityId::GraphInspectNodes,
            Self::FindConnections(_) => CapabilityId::GraphSearchConnections,
            Self::InspectResult(_) => CapabilityId::ResultInspect,
            Self::ReadResultTable(_) => CapabilityId::ResultReadTable,
            Self::ListResources(_) => CapabilityId::ProjectListResources,
            Self::ApplyGraphEdit(_) => CapabilityId::GraphApplyEdit,
            Self::GraphMutation(value) => value.input.capability_id(),
            Self::ValidateGraph(_) => CapabilityId::GraphValidate,
            Self::ExecuteGraph(_) => CapabilityId::GraphExecute,
            Self::SaveGraph(_) => CapabilityId::GraphSave,
            Self::ListGraphResults(_) => CapabilityId::ResultList,
        }
    }

    pub fn graph_edit(&self) -> Option<ApplyGraphEditRequest> {
        match self {
            Self::ApplyGraphEdit(value) => Some(value.clone()),
            Self::GraphMutation(value) => Some(value.edit_request()),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        match self {
            Self::InspectChart(request) => validate_resource_id("chart.id", &request.chart.id),
            Self::InspectResource(request) => request.validate(),
            Self::ReadDatabase(request) => request.input.validate(),
            Self::ReadMind(request) => request.input.validate(),
            Self::ReadDocument(request) => request.input.validate(),
            Self::ManageResource(request) => request.validate(),
            Self::EditResource(request) => request.validate(),
            Self::ExportDatabase(request) => request.validate(),
            Self::InspectUiIntent(request) => validate_resource_id("id", &request.id),
            Self::RequestUiIntent(request) => request
                .validate()
                .map_err(|_| CapabilityContractError::InvalidField("intent")),
            Self::InspectGraph(request) => request.validate(),
            Self::FindNodes(request) => request.validate(),
            Self::FindConstants(request) => request.validate(),
            Self::InspectConstants(request) => request.validate(),

            Self::InspectNodes(request) => request.validate(),
            Self::FindConnections(request) => request.validate(),
            Self::InspectResult(request) => request.validate(),
            Self::ReadResultTable(request) => request.validate(),
            Self::ListResources(value) => {
                if value.limit == 0 || value.limit > 100 {
                    return Err(CapabilityContractError::InvalidLimit { maximum: 100 });
                }
                if value
                    .query
                    .as_ref()
                    .is_some_and(|query| query.len() > MAX_CATALOG_QUERY_BYTES)
                {
                    return Err(CapabilityContractError::InvalidField("query"));
                }
                Ok(())
            }
            Self::ValidateGraph(request) => {
                validate_graph_request(&request.graph.id, &request.graph_hash)?;
                if request.node_ids.len() > 200 {
                    return Err(CapabilityContractError::InvalidField("nodeIds"));
                }
                if request.limit == 0 || request.limit > 100 {
                    return Err(CapabilityContractError::InvalidField("limit"));
                }
                for id in &request.node_ids {
                    validate_resource_id("nodeIds", id)?;
                }
                Ok(())
            }
            Self::SaveGraph(request) => {
                validate_graph_request(&request.graph_path, &request.graph_hash)
            }
            Self::ExecuteGraph(request) => {
                validate_graph_request(&request.graph.id, &request.graph_hash)?;
                if let crate::GraphExecutionDemand::Node { node_id, .. } = &request.demand {
                    validate_resource_id("nodeId", node_id)?;
                }
                Ok(())
            }
            Self::ListGraphResults(request) => request.validate(),
            Self::GraphMutation(request) => {
                request.input.validate()?;
                Self::ApplyGraphEdit(request.edit_request()).validate()
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
            Self::BrowseNodes(request) => {
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
                if let Some(category) = &request.category {
                    validate_resource_id("category", category)?;
                }
                Ok(())
            }
            Self::InspectNodeType(request) => {
                if request.locale.trim().is_empty() || request.locale.len() > MAX_LOCALE_BYTES {
                    return Err(CapabilityContractError::InvalidField("locale"));
                }
                if request.type_ids.is_empty()
                    || request.type_ids.len() > usize::from(MAX_CATALOG_RESULTS)
                {
                    return Err(CapabilityContractError::InvalidField("typeIds"));
                }
                for id in &request.type_ids {
                    validate_resource_id("typeIds", id)?;
                }
                Ok(())
            }
        }
    }
}

/// Report a contract violation against the capability that received it.
pub fn contract_failure(
    error: CapabilityContractError,
    capability: CapabilityId,
) -> CapabilityFailure {
    let limit_field = match capability {
        CapabilityId::GraphApplyEdit | CapabilityId::GraphUpdateFunctionSignature => "operations",
        _ => "limit",
    };
    error.into_failure(capability.as_str(), limit_field)
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AutomationCapabilityResult {
    ChartInspection(crate::ChartInspection),
    ResourceInspection(ResourceInspection),
    DatabaseRead(crate::DatabaseReadResult),
    MindRead(crate::MindReadResult),
    DocumentRead(crate::DocumentReadResult),
    ResourceManaged(ResourceMutationReceipt),
    ResourceEdited(ResourceMutationReceipt),
    DatabaseExported(DatabaseExported),
    UiIntentInspection(yss_ui_contract::UiIntentReceipt),
    UiIntentReceipt(yss_ui_contract::UiIntentReceipt),
    GraphInspection(GraphInspection),
    GraphInspectionPage(GraphInspectionPage),
    NodeCatalogPage(NodeCatalogPage),
    NodeTypeInspection(crate::NodeTypeInspection),
    ResultInspection(ResultInspection),
    ResultTablePage(crate::ResultTablePage),
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
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))?;
        if bytes.len() > maximum_bytes {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::RESULT_TOO_LARGE,
            ));
        }
        Ok(())
    }
    pub fn accepts_capability(&self, capability: CapabilityId) -> bool {
        let expected = match self {
            Self::ResourceManaged(_) => {
                return matches!(
                    capability,
                    CapabilityId::ProjectCreateResource
                        | CapabilityId::DataImport
                        | CapabilityId::ProjectRenameResource
                        | CapabilityId::ProjectDuplicateResource
                        | CapabilityId::ProjectDeleteResource
                        | CapabilityId::ProjectSaveResource
                );
            }
            Self::ChartInspection(_) => CapabilityId::ChartInspect,
            Self::ResourceInspection(_) => CapabilityId::ProjectInspectResource,
            Self::DatabaseRead(value) => value.content.capability_id(),
            Self::MindRead(value) => value.content.capability_id(),
            Self::DocumentRead(value) => value.content.capability_id(),
            Self::ResourceEdited(_) => {
                return matches!(
                    capability,
                    CapabilityId::ChartUpdate
                        | CapabilityId::GraphUpdateFunctionSignature
                        | CapabilityId::DocumentReplaceText
                        | CapabilityId::DocumentAppendText
                        | CapabilityId::DocumentWriteText
                        | CapabilityId::MindCreateTopics
                        | CapabilityId::MindUpdateTopics
                        | CapabilityId::MindMoveTopics
                        | CapabilityId::MindDeleteTopics
                        | CapabilityId::MindDuplicateTopics
                        | CapabilityId::DataCreateRows
                        | CapabilityId::DataUpdateCells
                        | CapabilityId::DataDeleteRows
                        | CapabilityId::DataCreateColumns
                        | CapabilityId::DataRenameColumns
                        | CapabilityId::DataDeleteColumns
                        | CapabilityId::DataCastColumns
                        | CapabilityId::DataUpdateColumnSemantics
                        | CapabilityId::ProjectUndoResource
                        | CapabilityId::ProjectRedoResource
                );
            }
            Self::DatabaseExported(_) => CapabilityId::DataExport,
            Self::GraphInspection(_) => CapabilityId::GraphInspect,
            Self::GraphInspectionPage(_) => {
                return matches!(
                    capability,
                    CapabilityId::GraphInspect
                        | CapabilityId::GraphSearchConstants
                        | CapabilityId::GraphInspectConstants
                        | CapabilityId::GraphSearchNodes
                        | CapabilityId::GraphInspectNodes
                        | CapabilityId::GraphSearchConnections
                );
            }
            Self::NodeCatalogPage(_) => CapabilityId::CatalogSearchNodes,
            Self::NodeTypeInspection(_) => CapabilityId::CatalogInspectNodes,
            Self::ResultInspection(_) => CapabilityId::ResultInspect,
            Self::ResultTablePage(_) => CapabilityId::ResultReadTable,
            Self::ProjectInspection(_) => CapabilityId::ProjectListResources,
            Self::GraphEditReceipt(_) => {
                return matches!(
                    capability,
                    CapabilityId::GraphApplyEdit
                        | CapabilityId::GraphCreateConstants
                        | CapabilityId::GraphUpdateConstants
                        | CapabilityId::GraphDeleteConstants
                        | CapabilityId::GraphCreateNodes
                        | CapabilityId::GraphUpdateNodes
                        | CapabilityId::GraphDeleteNodes
                        | CapabilityId::GraphDuplicateNodes
                        | CapabilityId::GraphMoveNodes
                        | CapabilityId::GraphCreateConnections
                        | CapabilityId::GraphUpdateConnections
                        | CapabilityId::GraphDeleteConnections
                );
            }
            Self::GraphValidation(_) => CapabilityId::GraphValidate,
            Self::GraphExecution(_) => CapabilityId::GraphExecute,
            Self::GraphSaved(_) => CapabilityId::GraphSave,
            Self::GraphResults(_) => CapabilityId::ResultList,
            Self::UiIntentInspection(_) => CapabilityId::UiInspectReceipt,
            Self::UiIntentReceipt(_) => CapabilityId::UiOpen,
        };
        expected == capability
    }
}

/// Business failure classifications added to the Harness codes.
pub mod failure_code {
    use crate::CapabilityFailureCode;

    pub const GRAPH_UNAVAILABLE: CapabilityFailureCode =
        CapabilityFailureCode::from_static("graph_unavailable");
    pub const DATABASE_UNAVAILABLE: CapabilityFailureCode =
        CapabilityFailureCode::from_static("database_unavailable");
    pub const CATALOG_UNAVAILABLE: CapabilityFailureCode =
        CapabilityFailureCode::from_static("catalog_unavailable");
    pub const GRAPH_DRAFT_CHANGED: CapabilityFailureCode =
        CapabilityFailureCode::from_static("graph_draft_changed");
}

pub fn capability_input_schema(capability_id: CapabilityId) -> schemars::Schema {
    use crate::model::*;
    let mut schema = match capability_id {
        CapabilityId::ChartInspect => schemars::schema_for!(InspectChartInput),
        CapabilityId::ChartUpdate => schemars::schema_for!(UpdateChartInput),
        CapabilityId::ProjectInspectResource => {
            schemars::schema_for!(crate::model::InspectResourceInput)
        }
        CapabilityId::ProjectCreateResource => schemars::schema_for!(CreateResourceInput),
        CapabilityId::DataInspect => schemars::schema_for!(InspectDatabaseInput),
        CapabilityId::DataInspectSchema => schemars::schema_for!(InspectDatabaseSchemaInput),
        CapabilityId::DataInspectProfile => schemars::schema_for!(ProfileDatabaseInput),
        CapabilityId::DataReadRows => schemars::schema_for!(ReadDatabaseRowsInput),
        CapabilityId::DocumentInspect => schemars::schema_for!(InspectDocumentInput),
        CapabilityId::DocumentRead => schemars::schema_for!(ReadDocumentInput),
        CapabilityId::DocumentSearch => schemars::schema_for!(SearchDocumentInput),
        CapabilityId::DocumentReplaceText => schemars::schema_for!(ReplaceDocumentTextInput),
        CapabilityId::DocumentAppendText => schemars::schema_for!(AppendDocumentInput),
        CapabilityId::DocumentWriteText => schemars::schema_for!(WriteDocumentInput),

        CapabilityId::MindInspect => schemars::schema_for!(InspectMindInput),
        CapabilityId::MindSearchTopics => schemars::schema_for!(FindTopicsInput),
        CapabilityId::MindInspectTopics => schemars::schema_for!(InspectTopicsInput),
        CapabilityId::MindCreateTopics => schemars::schema_for!(CreateTopicsInput),
        CapabilityId::MindUpdateTopics => schemars::schema_for!(UpdateTopicsInput),
        CapabilityId::MindMoveTopics => schemars::schema_for!(MoveTopicsInput),
        CapabilityId::MindDeleteTopics => schemars::schema_for!(DeleteTopicsInput),
        CapabilityId::MindDuplicateTopics => schemars::schema_for!(DuplicateTopicsInput),

        CapabilityId::DataImport => schemars::schema_for!(ImportDatabaseInput),
        CapabilityId::DataCreateRows => schemars::schema_for!(InsertRowsInput),
        CapabilityId::DataUpdateCells => schemars::schema_for!(UpdateCellsInput),
        CapabilityId::DataDeleteRows => schemars::schema_for!(DeleteRowsInput),
        CapabilityId::DataCreateColumns => schemars::schema_for!(CreateColumnsInput),
        CapabilityId::DataRenameColumns => schemars::schema_for!(RenameColumnsInput),
        CapabilityId::DataDeleteColumns => schemars::schema_for!(DeleteColumnsInput),
        CapabilityId::DataCastColumns => schemars::schema_for!(CastColumnsInput),
        CapabilityId::DataUpdateColumnSemantics => schemars::schema_for!(SetColumnSemanticsInput),
        CapabilityId::ProjectRenameResource => schemars::schema_for!(RenameResourceInput),
        CapabilityId::ProjectDuplicateResource => schemars::schema_for!(DuplicateResourceInput),
        CapabilityId::ProjectDeleteResource
        | CapabilityId::ProjectSaveResource
        | CapabilityId::ProjectUndoResource
        | CapabilityId::ProjectRedoResource => {
            schemars::schema_for!(ResourceTargetInput)
        }
        CapabilityId::GraphUpdateFunctionSignature => schemars::schema_for!(EditResourceInput),
        CapabilityId::DataExport => schemars::schema_for!(ExportDatabaseInput),
        CapabilityId::UiInspectReceipt => {
            schemars::schema_for!(yss_ui_contract::InspectUiIntentRequest)
        }
        CapabilityId::UiOpen => schemars::schema_for!(RequestUiIntentInput),
        CapabilityId::GraphInspect => schemars::schema_for!(InspectGraphInput),
        CapabilityId::CatalogSearchNodes => schemars::schema_for!(BrowseNodesRequest),
        CapabilityId::CatalogInspectNodes => schemars::schema_for!(crate::InspectNodeTypeRequest),
        CapabilityId::GraphSearchNodes => schemars::schema_for!(crate::FindNodesRequest),
        CapabilityId::GraphSearchConstants => schemars::schema_for!(crate::FindConstantsRequest),
        CapabilityId::GraphInspectConstants => {
            schemars::schema_for!(crate::InspectConstantsRequest)
        }
        CapabilityId::GraphCreateConstants => schemars::schema_for!(crate::CreateConstantsInput),
        CapabilityId::GraphUpdateConstants => schemars::schema_for!(crate::UpdateConstantsInput),
        CapabilityId::GraphDeleteConstants => schemars::schema_for!(crate::DeleteConstantsInput),

        CapabilityId::GraphInspectNodes => schemars::schema_for!(crate::InspectNodesRequest),
        CapabilityId::GraphSearchConnections => {
            schemars::schema_for!(crate::FindConnectionsRequest)
        }
        CapabilityId::KnowledgeSearch => schemars::schema_for!(crate::SearchKnowledgeRequest),
        CapabilityId::WebSearch => schemars::schema_for!(crate::WebSearchRequest),
        CapabilityId::WebFetch => schemars::schema_for!(crate::WebFetchRequest),
        CapabilityId::PythonInspectEnvironment => {
            schemars::schema_for!(crate::PythonInspectEnvironmentRequest)
        }
        CapabilityId::PythonReadScript => schemars::schema_for!(crate::PythonReadScriptRequest),
        CapabilityId::PythonWriteScript => schemars::schema_for!(crate::PythonWriteScriptRequest),
        CapabilityId::PythonInstallPackages => {
            schemars::schema_for!(crate::PythonInstallPackagesRequest)
        }
        CapabilityId::PythonRunScript => schemars::schema_for!(crate::PythonRunScriptRequest),
        CapabilityId::KnowledgeRead => schemars::schema_for!(crate::ReadKnowledgeRequest),
        CapabilityId::ResultInspect => schemars::schema_for!(InspectResultRequest),
        CapabilityId::ResultReadTable => schemars::schema_for!(crate::ReadResultTableRequest),
        CapabilityId::ProjectListResources => schemars::schema_for!(ListResourcesRequest),
        CapabilityId::GraphApplyEdit | CapabilityId::GraphSave => return false.into(),
        CapabilityId::GraphCreateNodes => schemars::schema_for!(CreateNodesInput),
        CapabilityId::GraphUpdateNodes => schemars::schema_for!(UpdateNodesInput),
        CapabilityId::GraphDeleteNodes => schemars::schema_for!(DeleteNodesInput),
        CapabilityId::GraphDuplicateNodes => schemars::schema_for!(DuplicateNodesInput),
        CapabilityId::GraphMoveNodes => schemars::schema_for!(MoveNodesInput),
        CapabilityId::GraphCreateConnections => schemars::schema_for!(CreateConnectionsInput),
        CapabilityId::GraphUpdateConnections => schemars::schema_for!(UpdateConnectionsInput),
        CapabilityId::GraphDeleteConnections => schemars::schema_for!(DeleteConnectionsInput),
        CapabilityId::GraphValidate => schemars::schema_for!(ValidateGraphInput),
        CapabilityId::GraphExecute => schemars::schema_for!(ExecuteGraphInput),
        CapabilityId::ResultList => schemars::schema_for!(ListGraphResultsRequest),
    };
    if matches!(
        capability_id,
        CapabilityId::ProjectUndoResource | CapabilityId::ProjectRedoResource
    ) {
        schema
            .get_mut("$defs")
            .and_then(|definitions| definitions.get_mut("ProjectResourceKind"))
            .expect("resource target schema defines its resource kinds")["enum"] =
            serde_json::json!(["event_graph", "function_graph", "database"]);
    }
    if capability_id == CapabilityId::GraphUpdateFunctionSignature {
        schema
            .get_mut("$defs")
            .and_then(|definitions| definitions.get_mut("ProjectResourceKind"))
            .expect("resource kinds")["enum"] = serde_json::json!(["function_graph"]);
    }
    // All capability arguments are objects, including tagged enums whose schemas
    // otherwise express this constraint only inside oneOf branches.
    schema.insert("type".into(), serde_json::json!("object"));
    schema
}
