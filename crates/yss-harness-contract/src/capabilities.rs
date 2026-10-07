//! Closed capability registry, typed envelopes, failures and model-facing schemas.

use crate::graph::{validate_graph_hash, validate_graph_request};

use crate::{
    ApplyGraphEditRequest, BrowseNodesRequest, DatabaseExported, DatasetProfileInspection,
    DatasetSchemaInspection, EditResourceRequest, ExecuteGraphRequest, ExportDatabaseRequest,
    GraphEditReceipt, GraphExecution, GraphInspection, GraphInspectionPage, GraphResults,
    GraphSaved, GraphValidation, InspectDatasetProfileRequest, InspectDatasetSchemaRequest,
    InspectGraphRequest, InspectResourceRequest, InspectResultRequest, ListGraphResultsRequest,
    ListResourcesRequest, MAX_CATALOG_QUERY_BYTES, MAX_CATALOG_RESULTS, MAX_LOCALE_BYTES,
    ManageResourceRequest, NodeCatalogPage, ProjectInspection, ResourceInspection,
    ResourceMutationReceipt, ResultInspection, ResultValueInspection, SaveGraphRequest,
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
    InspectChart,
    UpdateChart,
    InspectResource,
    CreateResource,
    ImportDatabase,
    InspectDatabase,
    InspectDatabaseSchema,
    ProfileDatabase,
    ReadDatabaseRows,
    InspectDocument,
    ReadDocument,
    SearchDocument,
    ReplaceDocumentText,
    AppendDocument,
    WriteDocument,

    InspectMind,
    FindTopics,
    InspectTopics,
    CreateTopics,
    UpdateTopics,
    MoveTopics,
    DeleteTopics,
    DuplicateTopics,

    InsertRows,
    UpdateCells,
    DeleteRows,
    CreateColumns,
    RenameColumns,
    DeleteColumns,
    CastColumns,
    SetColumnSemantics,
    RenameResource,
    DuplicateResource,
    DeleteResource,
    SaveResource,
    UndoResource,
    RedoResource,
    EditResource,
    ExportDatabase,
    InspectUiIntent,
    RequestUiIntent,
    InspectGraph,
    BrowseNodes,
    InspectNodeType,
    FindNodes,
    FindConstants,
    InspectConstants,
    CreateConstants,
    UpdateConstants,
    DeleteConstants,

    InspectNodes,
    FindConnections,
    CreateNodes,
    UpdateNodes,
    DeleteNodes,
    DuplicateNodes,
    MoveNodes,
    CreateConnections,
    UpdateConnections,
    DeleteConnections,
    SearchKnowledge,
    ReadKnowledge,
    InspectDatasetSchema,
    InspectDatasetProfile,
    InspectResult,
    ReadResultTable,
    ListResources,
    ApplyGraphEdit,
    ValidateGraph,
    ExecuteGraph,
    SaveGraph,
    ListGraphResults,
}

