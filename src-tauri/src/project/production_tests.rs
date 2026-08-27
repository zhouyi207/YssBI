use super::*;
use crate::graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicPortBinding, GraphRevision, InputState,
    NodeId, NodePosition, OrderKey, ParameterValues, PortAddress, PortInstanceId,
};
use crate::node_system::document::{
    ClipboardNodeCreationDto, ClipboardNodeDto, ClipboardNodeId, ClipboardSubgraphDto,
    DocumentError, EditorGraphMutationDto, GraphDocumentOperation, GraphDocumentPatch,
    GraphMutation, HistoryMutation, MutationConflict, MutationRequest, ResourceKey,
};
use crate::node_system::protocol::{NodeTypeId, PortKey};
use crate::node_system::runtime::NOOP_RUN_EVENT_SINK;
use crate::project::{OperationId, ResourceRevision};

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Production.yssbi-event").unwrap()
}

fn document_path() -> crate::graph_document::GraphResourcePath {
    crate::graph_document::GraphResourcePath::new(graph_path().as_str()).unwrap()
}

fn current_project_instance_id(state: &ProjectState) -> ProjectInstanceId {
    ProjectInstanceId::from_existing(state.project_instance_id())
}

fn load_graph(
    state: &ProjectState,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceDocument, ProjectFilesystemError> {
    let project_instance_id = state.capture_project_session()?.instance_id;
    state.load_graph_resource(&project_instance_id, graph_path, 1)
}

fn node(node_type: &str) -> DocumentNode {
    DocumentNode {
        id: crate::graph_document::NodeId::new(),
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: crate::graph_document::NodePosition { x: 10.0, y: 20.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

#[derive(Default)]
struct DemandRunEvents(std::sync::Mutex<Vec<crate::node_system::runtime::RunEvent>>);

impl crate::node_system::runtime::RunEventSink for DemandRunEvents {
    fn record(&self, event: crate::node_system::runtime::RunEvent) {
        self.0.lock().unwrap().push(event);
    }
}

struct ActivatedProjectState(crate::project::fixtures::TempProject);

impl std::ops::Deref for ActivatedProjectState {
    type Target = ProjectState;

    fn deref(&self) -> &Self::Target {
        self.0.state()
    }
}

fn state_with_empty_graph() -> ActivatedProjectState {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "production-empty-graph",
        ProjectData::new(),
    ));
    state
        .insert_graph(
            graph_path(),
            GraphResourceDocument::new("Production", GraphDocumentKind::Event),
        )
        .unwrap();
    state
}

fn create_node_mutation() -> EditorGraphMutationDto {
    EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
        },
        position: crate::graph_document::NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
        connect_from: None,
    }
}

fn test_variable(name: &str) -> crate::variable::VariableInstance {
    let id = crate::variable::VariableId::new();
    crate::variable::VariableInstance {
        id,
        name: name.into(),
        data_type: crate::data_contract::DataType::Int64,
        data_value: crate::data_contract::DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    }
}

fn state_with_project_path(label: &str) -> (ProjectState, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    (state, root)
}

fn graph_with_dangling_endpoint(
    name: &str,
    kind: GraphDocumentKind,
    missing_node_id: NodeId,
) -> GraphResourceDocument {
    let existing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x200));
    let connection_id = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x201));
    let mut resource = GraphResourceDocument::new(name, kind);
    resource.document.nodes.insert(
        existing_node_id,
        DocumentNode {
            id: existing_node_id,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: crate::graph_document::NodePosition { x: 10.0, y: 20.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    resource.document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(missing_node_id, PortKey::new("value").unwrap()),
            input: PortAddress::declared(existing_node_id, PortKey::new("value").unwrap()),
            order: None,
        },
    );
    resource
}

fn document_error_source(error: &ProjectFilesystemError) -> Option<&DocumentError> {
    std::error::Error::source(error).and_then(|source| source.downcast_ref::<DocumentError>())
}

fn insert_uncached_duckdb_declaration(state: &ProjectState, path: &str) {
    state.project_data.write().unwrap().databases.insert(
        "missing".into(),
        crate::database_contract::DatabaseDecl {
            id: crate::database_contract::DatabaseId::from_existing("missing".into()),
            engine: crate::database_contract::DatabaseEngine::DuckDb {
                path: path.into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: "Missing".into(),
        },
    );
}

fn editor_mutation_request(
    base_revision: GraphRevision,
    operation_id: OperationId,
) -> MutationRequest<EditorGraphMutationDto> {
    MutationRequest::new(
        ResourceKey::Graph(document_path()),
        ResourceRevision::from_graph_revision(base_revision),
        operation_id,
        create_node_mutation(),
    )
}

fn function_state_with_caller(
    label: &str,
) -> (
    ActivatedProjectState,
    GraphResourcePath,
    GraphResourcePath,
    ResourceKey,
) {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        label,
        ProjectData::new(),
    ));
    let function_path =
        GraphResourcePath::new(format!("functions/{label}.yssbi-function")).unwrap();
    let caller_path = GraphResourcePath::new(format!("events/{label}Caller.yssbi-event")).unwrap();
    state
        .insert_graph(
            function_path.clone(),
            GraphResourceDocument::new(label, GraphDocumentKind::Function),
        )
        .unwrap();
    let mut caller =
        GraphResourceDocument::new(format!("{label} Caller"), GraphDocumentKind::Event);
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    caller.document.nodes.insert(call.id, call);
    state.insert_graph(caller_path.clone(), caller).unwrap();
    let resource = ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
        function_path.as_str().into(),
    ));
    (state, function_path, caller_path, resource)
}

fn test_signature() -> crate::node_system::document::FunctionSignature {
    crate::node_system::document::FunctionSignature {
        parameters: Vec::new(),
        return_type: Some("Float64".into()),
    }
}

fn function_signature_request(
    resource: ResourceKey,
    base_revision: GraphRevision,
    before: crate::node_system::document::FunctionSignature,
    after: crate::node_system::document::FunctionSignature,
) -> MutationRequest<crate::node_system::document::FunctionDocumentPatch> {
    MutationRequest::new(
        resource,
        ResourceRevision::from_graph_revision(base_revision),
        OperationId::new(),
        crate::node_system::document::FunctionDocumentPatch::new(before, after),
    )
}

fn durable_unloaded_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    String,
    GraphResourcePath,
    ResourceKey,
    crate::graph_document::NodeId,
) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new(format!("events/{label}.yssbi-event")).unwrap();
    let resource = ResourceKey::Graph(graph_path.clone());
    let inserted_node = node("yssbi.constant.int64");
    let inserted_node_id = inserted_node.id;
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new(label, GraphDocumentKind::Event),
    );
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &graph_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    state
        .apply_graph_patch(
            &graph_path,
            MutationRequest::new(
                resource.clone(),
                ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: inserted_node,
                }]),
            ),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &graph_path).unwrap();
    state.unload_graph_resource(&graph_path).unwrap();
    (
        state,
        root,
        root_text,
        graph_path,
        resource,
        inserted_node_id,
    )
}

pub(super) fn durable_graph_global_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    GraphResourcePath,
    ResourceKey,
    crate::variable::VariableId,
) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new(format!("events/{label}.yssbi-event")).unwrap();
    let function_path =
        GraphResourcePath::new(format!("functions/{label}Observer.yssbi-function")).unwrap();
    let graph_key = graph_path.clone();
    let graph_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node("yssbi.constant.int64"),
    }]);
    let before_variable = test_variable("Before global History");
    let mut after_variable = before_variable.clone();
    after_variable.name = "After global History".into();
    let variable_id = before_variable.id;
    let variable_key = crate::node_system::document::VariableResourceKey(
        format!("variables/{variable_id}").into(),
    );
    let variable_patch = crate::node_system::document::VariableDocumentPatch::new(
        Some(serde_json::to_value(&before_variable).unwrap()),
        Some(serde_json::to_value(&after_variable).unwrap()),
    );
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new(label, GraphDocumentKind::Event),
    );
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new(format!("{label}Observer"), GraphDocumentKind::Function),
    );
    project.variables.insert(variable_id, before_variable);
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &graph_path).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &function_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
        OperationId::new(),
        vec![
            crate::node_system::document::ResourcePatch::graph(
                graph_key.clone(),
                GraphRevision::INITIAL,
                graph_patch,
            ),
            crate::node_system::document::ResourcePatch::variable(
                variable_key,
                ResourceRevision::INITIAL,
                variable_patch,
            ),
        ],
    );
    {
        let mut data = state.project_data.write().unwrap();
        let mut revisions = state.variable_revisions.write().unwrap();
        let mut documents = super::project_state::project_documents(&data, &revisions);
        let mut history = crate::node_system::document::ProjectHistory::default();
        history
            .apply_transaction(&mut documents, transaction)
            .unwrap();
        super::project_state::replace_project_documents(&mut data, &mut revisions, documents);
        state
            .graph_revisions
            .write()
            .unwrap()
            .insert(graph_path.clone(), GraphRevision::new(1));
        *state.history.write().unwrap() = history;
    }
    crate::project::fixtures::write_project(&state.get_data().unwrap(), &root_text).unwrap();
    crate::project::fixtures::write_state_graph(&state, &graph_path).unwrap();
    state.unload_graph_resource(&graph_path).unwrap();
    (
        state,
        root,
        graph_path,
        ResourceKey::Graph(graph_key),
        variable_id,
    )
}