impl CapabilityId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectChart => "inspect_chart",
            Self::UpdateChart => "update_chart",
            Self::InspectResource => "inspect_resource",
            Self::CreateResource => "create_resource",
            Self::InspectDatabase => "inspect_database",
            Self::InspectDatabaseSchema => "inspect_database_schema",
            Self::ProfileDatabase => "profile_database",
            Self::ReadDatabaseRows => "read_database_rows",
            Self::InspectDocument => "inspect_document",
            Self::ReadDocument => "read_document",
            Self::SearchDocument => "search_document",
            Self::ReplaceDocumentText => "replace_document_text",
            Self::AppendDocument => "append_document",
            Self::WriteDocument => "write_document",

            Self::InspectMind => "inspect_mind",
            Self::FindTopics => "find_topics",
            Self::InspectTopics => "inspect_topics",
            Self::CreateTopics => "create_topics",
            Self::UpdateTopics => "update_topics",
            Self::MoveTopics => "move_topics",
            Self::DeleteTopics => "delete_topics",
            Self::DuplicateTopics => "duplicate_topics",

            Self::ImportDatabase => "import_database",
            Self::InsertRows => "insert_rows",
            Self::UpdateCells => "update_cells",
            Self::DeleteRows => "delete_rows",
            Self::CreateColumns => "create_columns",
            Self::RenameColumns => "rename_columns",
            Self::DeleteColumns => "delete_columns",
            Self::CastColumns => "cast_columns",
            Self::SetColumnSemantics => "set_column_semantics",
            Self::RenameResource => "rename_resource",
            Self::DuplicateResource => "duplicate_resource",
            Self::DeleteResource => "delete_resource",
            Self::SaveResource => "save_resource",
            Self::UndoResource => "undo_resource",
            Self::RedoResource => "redo_resource",
            Self::EditResource => "edit_resource",
            Self::ExportDatabase => "export_database",
            Self::InspectUiIntent => "inspect_ui_intent",
            Self::RequestUiIntent => "request_ui_intent",
            Self::InspectGraph => "inspect_graph",
            Self::BrowseNodes => "browse_nodes",
            Self::InspectNodeType => "inspect_node_type",
            Self::FindNodes => "find_nodes",
            Self::FindConstants => "find_constants",
            Self::InspectConstants => "inspect_constants",
            Self::CreateConstants => "create_constants",
            Self::UpdateConstants => "update_constants",
            Self::DeleteConstants => "delete_constants",

            Self::InspectNodes => "inspect_nodes",
            Self::FindConnections => "find_connections",
            Self::CreateNodes => "create_nodes",
            Self::UpdateNodes => "update_nodes",
            Self::DeleteNodes => "delete_nodes",
            Self::DuplicateNodes => "duplicate_nodes",
            Self::MoveNodes => "move_nodes",
            Self::CreateConnections => "create_connections",
            Self::UpdateConnections => "update_connections",
            Self::DeleteConnections => "delete_connections",
            Self::SearchKnowledge => "search_knowledge",
            Self::ReadKnowledge => "read_knowledge",
            Self::InspectDatasetSchema => "inspect_dataset_schema",
            Self::InspectDatasetProfile => "inspect_dataset_profile",
            Self::InspectResult => "inspect_result",
            Self::ReadResultTable => "read_result_table",
            Self::ListResources => "list_resources",
            Self::ApplyGraphEdit => "apply_graph_edit",
            Self::ValidateGraph => "validate_graph",
            Self::ExecuteGraph => "execute_graph",
            Self::SaveGraph => "save_graph",
            Self::ListGraphResults => "list_graph_results",
        }
    }

    pub const fn descriptor(self) -> &'static CapabilityDescriptor {
        match self {
            Self::InspectChart => &CAPABILITY_DESCRIPTORS[66],
            Self::UpdateChart => &CAPABILITY_DESCRIPTORS[67],
            Self::InspectResource => &CAPABILITY_DESCRIPTORS[11],
            Self::CreateResource => &CAPABILITY_DESCRIPTORS[12],
            Self::InspectDatabase => &CAPABILITY_DESCRIPTORS[50],
            Self::InspectDatabaseSchema => &CAPABILITY_DESCRIPTORS[2],
            Self::ProfileDatabase => &CAPABILITY_DESCRIPTORS[3],
            Self::ReadDatabaseRows => &CAPABILITY_DESCRIPTORS[51],
            Self::InspectDocument => &CAPABILITY_DESCRIPTORS[60],
            Self::ReadDocument => &CAPABILITY_DESCRIPTORS[61],
            Self::SearchDocument => &CAPABILITY_DESCRIPTORS[62],
            Self::ReplaceDocumentText => &CAPABILITY_DESCRIPTORS[63],
            Self::AppendDocument => &CAPABILITY_DESCRIPTORS[64],
            Self::WriteDocument => &CAPABILITY_DESCRIPTORS[65],

            Self::InspectMind => &CAPABILITY_DESCRIPTORS[52],
            Self::FindTopics => &CAPABILITY_DESCRIPTORS[53],
            Self::InspectTopics => &CAPABILITY_DESCRIPTORS[54],
            Self::CreateTopics => &CAPABILITY_DESCRIPTORS[55],
            Self::UpdateTopics => &CAPABILITY_DESCRIPTORS[56],
            Self::MoveTopics => &CAPABILITY_DESCRIPTORS[57],
            Self::DeleteTopics => &CAPABILITY_DESCRIPTORS[58],
            Self::DuplicateTopics => &CAPABILITY_DESCRIPTORS[59],

            Self::ImportDatabase => &CAPABILITY_DESCRIPTORS[17],
            Self::InsertRows => &CAPABILITY_DESCRIPTORS[42],
            Self::UpdateCells => &CAPABILITY_DESCRIPTORS[43],
            Self::DeleteRows => &CAPABILITY_DESCRIPTORS[44],
            Self::CreateColumns => &CAPABILITY_DESCRIPTORS[45],
            Self::RenameColumns => &CAPABILITY_DESCRIPTORS[46],
            Self::DeleteColumns => &CAPABILITY_DESCRIPTORS[47],
            Self::CastColumns => &CAPABILITY_DESCRIPTORS[48],
            Self::SetColumnSemantics => &CAPABILITY_DESCRIPTORS[49],
            Self::RenameResource => &CAPABILITY_DESCRIPTORS[18],
            Self::DuplicateResource => &CAPABILITY_DESCRIPTORS[19],
            Self::DeleteResource => &CAPABILITY_DESCRIPTORS[20],
            Self::SaveResource => &CAPABILITY_DESCRIPTORS[21],
            Self::UndoResource => &CAPABILITY_DESCRIPTORS[40],
            Self::RedoResource => &CAPABILITY_DESCRIPTORS[41],
            Self::EditResource => &CAPABILITY_DESCRIPTORS[13],
            Self::ExportDatabase => &CAPABILITY_DESCRIPTORS[14],
            Self::InspectUiIntent => &CAPABILITY_DESCRIPTORS[9],
            Self::RequestUiIntent => &CAPABILITY_DESCRIPTORS[10],
            Self::InspectGraph => &CAPABILITY_DESCRIPTORS[0],
            Self::BrowseNodes => &CAPABILITY_DESCRIPTORS[1],
            Self::InspectNodeType => &CAPABILITY_DESCRIPTORS[22],
            Self::FindNodes => &CAPABILITY_DESCRIPTORS[23],
            Self::FindConstants => &CAPABILITY_DESCRIPTORS[34],
            Self::InspectConstants => &CAPABILITY_DESCRIPTORS[35],
            Self::CreateConstants => &CAPABILITY_DESCRIPTORS[36],
            Self::UpdateConstants => &CAPABILITY_DESCRIPTORS[37],
            Self::DeleteConstants => &CAPABILITY_DESCRIPTORS[38],

            Self::InspectNodes => &CAPABILITY_DESCRIPTORS[24],
            Self::FindConnections => &CAPABILITY_DESCRIPTORS[25],
            Self::CreateNodes => &CAPABILITY_DESCRIPTORS[26],
            Self::UpdateNodes => &CAPABILITY_DESCRIPTORS[27],
            Self::DeleteNodes => &CAPABILITY_DESCRIPTORS[28],
            Self::DuplicateNodes => &CAPABILITY_DESCRIPTORS[29],
            Self::MoveNodes => &CAPABILITY_DESCRIPTORS[30],
            Self::CreateConnections => &CAPABILITY_DESCRIPTORS[31],
            Self::UpdateConnections => &CAPABILITY_DESCRIPTORS[32],
            Self::DeleteConnections => &CAPABILITY_DESCRIPTORS[33],
            Self::SearchKnowledge => &CAPABILITY_DESCRIPTORS[15],
            Self::ReadKnowledge => &CAPABILITY_DESCRIPTORS[16],
            Self::InspectDatasetSchema => &INTERNAL_INSPECTDATASETSCHEMA_DESCRIPTOR,
            Self::InspectDatasetProfile => &INTERNAL_INSPECTDATASETPROFILE_DESCRIPTOR,
            Self::InspectResult => &CAPABILITY_DESCRIPTORS[4],
            Self::ReadResultTable => &CAPABILITY_DESCRIPTORS[39],
            Self::ListResources => &CAPABILITY_DESCRIPTORS[5],
            Self::ApplyGraphEdit => &INTERNAL_APPLYGRAPHEDIT_DESCRIPTOR,
            Self::ValidateGraph => &CAPABILITY_DESCRIPTORS[6],
            Self::ExecuteGraph => &CAPABILITY_DESCRIPTORS[7],
            Self::SaveGraph => &INTERNAL_SAVEGRAPH_DESCRIPTOR,
            Self::ListGraphResults => &CAPABILITY_DESCRIPTORS[8],
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

// Internal owner operations and stored history retain their metadata, but are not model tools.
const INTERNAL_APPLYGRAPHEDIT_DESCRIPTOR: CapabilityDescriptor = CapabilityDescriptor {
    id: CapabilityId::ApplyGraphEdit,
    effect: ToolEffect::Mutate,
    maximum_results: 200,
};
const INTERNAL_SAVEGRAPH_DESCRIPTOR: CapabilityDescriptor = CapabilityDescriptor {
    id: CapabilityId::SaveGraph,
    effect: ToolEffect::Mutate,
    maximum_results: 1,
};

const INTERNAL_INSPECTDATASETSCHEMA_DESCRIPTOR: CapabilityDescriptor = CapabilityDescriptor {
    id: CapabilityId::InspectDatasetSchema,
    effect: ToolEffect::Inspect,
    maximum_results: 4096,
};

const INTERNAL_INSPECTDATASETPROFILE_DESCRIPTOR: CapabilityDescriptor = CapabilityDescriptor {
    id: CapabilityId::InspectDatasetProfile,
    effect: ToolEffect::Inspect,
    maximum_results: 1,
};

pub const CAPABILITY_DESCRIPTORS: [CapabilityDescriptor; 68] = [
    CapabilityDescriptor {
        id: CapabilityId::InspectGraph,
        effect: ToolEffect::Inspect,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::BrowseNodes,
        effect: ToolEffect::Inspect,
        maximum_results: MAX_CATALOG_RESULTS,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatabaseSchema,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::ProfileDatabase,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectResult,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::ListResources,
        effect: ToolEffect::Inspect,
        maximum_results: 2_000,
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
        id: CapabilityId::ListGraphResults,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectUiIntent,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::RequestUiIntent,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectResource,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateResource,
        effect: ToolEffect::Mutate,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::EditResource,
        effect: ToolEffect::Mutate,
        maximum_results: 500,
    },
    CapabilityDescriptor {
        id: CapabilityId::ExportDatabase,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::SearchKnowledge,
        effect: ToolEffect::Inspect,
        maximum_results: crate::MAX_KNOWLEDGE_RESULTS,
    },
    CapabilityDescriptor {
        id: CapabilityId::ReadKnowledge,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::ImportDatabase,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::RenameResource,
        effect: ToolEffect::Mutate,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::DuplicateResource,
        effect: ToolEffect::Mutate,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteResource,
        effect: ToolEffect::Destructive,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::SaveResource,
        effect: ToolEffect::Mutate,
        maximum_results: 2_000,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectNodeType,
        effect: ToolEffect::Inspect,
        maximum_results: MAX_CATALOG_RESULTS,
    },
    CapabilityDescriptor {
        id: CapabilityId::FindNodes,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectNodes,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::FindConnections,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateNodes,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateNodes,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteNodes,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DuplicateNodes,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::MoveNodes,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateConnections,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateConnections,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteConnections,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::FindConstants,
        effect: ToolEffect::Inspect,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectConstants,
        effect: ToolEffect::Inspect,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateConstants,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateConstants,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteConstants,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::ReadResultTable,
        effect: ToolEffect::Inspect,
        maximum_results: 1000,
    },
    CapabilityDescriptor {
        id: CapabilityId::UndoResource,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::RedoResource,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InsertRows,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateCells,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteRows,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateColumns,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::RenameColumns,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteColumns,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::CastColumns,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::SetColumnSemantics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDatabase,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::ReadDatabaseRows,
        effect: ToolEffect::Inspect,
        maximum_results: 1000,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectMind,
        effect: ToolEffect::Inspect,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::FindTopics,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectTopics,
        effect: ToolEffect::Inspect,
        maximum_results: 20,
    },
    CapabilityDescriptor {
        id: CapabilityId::CreateTopics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateTopics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::MoveTopics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DeleteTopics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::DuplicateTopics,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectDocument,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::ReadDocument,
        effect: ToolEffect::Inspect,
        maximum_results: 16384,
    },
    CapabilityDescriptor {
        id: CapabilityId::SearchDocument,
        effect: ToolEffect::Inspect,
        maximum_results: 100,
    },
    CapabilityDescriptor {
        id: CapabilityId::ReplaceDocumentText,
        effect: ToolEffect::Mutate,
        maximum_results: 200,
    },
    CapabilityDescriptor {
        id: CapabilityId::AppendDocument,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::WriteDocument,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::InspectChart,
        effect: ToolEffect::Inspect,
        maximum_results: 1,
    },
    CapabilityDescriptor {
        id: CapabilityId::UpdateChart,
        effect: ToolEffect::Mutate,
        maximum_results: 1,
    },
];

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
    SearchKnowledge(crate::SearchKnowledgeRequest),
    ReadKnowledge(crate::ReadKnowledgeRequest),
    InspectDatasetSchema(InspectDatasetSchemaRequest),
    InspectDatasetProfile(InspectDatasetProfileRequest),
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
            Self::InspectChart(_) => CapabilityId::InspectChart,
            Self::InspectResource(_) => CapabilityId::InspectResource,
            Self::ReadDatabase(value) => value.input.capability_id(),
            Self::ReadMind(value) => value.input.capability_id(),
            Self::ReadDocument(value) => value.input.capability_id(),
            Self::ManageResource(value) => match value {
                ManageResourceRequest::Create {
                    specification: crate::ResourceCreation::Database { .. },
                } => CapabilityId::ImportDatabase,
                ManageResourceRequest::Create { .. } => CapabilityId::CreateResource,
                ManageResourceRequest::Rename { .. } => CapabilityId::RenameResource,
                ManageResourceRequest::Duplicate { .. } => CapabilityId::DuplicateResource,
                ManageResourceRequest::Delete { .. } => CapabilityId::DeleteResource,
                ManageResourceRequest::Save { .. } => CapabilityId::SaveResource,
            },
            Self::EditResource(value) => value.capability_id(),
            Self::ExportDatabase(_) => CapabilityId::ExportDatabase,
            Self::InspectUiIntent(_) => CapabilityId::InspectUiIntent,
            Self::RequestUiIntent(_) => CapabilityId::RequestUiIntent,
            Self::InspectGraph(_) => CapabilityId::InspectGraph,
            Self::BrowseNodes(_) => CapabilityId::BrowseNodes,
            Self::InspectNodeType(_) => CapabilityId::InspectNodeType,
            Self::FindNodes(_) => CapabilityId::FindNodes,
            Self::FindConstants(_) => CapabilityId::FindConstants,
            Self::InspectConstants(_) => CapabilityId::InspectConstants,

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
            Self::GraphMutation(value) => value.input.capability_id(),
            Self::ValidateGraph(_) => CapabilityId::ValidateGraph,
            Self::ExecuteGraph(_) => CapabilityId::ExecuteGraph,
            Self::SaveGraph(_) => CapabilityId::SaveGraph,
            Self::ListGraphResults(_) => CapabilityId::ListGraphResults,
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
            Self::SearchKnowledge(request) => request.validate(),
            Self::ReadKnowledge(_) => Ok(()),
            Self::InspectDatasetSchema(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
            Self::InspectDatasetProfile(request) => {
                validate_resource_id("databaseId", &request.database_id)
            }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CapabilityContractError {
    #[error("invalid field '{0}'")]
    InvalidField(&'static str),
    #[error("field '{field}' exceeds {maximum} bytes")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("result limit must be between 1 and {maximum}")]
    InvalidLimit { maximum: u16 },
}

impl CapabilityContractError {
    pub fn into_failure(self, capability: CapabilityId) -> CapabilityFailure {
        let failure = CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
            .with_detail("capabilityId", capability.as_str());
        match self {
            Self::InvalidField(field) => failure.with_detail("field", field),
            Self::FieldTooLong { field, maximum } => failure
                .with_detail("field", field)
                .with_detail("maximumBytes", maximum.to_string()),
            Self::InvalidLimit { maximum } => failure
                .with_detail(
                    "field",
                    match capability {
                        CapabilityId::ApplyGraphEdit | CapabilityId::EditResource => "operations",
                        _ => "limit",
                    },
                )
                .with_detail("maximumResults", maximum.to_string())
                .with_detail(
                    "expected",
                    format!("An item count between 1 and {maximum}."),
                ),
        }
    }
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
    KnowledgeSearch(crate::KnowledgeSearchResult),
    KnowledgePassage(crate::KnowledgePassage),
    DatasetSchemaInspection(DatasetSchemaInspection),
    DatasetProfileInspection(DatasetProfileInspection),
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
            .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?;
        if bytes.len() > maximum_bytes {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::ResultTooLarge,
            ));
        }
        Ok(())
    }
    pub fn accepts_capability(&self, capability: CapabilityId) -> bool {
        let expected = match self {
            Self::ResourceManaged(_) => {
                return matches!(
                    capability,
                    CapabilityId::CreateResource
                        | CapabilityId::ImportDatabase
                        | CapabilityId::RenameResource
                        | CapabilityId::DuplicateResource
                        | CapabilityId::DeleteResource
                        | CapabilityId::SaveResource
                );
            }
            Self::ChartInspection(_) => CapabilityId::InspectChart,
            Self::ResourceInspection(_) => CapabilityId::InspectResource,
            Self::DatabaseRead(value) => value.content.capability_id(),
            Self::MindRead(value) => value.content.capability_id(),
            Self::DocumentRead(value) => value.content.capability_id(),
            Self::ResourceEdited(_) => {
                return matches!(
                    capability,
                    CapabilityId::UpdateChart
                        | CapabilityId::EditResource
                        | CapabilityId::ReplaceDocumentText
                        | CapabilityId::AppendDocument
                        | CapabilityId::WriteDocument
                        | CapabilityId::CreateTopics
                        | CapabilityId::UpdateTopics
                        | CapabilityId::MoveTopics
                        | CapabilityId::DeleteTopics
                        | CapabilityId::DuplicateTopics
                        | CapabilityId::InsertRows
                        | CapabilityId::UpdateCells
                        | CapabilityId::DeleteRows
                        | CapabilityId::CreateColumns
                        | CapabilityId::RenameColumns
                        | CapabilityId::DeleteColumns
                        | CapabilityId::CastColumns
                        | CapabilityId::SetColumnSemantics
                        | CapabilityId::UndoResource
                        | CapabilityId::RedoResource
                );
            }
            Self::DatabaseExported(_) => CapabilityId::ExportDatabase,
            Self::GraphInspection(_) => CapabilityId::InspectGraph,
            Self::GraphInspectionPage(_) => {
                return matches!(
                    capability,
                    CapabilityId::InspectGraph
                        | CapabilityId::FindConstants
                        | CapabilityId::InspectConstants
                        | CapabilityId::FindNodes
                        | CapabilityId::InspectNodes
                        | CapabilityId::FindConnections
                );
            }
            Self::NodeCatalogPage(_) => CapabilityId::BrowseNodes,
            Self::NodeTypeInspection(_) => CapabilityId::InspectNodeType,
            Self::KnowledgeSearch(_) => CapabilityId::SearchKnowledge,
            Self::KnowledgePassage(_) => CapabilityId::ReadKnowledge,
            Self::DatasetSchemaInspection(_) => CapabilityId::InspectDatasetSchema,
            Self::DatasetProfileInspection(_) => CapabilityId::InspectDatasetProfile,
            Self::ResultInspection(_) => CapabilityId::InspectResult,
            Self::ResultTablePage(_) => CapabilityId::ReadResultTable,
            Self::ProjectInspection(_) => CapabilityId::ListResources,
            Self::GraphEditReceipt(_) => {
                return matches!(
                    capability,
                    CapabilityId::ApplyGraphEdit
                        | CapabilityId::CreateConstants
                        | CapabilityId::UpdateConstants
                        | CapabilityId::DeleteConstants
                        | CapabilityId::CreateNodes
                        | CapabilityId::UpdateNodes
                        | CapabilityId::DeleteNodes
                        | CapabilityId::DuplicateNodes
                        | CapabilityId::MoveNodes
                        | CapabilityId::CreateConnections
                        | CapabilityId::UpdateConnections
                        | CapabilityId::DeleteConnections
                );
            }
            Self::GraphValidation(_) => CapabilityId::ValidateGraph,
            Self::GraphExecution(_) => CapabilityId::ExecuteGraph,
            Self::GraphSaved(_) => CapabilityId::SaveGraph,
            Self::GraphResults(_) => CapabilityId::ListGraphResults,
            Self::UiIntentInspection(_) => CapabilityId::InspectUiIntent,
            Self::UiIntentReceipt(_) => CapabilityId::RequestUiIntent,
        };
        expected == capability
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
    use crate::model::*;
    let mut schema = match capability_id {
        CapabilityId::InspectChart => schemars::schema_for!(InspectChartInput),
        CapabilityId::UpdateChart => schemars::schema_for!(UpdateChartInput),
        CapabilityId::InspectResource => schemars::schema_for!(crate::model::InspectResourceInput),
        CapabilityId::CreateResource => schemars::schema_for!(CreateResourceInput),
        CapabilityId::InspectDatabase => schemars::schema_for!(InspectDatabaseInput),
        CapabilityId::InspectDatabaseSchema => schemars::schema_for!(InspectDatabaseSchemaInput),
        CapabilityId::ProfileDatabase => schemars::schema_for!(ProfileDatabaseInput),
        CapabilityId::ReadDatabaseRows => schemars::schema_for!(ReadDatabaseRowsInput),
        CapabilityId::InspectDocument => schemars::schema_for!(InspectDocumentInput),
        CapabilityId::ReadDocument => schemars::schema_for!(ReadDocumentInput),
        CapabilityId::SearchDocument => schemars::schema_for!(SearchDocumentInput),
        CapabilityId::ReplaceDocumentText => schemars::schema_for!(ReplaceDocumentTextInput),
        CapabilityId::AppendDocument => schemars::schema_for!(AppendDocumentInput),
        CapabilityId::WriteDocument => schemars::schema_for!(WriteDocumentInput),

        CapabilityId::InspectMind => schemars::schema_for!(InspectMindInput),
        CapabilityId::FindTopics => schemars::schema_for!(FindTopicsInput),
        CapabilityId::InspectTopics => schemars::schema_for!(InspectTopicsInput),
        CapabilityId::CreateTopics => schemars::schema_for!(CreateTopicsInput),
        CapabilityId::UpdateTopics => schemars::schema_for!(UpdateTopicsInput),
        CapabilityId::MoveTopics => schemars::schema_for!(MoveTopicsInput),
        CapabilityId::DeleteTopics => schemars::schema_for!(DeleteTopicsInput),
        CapabilityId::DuplicateTopics => schemars::schema_for!(DuplicateTopicsInput),

        CapabilityId::ImportDatabase => schemars::schema_for!(ImportDatabaseInput),
        CapabilityId::InsertRows => schemars::schema_for!(InsertRowsInput),
        CapabilityId::UpdateCells => schemars::schema_for!(UpdateCellsInput),
        CapabilityId::DeleteRows => schemars::schema_for!(DeleteRowsInput),
        CapabilityId::CreateColumns => schemars::schema_for!(CreateColumnsInput),
        CapabilityId::RenameColumns => schemars::schema_for!(RenameColumnsInput),
        CapabilityId::DeleteColumns => schemars::schema_for!(DeleteColumnsInput),
        CapabilityId::CastColumns => schemars::schema_for!(CastColumnsInput),
        CapabilityId::SetColumnSemantics => schemars::schema_for!(SetColumnSemanticsInput),
        CapabilityId::RenameResource => schemars::schema_for!(RenameResourceInput),
        CapabilityId::DuplicateResource => schemars::schema_for!(DuplicateResourceInput),
        CapabilityId::DeleteResource
        | CapabilityId::SaveResource
        | CapabilityId::UndoResource
        | CapabilityId::RedoResource => {
            schemars::schema_for!(ResourceTargetInput)
        }
        CapabilityId::EditResource => schemars::schema_for!(EditResourceInput),
        CapabilityId::ExportDatabase => schemars::schema_for!(ExportDatabaseInput),
        CapabilityId::InspectUiIntent => {
            schemars::schema_for!(yss_ui_contract::InspectUiIntentRequest)
        }
        CapabilityId::RequestUiIntent => schemars::schema_for!(RequestUiIntentInput),
        CapabilityId::InspectGraph => schemars::schema_for!(InspectGraphInput),
        CapabilityId::BrowseNodes => schemars::schema_for!(BrowseNodesRequest),
        CapabilityId::InspectNodeType => schemars::schema_for!(crate::InspectNodeTypeRequest),
        CapabilityId::FindNodes => schemars::schema_for!(crate::FindNodesRequest),
        CapabilityId::FindConstants => schemars::schema_for!(crate::FindConstantsRequest),
        CapabilityId::InspectConstants => schemars::schema_for!(crate::InspectConstantsRequest),
        CapabilityId::CreateConstants => schemars::schema_for!(crate::CreateConstantsInput),
        CapabilityId::UpdateConstants => schemars::schema_for!(crate::UpdateConstantsInput),
        CapabilityId::DeleteConstants => schemars::schema_for!(crate::DeleteConstantsInput),

        CapabilityId::InspectNodes => schemars::schema_for!(crate::InspectNodesRequest),
        CapabilityId::FindConnections => schemars::schema_for!(crate::FindConnectionsRequest),
        CapabilityId::SearchKnowledge => schemars::schema_for!(crate::SearchKnowledgeRequest),
        CapabilityId::ReadKnowledge => schemars::schema_for!(crate::ReadKnowledgeRequest),
        CapabilityId::InspectDatasetSchema => return false.into(),
        CapabilityId::InspectDatasetProfile => return false.into(),
        CapabilityId::InspectResult => schemars::schema_for!(InspectResultRequest),
        CapabilityId::ReadResultTable => schemars::schema_for!(crate::ReadResultTableRequest),
        CapabilityId::ListResources => schemars::schema_for!(ListResourcesRequest),
        CapabilityId::ApplyGraphEdit | CapabilityId::SaveGraph => return false.into(),
        CapabilityId::CreateNodes => schemars::schema_for!(CreateNodesInput),
        CapabilityId::UpdateNodes => schemars::schema_for!(UpdateNodesInput),
        CapabilityId::DeleteNodes => schemars::schema_for!(DeleteNodesInput),
        CapabilityId::DuplicateNodes => schemars::schema_for!(DuplicateNodesInput),
        CapabilityId::MoveNodes => schemars::schema_for!(MoveNodesInput),
        CapabilityId::CreateConnections => schemars::schema_for!(CreateConnectionsInput),
        CapabilityId::UpdateConnections => schemars::schema_for!(UpdateConnectionsInput),
        CapabilityId::DeleteConnections => schemars::schema_for!(DeleteConnectionsInput),
        CapabilityId::ValidateGraph => schemars::schema_for!(ValidateGraphInput),
        CapabilityId::ExecuteGraph => schemars::schema_for!(ExecuteGraphInput),
        CapabilityId::ListGraphResults => schemars::schema_for!(ListGraphResultsRequest),
    };
    if matches!(
        capability_id,
        CapabilityId::UndoResource | CapabilityId::RedoResource
    ) {
        schema
            .get_mut("$defs")
            .and_then(|definitions| definitions.get_mut("ProjectResourceKind"))
            .expect("resource target schema defines its resource kinds")["enum"] =
            serde_json::json!(["event_graph", "function_graph", "database"]);
    }
    if capability_id == CapabilityId::EditResource {
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