fn publish_empty_replacement_hook(
    state: &ProjectState,
    root: &std::path::Path,
) -> std::sync::Arc<dyn Fn() + Send + Sync> {
    let replacement_state = state.clone();
    let replacement_root =
        NormalizedProjectRoot::from_project_path(root.to_string_lossy().into_owned()).unwrap();
    let first = std::sync::atomic::AtomicBool::new(true);
    std::sync::Arc::new(move || {
        if !first.swap(false, std::sync::atomic::Ordering::AcqRel) {
            return;
        }
        replacement_state
            .publish_project_activation_without_test_hooks(
                PreparedProjectActivation::from_data(
                    Some(replacement_root.clone()),
                    ProjectData::new(),
                    None,
                    false,
                )
                .unwrap(),
            )
            .unwrap()
            .dispose();
    })
}

fn assert_empty_replacement_authority(state: &ProjectState, stale_project: &ProjectInstanceId) {
    let data = state.project_data.read().unwrap();
    assert!(data.graphs.is_empty());
    assert!(data.variables.is_empty());
    assert!(data.databases.is_empty());
    assert!(data.worksheets.is_empty());
    drop(data);
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: false,
        }
    );
    assert_eq!(state.history_lengths_for_test(), (0, 0));
    assert_eq!(state.history_head_id_for_test(true), None);
    assert_eq!(state.history_head_id_for_test(false), None);
    let revisions = state.revision_state_for_test();
    assert!(revisions.0.is_empty());
    assert!(revisions.1.is_empty());
    assert!(revisions.2.is_empty());
    let publication = state.publication_state_for_test();
    assert_ne!(publication.0, stale_project.as_str());
    assert_eq!((publication.1, publication.2), (0, 0));
}

fn active_state_with_empty_graph(label: &str) -> (ProjectState, std::path::PathBuf) {
    let (state, root) = state_with_project_path(label);
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    state
        .insert_graph(
            graph_path(),
            GraphResourceDocument::new("Production", GraphDocumentKind::Event),
        )
        .unwrap();
    (state, root)
}

fn temp_project_with_empty_graph(label: &str) -> crate::project::fixtures::TempProject {
    let project = crate::project::fixtures::TempProject::activate(label, ProjectData::new());
    project
        .state()
        .insert_graph(
            graph_path(),
            GraphResourceDocument::new("Production", GraphDocumentKind::Event),
        )
        .unwrap();
    project
}

fn active_state_with_valid_constant_graph(label: &str) -> (ProjectState, std::path::PathBuf) {
    let (state, root) = active_state_with_empty_graph(label);
    let mut constant = node("yssbi.constant.int64");
    constant.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(7),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: constant,
                }]),
            ),
        )
        .unwrap();
    (state, root)
}

#[cfg(test)]
mod compilation;
#[cfg(test)]
mod editor_mutations;
#[cfg(test)]
mod execution;
#[cfg(test)]
mod execution_relational;
#[cfg(test)]
mod graph_projection;
#[cfg(test)]
mod graph_resources;
#[cfg(test)]
mod history;
#[cfg(test)]
mod history_residency;
#[cfg(test)]
mod history_worksheets;
#[cfg(test)]
mod lifecycle;
#[cfg(test)]
mod persistence;
#[cfg(test)]
mod projection;
#[cfg(test)]
mod subgraph;
#[cfg(test)]
mod variables;
#[cfg(test)]
mod worksheets;
