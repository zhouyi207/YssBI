use super::*;
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentError, DocumentNode, EditorGraphMutationDto,
    GraphDocumentOperation, GraphDocumentPatch, GraphMutation, GraphRevision, HistoryMutation,
    MutationConflict, MutationRequest, NodeId, OperationId, ParameterValues, PortAddress,
    ResourceKey,
};
use crate::node_system::protocol::{NodeTypeId, PortKey};
use crate::node_system::runtime::NOOP_RUN_EVENT_SINK;

#[test]
fn execution_error_retains_typed_internal_compilation_failure() {
    let failure = crate::node_system::compiler::InternalCompilationFailure {
        stage: crate::node_system::compiler::CompilationStage::Lowering,
        code: "compiler.lowering.internal_invariant".into(),
        node_id: Some(NodeId::from_uuid(uuid::Uuid::from_u128(42))),
    };

    let error = ProjectExecutionError::internal_compilation(failure.clone());

    assert_eq!(error.internal_compilation_failure(), Some(&failure));
    assert!(error.run_error().is_none());
}

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Production.yssbi-event").unwrap()
}

fn document_path() -> crate::node_system::document::GraphResourcePath {
    crate::node_system::document::GraphResourcePath(graph_path().as_str().into())
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
        id: crate::node_system::document::NodeId::new(),
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
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
        position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
    }
}

fn test_variable(name: &str) -> crate::variable::VariableInstance {
    let id = crate::variable::VariableId::new();
    crate::variable::VariableInstance {
        id,
        name: name.into(),
        data_type: crate::graph::value::DataType::Int64,
        data_value: crate::graph::value::DataValue::Int64(1),
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
            position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
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

#[test]
fn structurally_invalid_insert_graph_has_zero_authoritative_effects() {
    let state = state_with_empty_graph();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    assert!(coordinator.contains_slot_for_test(&document_path()));
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let before_history = state.history_status();
    let before_history_head = state.history_head_id_for_test(true);
    let before_history_lengths = state.history_lengths_for_test();
    let missing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x202));

    let error = state
        .insert_graph(
            graph_path(),
            graph_with_dangling_endpoint(
                "Invalid Replacement",
                GraphDocumentKind::Event,
                missing_node_id,
            ),
        )
        .unwrap_err();

    assert!(matches!(
        &error,
        ProjectFilesystemError::InvalidGraphDocument { path, source }
            if path == &graph_path()
                && source == &DocumentError::EndpointNodeNotFound(missing_node_id)
    ));
    assert_eq!(
        document_error_source(&error),
        Some(&DocumentError::EndpointNodeNotFound(missing_node_id))
    );
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_head_id_for_test(true), before_history_head);
    assert_eq!(state.history_lengths_for_test(), before_history_lengths);
    assert!(coordinator.contains_slot_for_test(&document_path()));
}

#[test]
fn structurally_invalid_resource_patch_insert_has_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("invalid-resource-patch");
    state
        .insert_graph(
            graph_path(),
            GraphResourceDocument::new("Production", GraphDocumentKind::Event),
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    assert!(coordinator.contains_slot_for_test(&document_path()));
    let invalid_path = GraphResourcePath::new("events/Invalid.yssbi-event").unwrap();
    let invalid_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        invalid_path.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::from_uuid(uuid::Uuid::from_u128(0x203)),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: [invalid_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let before_history = state.history_status();
    let before_history_head = state.history_head_id_for_test(true);
    let before_history_lengths = state.history_lengths_for_test();
    let completion_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completion_for_hook = std::sync::Arc::clone(&completion_observed);
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        completion_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));
    let missing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x204));

    let error = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::InsertGraph {
                path: invalid_path.clone(),
                resource: graph_with_dangling_endpoint(
                    "Invalid",
                    GraphDocumentKind::Event,
                    missing_node_id,
                ),
            },
        )
        .unwrap_err();

    assert!(matches!(
        &error,
        ProjectFilesystemError::InvalidGraphDocument { path, source }
            if path == &invalid_path
                && source == &DocumentError::EndpointNodeNotFound(missing_node_id)
    ));
    assert_eq!(
        document_error_source(&error),
        Some(&DocumentError::EndpointNodeNotFound(missing_node_id))
    );
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&invalid_path));
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_head_id_for_test(true), before_history_head);
    assert_eq!(state.history_lengths_for_test(), before_history_lengths);
    assert!(!completion_observed.load(std::sync::atomic::Ordering::Acquire));
    assert!(coordinator.contains_slot_for_test(&document_path()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn structurally_invalid_move_graph_moved_has_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("invalid-move-moved");
    let from = graph_path();
    let to = GraphResourcePath::new("events/Moved.yssbi-event").unwrap();
    let source = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    state.insert_graph(from.clone(), source.clone()).unwrap();
    state.graph_projection(&from, "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    assert!(coordinator.contains_slot_for_test(&document_path()));
    let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        from.as_str().into(),
    ));
    let destination_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        to.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::from_uuid(uuid::Uuid::from_u128(0x205)),
        affected_resources: vec![source_key.clone()],
        expected_revisions: [(source_key, GraphRevision::INITIAL)].into_iter().collect(),
        expected_absent_resources: [destination_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let before_history = state.history_status();
    let before_history_head = state.history_head_id_for_test(true);
    let before_history_lengths = state.history_lengths_for_test();
    let completion_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completion_for_hook = std::sync::Arc::clone(&completion_observed);
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        completion_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));
    let missing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x206));

    let error = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: source,
                moved: graph_with_dangling_endpoint(
                    "Moved",
                    GraphDocumentKind::Event,
                    missing_node_id,
                ),
                referenced_graphs_before: Default::default(),
                referenced_graphs: Default::default(),
                loaded_referenced_graphs: Default::default(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap_err();

    assert!(matches!(
        &error,
        ProjectFilesystemError::InvalidGraphDocument { path, source }
            if path == &to
                && source == &DocumentError::EndpointNodeNotFound(missing_node_id)
    ));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    assert!(state.get_data().unwrap().graphs.contains_key(&from));
    assert!(!state.get_data().unwrap().graphs.contains_key(&to));
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_head_id_for_test(true), before_history_head);
    assert_eq!(state.history_lengths_for_test(), before_history_lengths);
    assert!(!completion_observed.load(std::sync::atomic::Ordering::Acquire));
    assert!(coordinator.contains_slot_for_test(&document_path()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn structurally_invalid_move_graph_referenced_graphs_have_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("invalid-move-references");
    let from = graph_path();
    let to = GraphResourcePath::new("events/Moved.yssbi-event").unwrap();
    let referenced_path = GraphResourcePath::new("events/Referenced.yssbi-event").unwrap();
    let source = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let referenced_before = GraphResourceDocument::new("Referenced", GraphDocumentKind::Event);
    state.insert_graph(from.clone(), source.clone()).unwrap();
    state
        .insert_graph(referenced_path.clone(), referenced_before.clone())
        .unwrap();
    state.graph_projection(&from, "en-US").unwrap();
    state.graph_projection(&referenced_path, "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    let referenced_document_path =
        crate::node_system::document::GraphResourcePath(referenced_path.as_str().into());
    assert!(coordinator.contains_slot_for_test(&document_path()));
    assert!(coordinator.contains_slot_for_test(&referenced_document_path));
    let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        from.as_str().into(),
    ));
    let referenced_key = ResourceKey::Graph(referenced_document_path.clone());
    let destination_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        to.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::from_uuid(uuid::Uuid::from_u128(0x207)),
        affected_resources: vec![source_key.clone(), referenced_key.clone()],
        expected_revisions: [
            (source_key, GraphRevision::INITIAL),
            (referenced_key, GraphRevision::INITIAL),
        ]
        .into_iter()
        .collect(),
        expected_absent_resources: [destination_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let before_history = state.history_status();
    let before_history_head = state.history_head_id_for_test(true);
    let before_history_lengths = state.history_lengths_for_test();
    let completion_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completion_for_hook = std::sync::Arc::clone(&completion_observed);
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        completion_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));
    let missing_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x208));

    let error = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: source,
                moved: GraphResourceDocument::new("Moved", GraphDocumentKind::Event),
                referenced_graphs_before: [(referenced_path.clone(), referenced_before)]
                    .into_iter()
                    .collect(),
                referenced_graphs: [(
                    referenced_path.clone(),
                    graph_with_dangling_endpoint(
                        "Referenced",
                        GraphDocumentKind::Event,
                        missing_node_id,
                    ),
                )]
                .into_iter()
                .collect(),
                loaded_referenced_graphs: [referenced_path.clone()].into_iter().collect(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap_err();

    assert!(matches!(
        &error,
        ProjectFilesystemError::InvalidGraphDocument { path, source }
            if path == &referenced_path
                && source == &DocumentError::EndpointNodeNotFound(missing_node_id)
    ));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    assert!(state.get_data().unwrap().graphs.contains_key(&from));
    assert!(!state.get_data().unwrap().graphs.contains_key(&to));
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_head_id_for_test(true), before_history_head);
    assert_eq!(state.history_lengths_for_test(), before_history_lengths);
    assert!(!completion_observed.load(std::sync::atomic::Ordering::Acquire));
    assert!(coordinator.contains_slot_for_test(&document_path()));
    assert!(coordinator.contains_slot_for_test(&referenced_document_path));
    std::fs::remove_dir_all(root).unwrap();
}

fn insert_uncached_duckdb_declaration(state: &ProjectState, path: &str) {
    state.project_data.write().unwrap().databases.insert(
        "missing".into(),
        crate::database::DatabaseDecl {
            id: "missing".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: path.into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: Some("Missing".into()),
        },
    );
}

fn editor_mutation_request(
    base_revision: GraphRevision,
    operation_id: OperationId,
) -> MutationRequest<EditorGraphMutationDto> {
    MutationRequest::new(
        ResourceKey::Graph(document_path()),
        base_revision,
        operation_id,
        create_node_mutation(),
    )
}

fn resource_descriptor_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    crate::variable::VariableId,
) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    let variable = test_variable("Catalog variable");
    let variable_id = variable.id;
    let mut data = ProjectData::new();
    data.variables.insert(variable_id, variable);
    data.graphs.insert(
        graph_path(),
        GraphResourceDocument::new("Production", GraphDocumentKind::Event),
    );
    crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
    crate::project::fixtures::write_graph(&data, root.to_string_lossy().as_ref(), &graph_path())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
    (state, root, variable_id)
}

fn resource_descriptor_request(
    variable_id: crate::variable::VariableId,
) -> MutationRequest<EditorGraphMutationDto> {
    MutationRequest::new(
        ResourceKey::Graph(document_path()),
        GraphRevision::INITIAL,
        OperationId::new(),
        EditorGraphMutationDto::CreateNode {
            descriptor: crate::node_system::catalog::NodeCreationDescriptor::ResourceBound {
                node_type_id: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
                resource_path: crate::node_system::catalog::CatalogResourcePath::new(format!(
                    "variables/{variable_id}"
                )),
                resource_revision: GraphRevision::INITIAL,
                create_args: crate::node_system::catalog::ResourceBoundCreateArgsDto::Variable,
            },
            position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
            user_label: None,
        },
    )
}

struct ResourceDescriptorMatrixFixture {
    state: ProjectState,
    root: std::path::PathBuf,
    function_path: GraphResourcePath,
    variable_id: crate::variable::VariableId,
    out_of_scope_variable_id: crate::variable::VariableId,
    database_id: String,
}

fn resource_descriptor_matrix_fixture(label: &str) -> ResourceDescriptorMatrixFixture {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    let function_path = GraphResourcePath::new("functions/Helper.yssbi-function").unwrap();
    let owner_path = GraphResourcePath::new("functions/Owner.yssbi-function").unwrap();
    let variable = test_variable("Catalog variable");
    let variable_id = variable.id;
    let mut out_of_scope = test_variable("Scoped variable");
    let out_of_scope_variable_id = out_of_scope.id;
    out_of_scope.scope = crate::variable::VariableScope::Function {
        function_path: owner_path.as_str().into(),
    };
    let database_id = "sales / . # 数据".to_string();

    let mut data = ProjectData::new();
    data.variables.insert(variable_id, variable);
    data.variables
        .insert(out_of_scope_variable_id, out_of_scope);
    for (path, name, kind) in [
        (graph_path(), "Production", GraphDocumentKind::Event),
        (function_path.clone(), "Helper", GraphDocumentKind::Function),
        (owner_path, "Owner", GraphDocumentKind::Function),
    ] {
        data.graphs
            .insert(path, GraphResourceDocument::new(name, kind));
    }
    data.databases.insert(
        database_id.clone(),
        crate::database::DatabaseDecl {
            id: database_id.clone(),
            engine: crate::database::DatabaseEngine::InMemory {
                name: database_id.clone(),
            },
            schema_version: 1,
            required: false,
            name: Some("Sales".into()),
        },
    );
    crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
    for path in data.graphs.keys() {
        crate::project::fixtures::write_graph(&data, root.to_string_lossy().as_ref(), path)
            .unwrap();
    }
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
    ResourceDescriptorMatrixFixture {
        state,
        root,
        function_path,
        variable_id,
        out_of_scope_variable_id,
        database_id,
    }
}

fn resource_descriptor_matrix_request(
    base_revision: GraphRevision,
    node_type_id: &str,
    resource_path: impl Into<Box<str>>,
    resource_revision: GraphRevision,
    create_args: crate::node_system::catalog::ResourceBoundCreateArgsDto,
) -> MutationRequest<EditorGraphMutationDto> {
    MutationRequest::new(
        ResourceKey::Graph(document_path()),
        base_revision,
        OperationId::new(),
        EditorGraphMutationDto::CreateNode {
            descriptor: crate::node_system::catalog::NodeCreationDescriptor::ResourceBound {
                node_type_id: NodeTypeId::new(node_type_id).unwrap(),
                resource_path: crate::node_system::catalog::CatalogResourcePath::new(resource_path),
                resource_revision,
                create_args,
            },
            position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
            user_label: None,
        },
    )
}

#[derive(Debug, PartialEq)]
struct ResourceDescriptorEffects {
    document_revision: GraphRevision,
    graph_revision_ledger: Option<GraphRevision>,
    history: crate::node_system::document::HistoryStatusDto,
    publication_project_instance_id: String,
    resource_publication_revision: u64,
    authority_generation: u64,
    project_data: serde_json::Value,
    filesystem: std::collections::BTreeMap<String, Option<Vec<u8>>>,
}

fn resource_descriptor_effects(
    state: &ProjectState,
    root: &std::path::Path,
) -> ResourceDescriptorEffects {
    fn collect(
        root: &std::path::Path,
        current: &std::path::Path,
        entries: &mut std::collections::BTreeMap<String, Option<Vec<u8>>>,
    ) {
        for entry in std::fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                entries.insert(relative, None);
                collect(root, &path, entries);
            } else {
                entries.insert(relative, Some(std::fs::read(path).unwrap()));
            }
        }
    }

    let data = state.get_data().unwrap();
    let document_revision = data.graphs[&graph_path()].document.revision;
    let graph_revision_ledger = state
        .revision_state_for_test()
        .0
        .get(&graph_path())
        .copied();
    let (publication_project_instance_id, resource_publication_revision, authority_generation) = {
        let publication = state.mutation_publication.lock().unwrap();
        (
            publication.project_instance_id.clone(),
            publication.resource_revision,
            publication.authority_generation(),
        )
    };
    let history = state.history.read().unwrap().status();
    let project_data = serde_json::to_value(data).unwrap();
    let mut filesystem = std::collections::BTreeMap::new();
    collect(root, root, &mut filesystem);
    ResourceDescriptorEffects {
        document_revision,
        graph_revision_ledger,
        history,
        publication_project_instance_id,
        resource_publication_revision,
        authority_generation,
        project_data,
        filesystem,
    }
}

#[test]
fn resource_descriptor_publication_accepts_function_variable_and_database_paths() {
    use crate::node_system::catalog::ResourceBoundCreateArgsDto;
    use crate::node_system::protocol::ParameterKey;

    let fixture = resource_descriptor_matrix_fixture("resource-descriptor-valid-matrix");
    let database_path = format!("databases/{}", fixture.database_id);
    let project_instance_id = fixture.state.capture_project_session().unwrap().instance_id;
    let catalog = fixture
        .state
        .catalog_snapshot(&project_instance_id)
        .unwrap();
    assert!(catalog.resources.iter().any(|resource| {
        resource.resource_path.as_str() == database_path
            && resource.node_type_id.as_str() == "yssbi.dataframe.source.get"
    }));
    let validation = fixture
        .state
        .catalog_mutation_validation_snapshot(&project_instance_id)
        .unwrap();
    assert!(validation.resources.contains_key(
        &crate::node_system::catalog::CatalogResourcePath::new(database_path.clone())
    ));
    let events = std::cell::Cell::new(0_u32);
    let cases = [
        (
            "yssbi.project.function.call",
            fixture.function_path.as_str().to_string(),
            ResourceBoundCreateArgsDto::Function,
            "target",
        ),
        (
            "yssbi.project.variable.get",
            format!("variables/{}", fixture.variable_id),
            ResourceBoundCreateArgsDto::Variable,
            "variable",
        ),
        (
            "yssbi.dataframe.source.get",
            database_path,
            ResourceBoundCreateArgsDto::Database,
            "dataframe",
        ),
    ];

    for (index, (node_type, path, create_args, parameter)) in cases.into_iter().enumerate() {
        fixture
            .state
            .apply_editor_graph_mutation_observed(
                &project_instance_id,
                &graph_path(),
                "en-US",
                resource_descriptor_matrix_request(
                    GraphRevision::new(index as u64),
                    node_type,
                    path.clone(),
                    GraphRevision::INITIAL,
                    create_args,
                ),
                |_| events.set(events.get() + 1),
            )
            .unwrap();
        let data = fixture.state.get_data().unwrap();
        let node = data.graphs[&graph_path()]
            .document
            .nodes
            .values()
            .find(|node| node.node_type.as_str() == node_type)
            .unwrap();
        assert_eq!(
            node.parameters[&ParameterKey::new(parameter).unwrap()],
            serde_json::json!(path)
        );
    }

    assert_eq!(events.get(), 3);
    assert_eq!(
        fixture.state.get_data().unwrap().graphs[&graph_path()]
            .document
            .revision,
        GraphRevision::new(3)
    );
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn resource_descriptor_rejections_have_zero_publication_effects() {
    use crate::node_system::catalog::ResourceBoundCreateArgsDto;

    let cases = [
        ("wrong-tuple", "catalog_descriptor_invalid"),
        ("malformed-path", "catalog_descriptor_invalid"),
        ("stale-revision", "catalog_resource_stale"),
        ("missing-resource", "catalog_resource_stale"),
        ("out-of-scope", "catalog_descriptor_invalid"),
    ];

    for (case, expected_code) in cases {
        let fixture = resource_descriptor_matrix_fixture(case);
        let request = match case {
            "wrong-tuple" => resource_descriptor_matrix_request(
                GraphRevision::INITIAL,
                "yssbi.project.variable.get",
                fixture.function_path.as_str(),
                GraphRevision::INITIAL,
                ResourceBoundCreateArgsDto::Function,
            ),
            "malformed-path" => resource_descriptor_matrix_request(
                GraphRevision::INITIAL,
                "yssbi.project.function.call",
                r"functions\Helper.yssbi-function",
                GraphRevision::INITIAL,
                ResourceBoundCreateArgsDto::Function,
            ),
            "stale-revision" => resource_descriptor_matrix_request(
                GraphRevision::INITIAL,
                "yssbi.project.function.call",
                fixture.function_path.as_str(),
                GraphRevision::new(1),
                ResourceBoundCreateArgsDto::Function,
            ),
            "missing-resource" => resource_descriptor_matrix_request(
                GraphRevision::INITIAL,
                "yssbi.project.function.call",
                "functions/Missing.yssbi-function",
                GraphRevision::INITIAL,
                ResourceBoundCreateArgsDto::Function,
            ),
            "out-of-scope" => resource_descriptor_matrix_request(
                GraphRevision::INITIAL,
                "yssbi.project.variable.get",
                format!("variables/{}", fixture.out_of_scope_variable_id),
                GraphRevision::INITIAL,
                ResourceBoundCreateArgsDto::Variable,
            ),
            _ => unreachable!(),
        };
        let before = resource_descriptor_effects(&fixture.state, &fixture.root);
        let event_count = std::cell::Cell::new(0_u32);

        let error = fixture
            .state
            .apply_editor_graph_mutation_observed(
                &fixture.state.capture_project_session().unwrap().instance_id,
                &graph_path(),
                "en-US",
                request,
                |_| event_count.set(event_count.get() + 1),
            )
            .unwrap_err();

        assert_eq!(error.code(), expected_code, "case: {case}");
        assert_eq!(event_count.get(), 0, "case: {case}");
        assert_eq!(
            resource_descriptor_effects(&fixture.state, &fixture.root),
            before,
            "case: {case}"
        );
        std::fs::remove_dir_all(fixture.root).unwrap();
    }
}

#[test]
fn resource_bound_editor_mutation_preserves_stale_lifecycle_before_catalog_observer() {
    let (state, root, variable_id) =
        resource_descriptor_fixture("resource-descriptor-stale-caller");
    let stale_id = state.capture_project_session().unwrap().instance_id;
    let replacement_root = std::env::temp_dir().join(format!(
        "yssbi-resource-descriptor-stale-replacement-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&replacement_root).unwrap();
    let mut replacement = ProjectData::new();
    replacement.graphs.insert(
        graph_path(),
        GraphResourceDocument::new("Production", GraphDocumentKind::Event),
    );
    state.activate_project_fixture(replacement_root.to_string_lossy().into_owned(), replacement);
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before = state.history_status();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();
    let catalog_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let catalog_observed_by_hook = std::sync::Arc::clone(&catalog_observed);
    state.set_catalog_mutation_before_publication_test_hook(std::sync::Arc::new(move || {
        catalog_observed_by_hook.store(true, std::sync::atomic::Ordering::Release);
    }));
    let mutation_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mutation_observed_by_callback = std::sync::Arc::clone(&mutation_observed);

    let error = state
        .apply_editor_graph_mutation_observed(
            &stale_id,
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
            move |_| {
                mutation_observed_by_callback.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    assert!(!catalog_observed.load(std::sync::atomic::Ordering::Acquire));
    assert!(!mutation_observed.load(std::sync::atomic::Ordering::Acquire));
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(replacement_root).unwrap();
}

#[test]
fn resource_bound_editor_mutation_classifies_in_snapshot_authority_drift_as_catalog_stale() {
    let (state, root, variable_id) =
        resource_descriptor_fixture("resource-descriptor-in-snapshot-drift");
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let normalized_root = NormalizedProjectRoot::from_project_path(&root).unwrap();
    let held_lease = state.filesystem().acquire(normalized_root).unwrap();
    let catalog_waiting = state.filesystem().observe_next_wait();
    let catalog_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let catalog_observed_by_hook = std::sync::Arc::clone(&catalog_observed);
    state.set_catalog_mutation_before_publication_test_hook(std::sync::Arc::new(move || {
        catalog_observed_by_hook.store(true, std::sync::atomic::Ordering::Release);
    }));
    let mutation_observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mutation_observed_by_callback = std::sync::Arc::clone(&mutation_observed);
    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        mutation_state.apply_editor_graph_mutation_observed(
            &project_instance_id,
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
            move |_| {
                mutation_observed_by_callback.store(true, std::sync::atomic::Ordering::Release);
            },
        )
    });
    catalog_waiting
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("resource-bound mutation must wait for the held catalog root lease");
    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    let data_before_release = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before_release = state.history_status();
    let revisions_before_release = state.revision_state_for_test();
    let publication_before_release = state.publication_state_for_test();
    drop(held_lease);

    let error = mutation.join().unwrap().unwrap_err();

    assert_eq!(error.code(), "catalog_resource_stale");
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before_release
    );
    assert_eq!(state.history_status(), history_before_release);
    assert_eq!(state.revision_state_for_test(), revisions_before_release);
    assert_eq!(
        state.publication_state_for_test(),
        publication_before_release
    );
    assert!(!catalog_observed.load(std::sync::atomic::Ordering::Acquire));
    assert!(!mutation_observed.load(std::sync::atomic::Ordering::Acquire));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_descriptor_publication_materializes_exact_variable_binding() {
    let (state, root, variable_id) = resource_descriptor_fixture("resource-descriptor-valid");

    let result = state
        .apply_editor_graph_mutation(
            &state.capture_project_session().unwrap().instance_id,
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
        )
        .unwrap();

    assert_eq!(result.delta.from_revision, GraphRevision::INITIAL);
    assert_eq!(result.delta.to_revision, GraphRevision::new(1));
    let data = state.get_data().unwrap();
    let node = data.graphs[&graph_path()]
        .document
        .nodes
        .values()
        .next()
        .unwrap();
    assert_eq!(node.node_type.as_str(), "yssbi.project.variable.get");
    assert_eq!(
        node.parameters[&crate::node_system::protocol::ParameterKey::new("variable").unwrap()],
        serde_json::json!(format!("variables/{variable_id}"))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_descriptor_authority_change_after_snapshot_has_zero_mutation_effects() {
    let (state, root, variable_id) = resource_descriptor_fixture("resource-descriptor-authority");
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let graph_before = state.get_data().unwrap().graphs[&graph_path()]
        .document
        .clone();
    let history_before = state.history_status();
    let generation_before = state.authority_generation_for_test();
    let hook_state = state.clone();
    state.set_catalog_mutation_before_publication_test_hook(std::sync::Arc::new(move || {
        hook_state
            .mutation_publication
            .lock()
            .unwrap()
            .advance_authority_generation();
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_by_callback = std::sync::Arc::clone(&observed);

    let error = state
        .apply_editor_graph_mutation_observed(
            &project_instance_id,
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
            move |_| {
                observed_by_callback.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "catalog_resource_stale");
    assert_eq!(
        state.get_data().unwrap().graphs[&graph_path()].document,
        graph_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.authority_generation_for_test(), generation_before + 1);
    assert!(!observed.load(std::sync::atomic::Ordering::Acquire));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_descriptor_project_replacement_after_snapshot_has_zero_mutation_effects() {
    let (state, root, variable_id) = resource_descriptor_fixture("resource-descriptor-project");
    let original_instance = state.capture_project_session().unwrap().instance_id;
    let replacement_root = std::env::temp_dir().join(format!(
        "yssbi-resource-descriptor-replacement-{}",
        uuid::Uuid::new_v4()
    ));
    crate::project::fixtures::write_project(
        &ProjectData::new(),
        replacement_root.to_string_lossy().as_ref(),
    )
    .unwrap();
    let replacement_state = state.clone();
    let replacement_path = replacement_root.to_string_lossy().into_owned();
    state.set_catalog_mutation_before_publication_test_hook(std::sync::Arc::new(move || {
        replacement_state.activate_project_fixture(replacement_path.clone(), ProjectData::new());
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_by_callback = std::sync::Arc::clone(&observed);

    let error = state
        .apply_editor_graph_mutation_observed(
            &original_instance,
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
            move |_| {
                observed_by_callback.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert_ne!(
        state.capture_project_session().unwrap().instance_id,
        original_instance
    );
    assert!(state.get_data().unwrap().graphs.is_empty());
    assert_eq!(state.history_status(), Default::default());
    assert!(!observed.load(std::sync::atomic::Ordering::Acquire));
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(replacement_root).unwrap();
}

#[test]
fn narrow_graph_move_patch_preserves_unrelated_concurrent_mutation() {
    let (state, root) = state_with_project_path("narrow-graph-move");
    let from = GraphResourcePath::new("events/Before.yssbi-event").unwrap();
    let to = GraphResourcePath::new("events/After.yssbi-event").unwrap();
    let mut moved = GraphResourceDocument::new("After", GraphDocumentKind::Event);
    let original = GraphResourceDocument::new("Before", GraphDocumentKind::Event);
    state.insert_graph(from.clone(), original.clone());
    let session = state.capture_project_session().unwrap();
    let resource = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        from.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session,
        operation_id: OperationId::new(),
        affected_resources: vec![resource.clone()],
        expected_revisions: [(resource, GraphRevision::INITIAL)].into_iter().collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let concurrent = test_variable("Concurrent");
    let concurrent_id = concurrent.id;
    state
        .project_data
        .write()
        .unwrap()
        .variables
        .insert(concurrent_id, concurrent.clone());
    moved.document.revision = GraphRevision::new(1);

    state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: original,
                moved: moved.clone(),
                referenced_graphs_before: Default::default(),
                referenced_graphs: Default::default(),
                loaded_referenced_graphs: Default::default(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap();

    let data = state.get_data().unwrap();
    assert!(!data.graphs.contains_key(&from));
    assert_eq!(data.graphs[&to], moved);
    assert_eq!(data.variables[&concurrent_id].name, "Concurrent");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rename_environment_failure_never_changes_filesystem_targets() {
    let (state, root) = state_with_project_path("rename-environment-failure");
    let source = state
        .create_graph_resource_fixture("Before", GraphDocumentKind::Event)
        .unwrap();
    let source_file = root.join(source.as_str());
    let source_before = std::fs::read(&source_file).unwrap();
    let target_file = root.join("events/After.yssbi-event");
    insert_uncached_duckdb_declaration(&state, "database/missing.duckdb");

    state.set_project_filesystem_rollback_fault(true);
    let result =
        state.rename_graph_resource_fixture(&state.project_instance_id(), &source, "After");
    state.set_project_filesystem_rollback_fault(false);

    assert!(result.is_err());
    assert_eq!(std::fs::read(&source_file).unwrap(), source_before);
    assert!(!target_file.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_upsert_environment_failure_never_changes_filesystem_target() {
    let (upsert_state, upsert_root) =
        state_with_project_path("worksheet-upsert-environment-failure");
    let invalid_database = upsert_root.join("database/invalid.duckdb");
    std::fs::create_dir_all(invalid_database.parent().unwrap()).unwrap();
    std::fs::write(&invalid_database, b"not a DuckDB database").unwrap();
    insert_uncached_duckdb_declaration(&upsert_state, "database/invalid.duckdb");
    let worksheet = WorksheetDocument::new("Uncommitted", "database");
    let worksheet_file = upsert_root.join(crate::project::worksheet_relative_path(&worksheet));

    upsert_state.set_project_filesystem_rollback_fault(true);
    let upsert_result = upsert_state.upsert_worksheet_document(worksheet.clone());
    upsert_state.set_project_filesystem_rollback_fault(false);

    assert!(upsert_result.is_err());
    assert!(!worksheet_file.exists());
    assert!(
        !upsert_state
            .project_data
            .read()
            .unwrap()
            .worksheets
            .contains_key(&worksheet.id)
    );
    std::fs::remove_dir_all(upsert_root).unwrap();
}

#[test]
fn worksheet_removal_environment_failure_never_changes_filesystem_target() {
    let (remove_state, remove_root) =
        state_with_project_path("worksheet-remove-environment-failure");
    let invalid_database = remove_root.join("database/invalid.duckdb");
    std::fs::create_dir_all(invalid_database.parent().unwrap()).unwrap();
    std::fs::write(&invalid_database, b"not a DuckDB database").unwrap();
    let worksheet = WorksheetDocument::new("Preserved", "database");
    remove_state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet.id.clone(), worksheet.clone());
    remove_state.initialize_worksheet_revision_for_test(&worksheet.id);
    crate::project::fixtures::write_worksheet(&remove_root, &worksheet).unwrap();
    insert_uncached_duckdb_declaration(&remove_state, "database/invalid.duckdb");
    let worksheet_file = remove_root.join(crate::project::worksheet_relative_path(&worksheet));
    let worksheet_before = std::fs::read(&worksheet_file).unwrap();

    remove_state.set_project_filesystem_rollback_fault(true);
    let remove_result = remove_state.remove_worksheet_document(&worksheet.id);
    remove_state.set_project_filesystem_rollback_fault(false);

    assert!(remove_result.is_err());
    assert_eq!(std::fs::read(&worksheet_file).unwrap(), worksheet_before);
    assert!(
        remove_state
            .project_data
            .read()
            .unwrap()
            .worksheets
            .contains_key(&worksheet.id)
    );
    std::fs::remove_dir_all(remove_root).unwrap();
}

#[test]
fn destination_appearance_rejects_graph_move_without_authoritative_effects() {
    let (state, root) = state_with_project_path("destination-conflict");
    let from = GraphResourcePath::new("events/Source.yssbi-event").unwrap();
    let to = GraphResourcePath::new("events/Destination.yssbi-event").unwrap();
    let source = GraphResourceDocument::new("Source", GraphDocumentKind::Event);
    state.insert_graph(from.clone(), source.clone());
    let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        from.as_str().into(),
    ));
    let destination_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        to.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![source_key.clone()],
        expected_revisions: [(source_key, GraphRevision::INITIAL)].into_iter().collect(),
        expected_absent_resources: [destination_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    state.insert_graph(
        to.clone(),
        GraphResourceDocument::new("Concurrent", GraphDocumentKind::Event),
    );

    let error = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: source.clone(),
                moved: source,
                referenced_graphs_before: Default::default(),
                referenced_graphs: Default::default(),
                loaded_referenced_graphs: Default::default(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert!(state.get_data().unwrap().graphs.contains_key(&from));
    assert_eq!(state.get_data().unwrap().graphs[&to].name, "Concurrent");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_required_gate_blocks_project_authority_until_activation() {
    let (state, root) = state_with_project_path("recovery-authority-gate");
    let graph = graph_path();
    let resource = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    state.insert_graph(graph.clone(), resource).unwrap();
    let worksheet = WorksheetDocument::new("Recovery", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet.id.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet.id);
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    state
        .project_recovery_marker()
        .mark("injected recovery requirement");

    assert!(matches!(
        state.apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph,
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    assert!(matches!(
        state.update_function_signature_observed(
            &current_project_instance_id(&state),
            &graph,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    graph.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                Default::default(),
            ),
            |_| {},
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    assert!(matches!(
        state.undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    assert!(
        state
            .graph_projection(&graph, "en-US")
            .unwrap_err()
            .contains("project_recovery_required")
    );
    let error = load_graph(&state, &graph).unwrap_err();
    assert_eq!(error.code(), "project_recovery_required");
    assert!(error.recovery_required());
    assert_eq!(
        state
            .insert_graph(
                GraphResourcePath::new("events/Blocked.yssbi-event").unwrap(),
                GraphResourceDocument::new("Blocked", GraphDocumentKind::Event),
            )
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state.unload_graph_resource(&graph).unwrap_err().code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .rename_graph_resource_fixture(&state.project_instance_id(), &graph, "Blocked")
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .load_worksheet_document(&context.session.instance_id, &worksheet.id)
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state.worksheet_creation_snapshot().unwrap_err().code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .apply_resource_document_patch(
                &context,
                ResourceDocumentPatch::RemoveWorksheet {
                    id: worksheet.id.clone(),
                },
            )
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );

    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    state
        .insert_graph(
            graph.clone(),
            GraphResourceDocument::new("Recovered", GraphDocumentKind::Event),
        )
        .unwrap();
    assert!(state.graph_projection(&graph, "en-US").is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn worksheet_revision_conflict_has_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("worksheet-revision-conflict");
    let worksheet = WorksheetDocument::new("Original", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet.id.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet.id);
    let key = ResourceKey::Worksheet(crate::node_system::document::WorksheetResourceKey(
        worksheet.id.clone().into(),
    ));
    let stale = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![key.clone()],
        expected_revisions: [(key.clone(), GraphRevision::INITIAL)]
            .into_iter()
            .collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let current = ProjectTransactionContext {
        session: stale.session.clone(),
        operation_id: OperationId::new(),
        affected_resources: vec![key.clone()],
        expected_revisions: [(key, GraphRevision::INITIAL)].into_iter().collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let mut concurrent = worksheet.clone();
    concurrent.name = "Concurrent".into();
    state
        .apply_resource_document_patch(
            &current,
            ResourceDocumentPatch::UpsertWorksheet {
                id: concurrent.id.clone(),
                document: concurrent,
            },
        )
        .unwrap();
    let mut stale_document = worksheet.clone();
    stale_document.name = "Stale".into();

    let error = state
        .apply_resource_document_patch(
            &stale,
            ResourceDocumentPatch::UpsertWorksheet {
                id: stale_document.id.clone(),
                document: stale_document,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        state.get_data().unwrap().worksheets[&worksheet.id].name,
        "Concurrent"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_worksheet_save_is_rejected_without_disk_or_authoritative_effects() {
    let (state, root) = state_with_project_path("worksheet-stale-save");
    let worksheet = WorksheetDocument::new("Original", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet.id.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet.id);
    crate::project::fixtures::write_worksheet(&root, &worksheet).unwrap();
    let mut current = worksheet.clone();
    current.name = "Current".into();
    state.upsert_worksheet_document(current).unwrap();
    let mut stale = worksheet.clone();
    stale.name = "Stale".into();

    let error = state.upsert_worksheet_document(stale).unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        state.get_data().unwrap().worksheets[&worksheet.id].name,
        "Current"
    );
    assert_eq!(
        crate::project::load_worksheet_from_file(&root, &worksheet.id)
            .unwrap()
            .name,
        "Current"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unwind_rollback_failure_blocks_mutations_until_activation() {
    let (state, root) = state_with_project_path("recovery-boundary");
    let worksheet = WorksheetDocument::new("Blocked Read", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet.id.clone(), worksheet.clone());
    let session = state.capture_project_session().unwrap();
    let context = ProjectTransactionContext {
        session: session.clone(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let lease = state.filesystem().acquire(session.root.clone()).unwrap();
    let prepared = ProjectFilesystemTransaction::prepare_with_validator(
        context.clone(),
        lease,
        vec![StagedFilesystemMutation::Write {
            relative_path: "recovery.json".into(),
            contents: br#"{"changed":true}"#.to_vec(),
        }],
        |_, _| Ok(()),
    )
    .unwrap();
    let committed = prepared.commit().unwrap();
    state.set_project_filesystem_rollback_fault(true);
    drop(committed);

    let blocked = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::InsertGraph {
                path: GraphResourcePath::new("events/Blocked.yssbi-event").unwrap(),
                resource: GraphResourceDocument::new("Blocked", GraphDocumentKind::Event),
            },
        )
        .unwrap_err();
    assert_eq!(blocked.code(), "project_recovery_required");
    assert!(blocked.recovery_required());
    let blocked_read = state
        .load_worksheet_document(&context.session.instance_id, &worksheet.id)
        .unwrap_err();
    assert_eq!(blocked_read.code(), "project_recovery_required");

    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let fresh = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    state
        .apply_resource_document_patch(
            &fresh,
            ResourceDocumentPatch::InsertGraph {
                path: GraphResourcePath::new("events/Recovered.yssbi-event").unwrap(),
                resource: GraphResourceDocument::new("Recovered", GraphDocumentKind::Event),
            },
        )
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_required_blocks_authoritative_entry_points_until_activation() {
    let state = ProjectState::new();
    let event = GraphResourcePath::new("events/Recovery.yssbi-event").unwrap();
    let function = GraphResourcePath::new("functions/Recovery.yssbi-function").unwrap();
    state.insert_graph(
        event.clone(),
        GraphResourceDocument::new("Recovery", GraphDocumentKind::Event),
    );
    state.insert_graph(
        function.clone(),
        GraphResourceDocument::new("Recovery", GraphDocumentKind::Function),
    );
    state.project_recovery_marker().mark("rollback failed");
    let mut observed = 0;

    let editor_error = state
        .apply_editor_graph_mutation_observed(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &event,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    event.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                create_node_mutation(),
            ),
            |_| observed += 1,
        )
        .unwrap_err();
    assert_eq!(editor_error.code(), "project_recovery_required");

    let function_error = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    function.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    Default::default(),
                ),
            ),
            |_| observed += 1,
        )
        .unwrap_err();
    assert_eq!(function_error.code(), "project_recovery_required");

    for error in [
        state.undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    event.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        ),
        state.redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    event.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        ),
    ] {
        assert_eq!(error.unwrap_err().code(), "project_recovery_required");
    }
    assert!(
        state
            .graph_projection(&event, "en-US")
            .unwrap_err()
            .contains("project_recovery_required")
    );
    let error = load_graph(&state, &event).unwrap_err();
    assert_eq!(error.code(), "project_recovery_required");
    assert!(error.recovery_required());
    assert_eq!(
        state
            .insert_graph(
                GraphResourcePath::new("events/Blocked.yssbi-event").unwrap(),
                GraphResourceDocument::new("Blocked", GraphDocumentKind::Event),
            )
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state.unload_graph_resource(&event).unwrap_err().code(),
        "project_recovery_required"
    );
    assert_eq!(observed, 0);
    assert!(
        state
            .execute_graph_for_current_project_for_test(
                &event,
                &crate::node_system::plan::ExecutionDemand::Default,
                &NOOP_RUN_EVENT_SINK
            )
            .unwrap_err()
            .contains("project_recovery_required")
    );
    assert!(
        state
            .with_database_mut(
                &crate::project::ProjectInstanceId::from_existing("blocked".into()),
                "missing",
                GraphRevision::INITIAL,
                OperationId::new(),
                |_| Ok(()),
            )
            .unwrap_err()
            .contains("project_recovery_required")
    );
    assert_eq!(
        state
            .result_source_descriptor(crate::node_system::runtime::ResultSourceId::new(1))
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .release_run_result_sources(crate::node_system::analysis::RunId::new(1))
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert!(
        state
            .project_data
            .read()
            .unwrap()
            .graphs
            .contains_key(&event)
    );

    state.activate_project_fixture("recovered".into(), ProjectData::new());
    assert!(state.ensure_project_operational().is_ok());
}

#[test]
fn rename_remains_committed_when_project_replacement_runs_during_receipt_completion() {
    let (state, root) = state_with_project_path("rename-replacement-after-publication");
    let source = state
        .create_graph_resource_fixture("Before", GraphDocumentKind::Event)
        .unwrap();
    let target = GraphResourcePath::new("events/After.yssbi-event").unwrap();
    let replacement_state = state.clone();
    let receipt_completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let receipt_completed_for_hook = std::sync::Arc::clone(&receipt_completed);
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        receipt_completed_for_hook.store(true, std::sync::atomic::Ordering::Release);
        replacement_state.activate_project_fixture("replacement".into(), ProjectData::new());
    }));
    let project_instance_id = state.project_instance_id();

    let result = state
        .rename_graph_resource_fixture(&project_instance_id, &source, "After")
        .unwrap();

    assert!(receipt_completed.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(result.path, target);
    assert_eq!(result.publication.project_instance_id, project_instance_id);
    assert!(!root.join(source.as_str()).exists());
    assert!(root.join(target.as_str()).is_file());
    assert_eq!(state.graph_lifecycle_entry_count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rename_rejects_concurrent_referenced_graph_and_variable_changes() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-rename-touched-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let source = GraphResourcePath::new("events/Source.yssbi-event").unwrap();
    let caller = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    let source_document = GraphResourceDocument::new("Source", GraphDocumentKind::Event);
    let mut caller_document = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
    let mut reference = node("yssbi.test.reference");
    reference.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(source.as_str()),
    );
    caller_document
        .document
        .nodes
        .insert(reference.id, reference);
    let mut variable = test_variable("Scoped");
    variable.scope = crate::variable::VariableScope::Event {
        event_path: source.as_str().into(),
    };
    let variable_id = variable.id;
    let mut project = ProjectData::new();
    project
        .graphs
        .insert(source.clone(), source_document.clone());
    project
        .graphs
        .insert(caller.clone(), caller_document.clone());
    project.variables.insert(variable_id, variable);
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    for path in [&source, &caller] {
        crate::project::fixtures::write_graph(&project, root.to_string_lossy().as_ref(), path)
            .unwrap();
    }
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let concurrent_state = state.clone();
    let source_for_hook = source.clone();
    let caller_for_hook = caller.clone();
    state.set_graph_rename_io_checkpoint(std::sync::Arc::new(move || {
        let mut data = concurrent_state.project_data.write().unwrap();
        let source = data.graphs.get_mut(&source_for_hook).unwrap();
        source.name = "Concurrent Source".into();
        source.document.revision = GraphRevision::new(1);
        let caller = data.graphs.get_mut(&caller_for_hook).unwrap();
        caller.name = "Concurrent Caller".into();
        caller.document.revision = GraphRevision::new(1);
        drop(data);
        concurrent_state.variable_revisions.write().unwrap().insert(
            variable_id,
            crate::project::project_state::VariableRevisionEntry::present(GraphRevision::new(1)),
        );
    }));
    let project_instance_id = state.project_instance_id();

    let error = state
        .rename_graph_resource_fixture(&project_instance_id, &source, "Renamed")
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        state.get_data().unwrap().graphs[&source].name,
        "Concurrent Source"
    );
    assert_eq!(
        state.get_data().unwrap().graphs[&caller].name,
        "Concurrent Caller"
    );
    assert_eq!(
        state.variable_revisions.read().unwrap()[&variable_id].revision,
        GraphRevision::new(1)
    );
    assert!(root.join(source.as_str()).is_file());
    assert!(!root.join("events/Renamed.yssbi-event").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_transaction_context_has_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("stale-transaction");
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let before = state.get_data().unwrap();
    let inserted_path = GraphResourcePath::new("events/Stale.yssbi-event").unwrap();

    let error = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::InsertGraph {
                path: inserted_path.clone(),
                resource: GraphResourceDocument::new("Stale", GraphDocumentKind::Event),
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert_eq!(state.get_data().unwrap().graphs, before.graphs);
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&inserted_path)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_patch_preserves_unrelated_concurrent_project_data() {
    let (state, root) = state_with_project_path("worksheet-patch");
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let concurrent = test_variable("Concurrent Worksheet Variable");
    let concurrent_id = concurrent.id;
    state
        .project_data
        .write()
        .unwrap()
        .variables
        .insert(concurrent_id, concurrent);
    let worksheet = WorksheetDocument::new("Authoritative", "database");

    state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::UpsertWorksheet {
                id: worksheet.id.clone(),
                document: worksheet.clone(),
            },
        )
        .unwrap();

    let data = state.get_data().unwrap();
    assert_eq!(data.worksheets[&worksheet.id].name, "Authoritative");
    assert_eq!(
        data.variables[&concurrent_id].name,
        "Concurrent Worksheet Variable"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn editor_mutation_returns_correlated_delta_projection_and_history_status() {
    let state = state_with_empty_graph();
    let operation_id = OperationId::new();
    let request = editor_mutation_request(GraphRevision::INITIAL, operation_id);

    let result = state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            request,
        )
        .unwrap();

    assert_eq!(result.delta.caused_by, Some(operation_id));
    assert_eq!(result.delta.from_revision, GraphRevision::INITIAL);
    assert_eq!(
        result.delta.to_revision.get(),
        result.projection_replacement.projection.source_revision
    );
    assert!(result.history.can_undo);
    assert!(!result.history.can_redo);
}

#[test]
fn stale_editor_mutation_rejects_without_consuming_history() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();
    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );

    let error = state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::StaleRevision { .. }));
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );
}

#[test]
fn undo_redo_return_atomic_replacements_and_current_history_status() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(undo.deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo.projection_replacements.len(), 1);
    assert_eq!(
        undo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![graph_path().as_str().to_string()],
        }
    );
    assert_eq!(
        undo.projection_replacements[0].projection.source_revision,
        2
    );
    assert_eq!(
        undo.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(redo.deltas[0].to_revision, GraphRevision::new(3));
    assert_eq!(redo.projection_replacements.len(), 1);
    assert_eq!(
        redo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![graph_path().as_str().to_string()],
        }
    );
    assert_eq!(
        redo.projection_replacements[0].projection.source_revision,
        3
    );
    assert_eq!(
        redo.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
}

#[test]
fn graph_cache_unload_preserves_complete_project_history() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-history-cache-unload-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let unloaded = graph_path();
    let retained = GraphResourcePath::new("events/Retained.yssbi-event").unwrap();
    let mut local_variable = test_variable("Unloaded local");
    local_variable.scope = crate::variable::VariableScope::Event {
        event_path: unloaded.as_str().into(),
    };
    let local_variable_id = local_variable.id;
    let mut project = ProjectData::new();
    project.graphs.insert(
        unloaded.clone(),
        GraphResourceDocument::new("Production", GraphDocumentKind::Event),
    );
    project.graphs.insert(
        retained.clone(),
        GraphResourceDocument::new("Retained", GraphDocumentKind::Event),
    );
    project.variables.insert(local_variable_id, local_variable);
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &unloaded).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &retained).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);

    for path in [&unloaded, &retained] {
        state
            .apply_graph_patch(
                path,
                MutationRequest::new(
                    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        path.as_str().into(),
                    )),
                    GraphRevision::INITIAL,
                    OperationId::new(),
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                        node: node("yssbi.constant.int64"),
                    }]),
                ),
            )
            .unwrap();
    }
    crate::project::fixtures::write_state_graph(&state, &unloaded).unwrap();
    state.graph_projection(&unloaded, "en-US").unwrap();
    state.graph_projection(&retained, "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    let retained_document_path =
        crate::node_system::document::GraphResourcePath(retained.as_str().into());
    assert!(coordinator.contains_slot_for_test(&document_path()));
    assert!(coordinator.contains_slot_for_test(&retained_document_path));

    let before_status = state.history_status();
    let before_lengths = state.history_lengths_for_test();
    let before_head = state.history_head_id_for_test(true);
    let before_revisions = state.revision_state_for_test();
    let before_generation = state.authority_generation_for_test();
    assert_eq!(before_lengths, (2, 0));

    state.unload_graph_resource(&unloaded).unwrap();

    let data = state.get_data().unwrap();
    assert!(!data.graphs.contains_key(&unloaded));
    assert!(data.graphs.contains_key(&retained));
    assert!(!data.variables.contains_key(&local_variable_id));
    assert_eq!(state.history_status(), before_status);
    assert_eq!(state.history_lengths_for_test(), before_lengths);
    assert_eq!(state.history_head_id_for_test(true), before_head);
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.authority_generation_for_test(), before_generation + 1);
    assert!(!coordinator.contains_slot_for_test(&document_path()));
    assert!(coordinator.contains_slot_for_test(&retained_document_path));

    state.graph_projection(&retained, "en-US").unwrap();
    let before_noop_retained_slot = coordinator.contains_slot_for_test(&retained_document_path);
    assert!(before_noop_retained_slot);
    let before_noop_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_noop_status = state.history_status();
    let before_noop_lengths = state.history_lengths_for_test();
    let before_noop_head = state.history_head_id_for_test(true);
    let before_noop_revisions = state.revision_state_for_test();
    let before_noop_generation = state.authority_generation_for_test();

    state.unload_graph_resource(&unloaded).unwrap();

    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_noop_data
    );
    assert_eq!(state.history_status(), before_noop_status);
    assert_eq!(state.history_lengths_for_test(), before_noop_lengths);
    assert_eq!(state.history_head_id_for_test(true), before_noop_head);
    assert_eq!(state.revision_state_for_test(), before_noop_revisions);
    assert_eq!(
        state.authority_generation_for_test(),
        before_noop_generation
    );
    assert_eq!(
        coordinator.contains_slot_for_test(&retained_document_path),
        before_noop_retained_slot
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_history_preparation_hydrates_disk_without_loading_cache() {
    let root =
        std::env::temp_dir().join(format!("yssbi-history-hydration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/Hydrated.yssbi-event").unwrap();
    let document_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
    let mut local_variable = test_variable("Hydrated local");
    local_variable.scope = crate::variable::VariableScope::Event {
        event_path: graph_path.as_str().into(),
    };
    let local_variable_id = local_variable.id;
    let local_variable_key = crate::node_system::document::VariableResourceKey(
        format!("variables/{local_variable_id}").into(),
    );
    let inserted_node = node("yssbi.constant.int64");
    let inserted_node_id = inserted_node.id;
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Hydrated", GraphDocumentKind::Event),
    );
    project.variables.insert(local_variable_id, local_variable);
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &graph_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    state
        .apply_graph_patch(
            &graph_path,
            MutationRequest::new(
                ResourceKey::Graph(document_path.clone()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: inserted_node,
                }]),
            ),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &graph_path).unwrap();
    state.unload_graph_resource(&graph_path).unwrap();
    let before_history = state.history_status();
    let before_lengths = state.history_lengths_for_test();
    let before_head = state.history_head_id_for_test(true);
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let acquisition = state.filesystem().observe_acquire_many_attempts();
    crate::project::filesystem::reset_normalized_root_reconstruction_count_for_test();

    let prepared = state
        .prepare_history_for_test(
            true,
            MutationRequest::new(
                ResourceKey::Graph(document_path.clone()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
        )
        .unwrap();
    let acquired_roots = acquisition
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("History hydration must acquire the project root lease");

    assert_eq!(
        crate::project::filesystem::normalized_root_reconstruction_count_for_test(),
        0,
        "History preparation must clone the already-normalized active root"
    );
    assert_eq!(acquired_roots, vec![prepared.basis.session.root.clone()]);
    assert!(prepared.contains_unloaded_graph);
    assert_eq!(
        prepared.touched_graphs,
        std::collections::BTreeSet::from([graph_path.clone()])
    );
    assert!(
        prepared.before.graphs[&document_path]
            .nodes
            .contains_key(&inserted_node_id)
    );
    assert!(prepared.before.variables.contains_key(&local_variable_key));
    assert!(
        !prepared.after.graphs[&document_path]
            .nodes
            .contains_key(&inserted_node_id)
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    assert!(
        !state
            .get_data()
            .unwrap()
            .variables
            .contains_key(&local_variable_id)
    );
    assert_eq!(prepared.basis.history_id, before_head.unwrap());
    assert_eq!(
        prepared.basis.expected_revisions[&ResourceKey::Graph(document_path.clone())],
        GraphRevision::new(1)
    );
    drop(prepared);

    let graph_file = root.join(graph_path.as_str());
    std::fs::remove_file(&graph_file).unwrap();
    let missing_error = match state.prepare_history_for_test(
        true,
        MutationRequest::new(
            ResourceKey::Graph(document_path.clone()),
            GraphRevision::new(1),
            OperationId::new(),
            HistoryMutation {},
        ),
    ) {
        Ok(_) => panic!("missing graph hydration unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(matches!(missing_error, MutationConflict::History(_)));
    assert!(!graph_file.exists());
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_lengths_for_test(), before_lengths);
    assert_eq!(state.history_head_id_for_test(true), before_head);
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );

    let corrupt = b"{not valid graph json";
    std::fs::write(&graph_file, corrupt).unwrap();
    let corrupt_error = match state.prepare_history_for_test(
        true,
        MutationRequest::new(
            ResourceKey::Graph(document_path),
            GraphRevision::new(1),
            OperationId::new(),
            HistoryMutation {},
        ),
    ) {
        Ok(_) => panic!("corrupt graph hydration unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(matches!(corrupt_error, MutationConflict::History(_)));
    assert_eq!(std::fs::read(&graph_file).unwrap(), corrupt);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.history_lengths_for_test(), before_lengths);
    assert_eq!(state.history_head_id_for_test(true), before_head);
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn durable_unloaded_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    String,
    GraphResourcePath,
    ResourceKey,
    crate::node_system::document::NodeId,
) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new(format!("events/{label}.yssbi-event")).unwrap();
    let resource = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        graph_path.as_str().into(),
    ));
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
                GraphRevision::INITIAL,
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

#[derive(Clone, Copy)]
enum HistoryLifecycleReplacementCheckpoint {
    Preparation,
    Finalize,
}

fn assert_history_lifecycle_replacement_has_zero_effects(
    label: &str,
    checkpoint: HistoryLifecycleReplacementCheckpoint,
) {
    let (state, root, root_text, graph_path, resource, _) = durable_unloaded_history_fixture(label);
    let expected_project = state.capture_project_session().unwrap().instance_id;
    let graph_file = root.join(graph_path.as_str());
    let file_before = std::fs::read(&graph_file).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    let hook = std::sync::Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("bounded History lifecycle checkpoint release");
    });
    match checkpoint {
        HistoryLifecycleReplacementCheckpoint::Preparation => {
            state.set_history_after_preparation_test_hook(hook);
        }
        HistoryLifecycleReplacementCheckpoint::Finalize => {
            state.set_history_after_disk_commit_test_hook(hook);
        }
    }
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_observed = std::sync::Arc::clone(&observed);
    let worker_state = state.clone();
    let worker = std::thread::spawn(move || {
        worker_state.undo_last_transaction_observed(
            &expected_project,
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| worker_observed.store(true, std::sync::atomic::Ordering::SeqCst),
        )
    });

    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("History reached lifecycle checkpoint");
    let mut replacement = ProjectData::new();
    replacement.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Replacement", GraphDocumentKind::Event),
    );
    let replacement_root = NormalizedProjectRoot::from_project_path(root_text).unwrap();
    state
        .publish_project_activation_without_test_hooks(
            PreparedProjectActivation::from_data(Some(replacement_root), replacement, None, false)
                .unwrap(),
        )
        .unwrap()
        .dispose();
    let data_after_replacement = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let status_after_replacement = state.history_status();
    let lengths_after_replacement = state.history_lengths_for_test();
    let undo_head_after_replacement = state.history_head_id_for_test(true);
    let redo_head_after_replacement = state.history_head_id_for_test(false);
    let revisions_after_replacement = state.revision_state_for_test();
    let publication_after_replacement = state.publication_state_for_test();
    release_tx.send(()).unwrap();

    let error = worker.join().unwrap().unwrap_err();

    assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
    assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(std::fs::read(&graph_file).unwrap(), file_before);
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_after_replacement
    );
    assert_eq!(state.history_status(), status_after_replacement);
    assert_eq!(state.history_lengths_for_test(), lengths_after_replacement);
    assert_eq!(
        state.history_head_id_for_test(true),
        undo_head_after_replacement
    );
    assert_eq!(
        state.history_head_id_for_test(false),
        redo_head_after_replacement
    );
    assert_eq!(state.revision_state_for_test(), revisions_after_replacement);
    assert_eq!(
        state.publication_state_for_test(),
        publication_after_replacement
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn history_commands_reject_stale_project_identity_with_zero_effects_during_preparation() {
    assert_history_lifecycle_replacement_has_zero_effects(
        "HistoryLifecyclePreparation",
        HistoryLifecycleReplacementCheckpoint::Preparation,
    );
}

#[test]
fn history_commands_reject_stale_project_identity_with_zero_effects_before_final_commit() {
    assert_history_lifecycle_replacement_has_zero_effects(
        "HistoryLifecycleFinalize",
        HistoryLifecycleReplacementCheckpoint::Finalize,
    );
}

fn durable_graph_global_history_fixture(
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
    let graph_key = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
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
                GraphRevision::INITIAL,
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
    assert!(
        state
            .project_store
            .read()
            .unwrap()
            .variable_tabular
            .is_empty()
    );
}

fn file_snapshots(
    paths: impl IntoIterator<Item = std::path::PathBuf>,
) -> Vec<(std::path::PathBuf, Option<Vec<u8>>)> {
    paths
        .into_iter()
        .map(|path| {
            let contents = std::fs::read(&path).ok();
            (path, contents)
        })
        .collect()
}

fn assert_file_snapshots_unchanged(snapshots: &[(std::path::PathBuf, Option<Vec<u8>>)]) {
    for (path, expected) in snapshots {
        assert_eq!(
            std::fs::read(path).ok().as_ref(),
            expected.as_ref(),
            "{path:?}"
        );
    }
}

fn durable_variable_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    ResourceKey,
    crate::variable::VariableId,
) {
    let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let variable = test_variable(label);
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    (
        state,
        root,
        ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        )),
        variable.id,
    )
}

fn graph_move_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    GraphResourcePath,
    GraphResourcePath,
    GraphRevision,
) {
    let (state, root) = state_with_project_path(label);
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("Move Source", GraphDocumentKind::Event)
        .unwrap();
    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Move Target")
        .unwrap();
    let target = renamed.path;
    let revision = renamed
        .publication
        .deltas
        .iter()
        .find_map(|delta| match &delta.resource {
            ResourceKey::Graph(path) if path.0.as_ref() == target.as_str() => {
                Some(delta.to_revision)
            }
            _ => None,
        })
        .unwrap();
    (state, root, source, target, revision)
}

fn history_request(
    resource: ResourceKey,
    revision: GraphRevision,
) -> MutationRequest<HistoryMutation> {
    MutationRequest::new(resource, revision, OperationId::new(), HistoryMutation {})
}

fn run_history_direction(
    state: &ProjectState,
    project: &ProjectInstanceId,
    undo: bool,
    request: MutationRequest<HistoryMutation>,
    observed: &std::sync::atomic::AtomicBool,
) -> Result<crate::event::ResourceMutationResultDto, MutationConflict> {
    if undo {
        state.undo_last_transaction_observed(project, "en-US", request, |_| {
            observed.store(true, std::sync::atomic::Ordering::SeqCst)
        })
    } else {
        state.redo_last_transaction_observed(project, "en-US", request, |_| {
            observed.store(true, std::sync::atomic::Ordering::SeqCst)
        })
    }
}

#[test]
fn history_lifecycle_typing_rejects_projection_replacement_for_every_durable_policy_and_direction()
{
    for undo in [true, false] {
        let (state, root, root_text, graph_path, resource, _) =
            durable_unloaded_history_fixture(if undo {
                "ProjectionUnloadedUndo"
            } else {
                "ProjectionUnloadedRedo"
            });
        let project = state.capture_project_session().unwrap().instance_id;
        let mut revision = GraphRevision::new(1);
        if !undo {
            let result = state
                .undo_last_transaction_observed(
                    &project,
                    "en-US",
                    history_request(resource.clone(), revision),
                    |_| {},
                )
                .unwrap();
            revision = result.deltas[0].to_revision;
        }
        let files = file_snapshots([root.join(graph_path.as_str())]);
        state.set_projection_environment_after_path_data_test_hook(publish_empty_replacement_hook(
            &state, &root,
        ));
        let observed = std::sync::atomic::AtomicBool::new(false);
        let error = run_history_direction(
            &state,
            &project,
            undo,
            history_request(resource, revision),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_empty_replacement_authority(&state, &project);
        assert_file_snapshots_unchanged(&files);
        let _ = std::fs::remove_dir_all(root_text);

        let (state, root, resource, _) = durable_variable_history_fixture(if undo {
            "ProjectionVariableUndo"
        } else {
            "ProjectionVariableRedo"
        });
        let project = state.capture_project_session().unwrap().instance_id;
        let mut revision = GraphRevision::new(1);
        if !undo {
            let result = state
                .undo_last_transaction_observed(
                    &project,
                    "en-US",
                    history_request(resource.clone(), revision),
                    |_| {},
                )
                .unwrap();
            revision = result.deltas[0].to_revision;
        }
        let files = file_snapshots([root.join(crate::project::GLOBAL_VARIABLES_FILE)]);
        state.set_projection_environment_after_path_data_test_hook(publish_empty_replacement_hook(
            &state, &root,
        ));
        let observed = std::sync::atomic::AtomicBool::new(false);
        let error = run_history_direction(
            &state,
            &project,
            undo,
            history_request(resource, revision),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_empty_replacement_authority(&state, &project);
        assert_file_snapshots_unchanged(&files);
        let _ = std::fs::remove_dir_all(root);

        let (state, root, source, target, mut revision) = graph_move_history_fixture(if undo {
            "ProjectionMoveUndo"
        } else {
            "ProjectionMoveRedo"
        });
        let project = state.capture_project_session().unwrap().instance_id;
        let mut path = target.clone();
        if !undo {
            let result = state
                .undo_last_transaction_observed(
                    &project,
                    "en-US",
                    history_request(
                        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                            path.as_str().into(),
                        )),
                        revision,
                    ),
                    |_| {},
                )
                .unwrap();
            path = source.clone();
            revision = result.deltas[0].to_revision;
        }
        let files = file_snapshots([root.join(path.as_str())]);
        state.set_projection_environment_after_path_data_test_hook(publish_empty_replacement_hook(
            &state, &root,
        ));
        let observed = std::sync::atomic::AtomicBool::new(false);
        let error = run_history_direction(
            &state,
            &project,
            undo,
            history_request(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    path.as_str().into(),
                )),
                revision,
            ),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_empty_replacement_authority(&state, &project);
        assert_file_snapshots_unchanged(&files);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn history_lifecycle_typing_rolls_back_variable_and_graph_move_finalization_for_undo_and_redo() {
    for undo in [true, false] {
        let (state, root, resource, _) = durable_variable_history_fixture(if undo {
            "FinalizeVariableUndo"
        } else {
            "FinalizeVariableRedo"
        });
        let project = state.capture_project_session().unwrap().instance_id;
        let mut revision = GraphRevision::new(1);
        if !undo {
            let result = state
                .undo_last_transaction_observed(
                    &project,
                    "en-US",
                    history_request(resource.clone(), revision),
                    |_| {},
                )
                .unwrap();
            revision = result.deltas[0].to_revision;
        }
        let files = file_snapshots([root.join(crate::project::GLOBAL_VARIABLES_FILE)]);
        state
            .set_history_after_disk_commit_test_hook(publish_empty_replacement_hook(&state, &root));
        let observed = std::sync::atomic::AtomicBool::new(false);
        let error = run_history_direction(
            &state,
            &project,
            undo,
            history_request(resource, revision),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_empty_replacement_authority(&state, &project);
        assert_file_snapshots_unchanged(&files);
        let _ = std::fs::remove_dir_all(root);

        let (state, root, source, target, mut revision) = graph_move_history_fixture(if undo {
            "FinalizeMoveUndo"
        } else {
            "FinalizeMoveRedo"
        });
        let project = state.capture_project_session().unwrap().instance_id;
        let mut path = target.clone();
        if !undo {
            let result = state
                .undo_last_transaction_observed(
                    &project,
                    "en-US",
                    history_request(
                        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                            path.as_str().into(),
                        )),
                        revision,
                    ),
                    |_| {},
                )
                .unwrap();
            path = source.clone();
            revision = result.deltas[0].to_revision;
        }
        let source_file = root.join(source.as_str());
        let target_file = root.join(target.as_str());
        assert_ne!(source_file, target_file);
        if undo {
            assert!(!source_file.exists());
            assert!(target_file.is_file());
        } else {
            assert!(source_file.is_file());
            assert!(!target_file.exists());
        }
        let files = file_snapshots([source_file, target_file]);
        assert_ne!(files[0].0, files[1].0);
        state.set_graph_move_history_io_checkpoint(publish_empty_replacement_hook(&state, &root));
        let observed = std::sync::atomic::AtomicBool::new(false);
        let error = run_history_direction(
            &state,
            &project,
            undo,
            history_request(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    path.as_str().into(),
                )),
                revision,
            ),
            &observed,
        )
        .unwrap_err();
        assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_empty_replacement_authority(&state, &project);
        assert_file_snapshots_unchanged(&files);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn history_lifecycle_typing_preserves_recovery_required_when_durable_rollback_fails() {
    for variable_policy in [true, false] {
        for undo in [true, false] {
            let label = match (variable_policy, undo) {
                (true, true) => "RecoveryVariableUndo",
                (true, false) => "RecoveryVariableRedo",
                (false, true) => "RecoveryMoveUndo",
                (false, false) => "RecoveryMoveRedo",
            };
            let (state, root, mut resource, mut revision, redo_path) = if variable_policy {
                let (state, root, resource, _) = durable_variable_history_fixture(label);
                (state, root, resource, GraphRevision::new(1), None)
            } else {
                let (state, root, source, target, revision) = graph_move_history_fixture(label);
                (
                    state,
                    root,
                    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        target.as_str().into(),
                    )),
                    revision,
                    Some(source),
                )
            };
            let project = state.capture_project_session().unwrap().instance_id;
            if !undo {
                let result = state
                    .undo_last_transaction_observed(
                        &project,
                        "en-US",
                        history_request(resource.clone(), revision),
                        |_| {},
                    )
                    .unwrap();
                revision = result.deltas[0].to_revision;
                if let Some(path) = redo_path {
                    resource = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        path.as_str().into(),
                    ));
                }
            }
            if variable_policy {
                state.set_history_after_disk_commit_test_hook(publish_empty_replacement_hook(
                    &state, &root,
                ));
            } else {
                state.set_graph_move_history_io_checkpoint(publish_empty_replacement_hook(
                    &state, &root,
                ));
            }
            state.set_project_filesystem_rollback_fault(true);
            let observed = std::sync::atomic::AtomicBool::new(false);
            let result = run_history_direction(
                &state,
                &project,
                undo,
                history_request(resource, revision),
                &observed,
            );
            state.set_project_filesystem_rollback_fault(false);
            let error = result.unwrap_err();
            assert!(matches!(error, MutationConflict::RecoveryRequired(_)));
            assert_eq!(error.code(), "project_recovery_required");
            assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
            assert_empty_replacement_authority(&state, &project);
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

#[derive(Clone, Copy)]
enum ExpectedHistoryConflict {
    History,
    StaleProjectLifecycle,
}

fn assert_unloaded_post_disk_race_rejected(
    label: &str,
    expected_conflict: ExpectedHistoryConflict,
    mutate: impl FnOnce(&ProjectState, &GraphResourcePath, &str),
) {
    let (state, root, root_text, graph_path, resource, _) = durable_unloaded_history_fixture(label);
    let graph_file = root.join(graph_path.as_str());
    let before_file = std::fs::read(&graph_file).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("bounded post-disk checkpoint release");
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_thread = observed.clone();
    let history_state = state.clone();
    let history_thread = std::thread::spawn(move || {
        history_state.undo_last_transaction_observed(
            &current_project_instance_id(&history_state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {
                observed_thread.store(true, std::sync::atomic::Ordering::SeqCst);
            },
        )
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("History reached post-disk checkpoint");
    mutate(&state, &graph_path, &root_text);
    let raced_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let raced_status = state.history_status();
    let raced_lengths = state.history_lengths_for_test();
    let raced_head = state.history_head_id_for_test(true);
    let raced_revisions = state.revision_state_for_test();
    let raced_publication = state.publication_state_for_test();
    release_tx.send(()).unwrap();

    let error = history_thread.join().unwrap().unwrap_err();

    match expected_conflict {
        ExpectedHistoryConflict::History => {
            assert!(matches!(error, MutationConflict::History(_)));
        }
        ExpectedHistoryConflict::StaleProjectLifecycle => {
            assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
        }
    }
    assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        raced_data
    );
    assert_eq!(state.history_status(), raced_status);
    assert_eq!(state.history_lengths_for_test(), raced_lengths);
    assert_eq!(state.history_head_id_for_test(true), raced_head);
    assert_eq!(state.revision_state_for_test(), raced_revisions);
    assert_eq!(state.publication_state_for_test(), raced_publication);
    assert_eq!(std::fs::read(&graph_file).unwrap(), before_file);
    assert!(!root.join(".yssbi-transaction").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_history_post_disk_authority_race_matrix_rejects_without_publication() {
    assert_unloaded_post_disk_race_rejected(
        "HistoryHeadRace",
        ExpectedHistoryConflict::History,
        |state, _, _| state.append_history_head_for_test(),
    );
    assert_unloaded_post_disk_race_rejected(
        "HistoryDirectionRace",
        ExpectedHistoryConflict::History,
        |state, _, _| {
            state
                .history
                .write()
                .unwrap()
                .move_undo_head_to_redo_for_test();
        },
    );
    assert_unloaded_post_disk_race_rejected(
        "GraphRevisionRace",
        ExpectedHistoryConflict::History,
        |state, path, _| {
            state
                .graph_revisions
                .write()
                .unwrap()
                .insert(path.clone(), GraphRevision::new(7));
        },
    );
    assert_unloaded_post_disk_race_rejected(
        "AuthorityGenerationRace",
        ExpectedHistoryConflict::StaleProjectLifecycle,
        |state, _, _| {
            state
                .mutation_publication
                .lock()
                .unwrap()
                .advance_authority_generation();
        },
    );
    assert_unloaded_post_disk_race_rejected(
        "ProjectInstanceRace",
        ExpectedHistoryConflict::StaleProjectLifecycle,
        |state, _, _| {
            state
                .mutation_publication
                .lock()
                .unwrap()
                .project_instance_id = uuid::Uuid::new_v4().to_string();
        },
    );
    assert_unloaded_post_disk_race_rejected(
        "ProjectSessionRace",
        ExpectedHistoryConflict::StaleProjectLifecycle,
        |state, _, root_text| {
            let replacement = std::path::PathBuf::from(root_text).join("replacement-session");
            std::fs::create_dir_all(&replacement).unwrap();
            state.replace_active_root_for_test(
                crate::project::NormalizedProjectRoot::from_project_path(replacement).unwrap(),
            );
        },
    );
    assert_unloaded_post_disk_race_rejected(
        "ResidencyRace",
        ExpectedHistoryConflict::History,
        |state, path, root_text| {
            let graph =
                crate::project::project_io::load_project_graph_from_file(root_text, path).unwrap();
            state
                .project_data
                .write()
                .unwrap()
                .graphs
                .insert(path.clone(), graph);
        },
    );
}

#[test]
fn mixed_residency_history_rejects_variable_revision_and_tombstone_race() {
    let (state, root, graph_path, resource, variable_id) =
        durable_graph_global_history_fixture("VariableTombstoneRace");
    let graph_file = root.join(graph_path.as_str());
    let variables_file = root.join(crate::project::GLOBAL_VARIABLES_FILE);
    let before_graph = std::fs::read(&graph_file).unwrap();
    let before_variables = std::fs::read(&variables_file).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_thread = observed.clone();
    let history_state = state.clone();
    let history_thread = std::thread::spawn(move || {
        history_state.undo_last_transaction_observed(
            &current_project_instance_id(&history_state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| observed_thread.store(true, std::sync::atomic::Ordering::SeqCst),
        )
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    state
        .project_data
        .write()
        .unwrap()
        .variables
        .remove(&variable_id);
    state.variable_revisions.write().unwrap().insert(
        variable_id,
        super::project_state::VariableRevisionEntry::deleted(GraphRevision::new(7)),
    );
    let raced_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let raced_history = (
        state.history_status(),
        state.history_lengths_for_test(),
        state.history_head_id_for_test(true),
    );
    let raced_revisions = state.revision_state_for_test();
    let raced_entry = state.variable_revision_entry_for_test(&variable_id);
    let raced_publication = state.publication_state_for_test();
    release_tx.send(()).unwrap();

    assert!(matches!(
        history_thread.join().unwrap(),
        Err(MutationConflict::History(_))
    ));
    assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        raced_data
    );
    assert_eq!(
        (
            state.history_status(),
            state.history_lengths_for_test(),
            state.history_head_id_for_test(true),
        ),
        raced_history
    );
    assert_eq!(state.revision_state_for_test(), raced_revisions);
    assert_eq!(
        state.variable_revision_entry_for_test(&variable_id),
        raced_entry
    );
    assert_eq!(state.publication_state_for_test(), raced_publication);
    assert_eq!(std::fs::read(&graph_file).unwrap(), before_graph);
    assert_eq!(std::fs::read(&variables_file).unwrap(), before_variables);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn loaded_only_history_routing_rejects_specialized_policy_races() {
    for (label, policy) in [
        (
            "LoadedVariablePolicyRace",
            crate::node_system::document::HistoryPersistencePolicy::DurableVariableEffects,
        ),
        (
            "LoadedMovePolicyRace",
            crate::node_system::document::HistoryPersistencePolicy::DurableResourceMove,
        ),
    ] {
        let (state, root, _root_text, graph_path, resource, _) =
            durable_unloaded_history_fixture(label);
        load_graph(&state, &graph_path).unwrap();
        let graph_file = root.join(graph_path.as_str());
        let before_file = std::fs::read(&graph_file).unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Mutex::new(release_rx);
        state.set_history_after_routing_test_hook(std::sync::Arc::new(move || {
            entered_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("bounded loaded-only routing checkpoint release");
        }));
        let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed_thread = observed.clone();
        let history_state = state.clone();
        let request_resource = resource.clone();
        let history_thread = std::thread::spawn(move || {
            history_state.undo_last_transaction_observed(
                &current_project_instance_id(&history_state),
                "en-US",
                MutationRequest::new(
                    request_resource,
                    GraphRevision::new(1),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| observed_thread.store(true, std::sync::atomic::Ordering::SeqCst),
            )
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("loaded-only History reached routing checkpoint");
        let mut specialized_head = state.history.read().unwrap().next_undo().cloned().unwrap();
        specialized_head.history_id = crate::node_system::document::HistoryEntryId::new();
        specialized_head.persistence = policy;
        match policy {
            crate::node_system::document::HistoryPersistencePolicy::DurableVariableEffects => {
                specialized_head.variable_effect_snapshots = Some(Default::default());
                specialized_head.graph_resource_move = None;
            }
            crate::node_system::document::HistoryPersistencePolicy::DurableResourceMove => {
                let ResourceKey::Graph(path) = resource.clone() else {
                    unreachable!();
                };
                specialized_head.variable_effect_snapshots = None;
                specialized_head.graph_resource_move = Some(
                    crate::node_system::document::GraphResourceMoveHistoryPatch {
                        from: path.clone(),
                        to: path,
                        payload: serde_json::Value::Null,
                    },
                );
            }
            crate::node_system::document::HistoryPersistencePolicy::InMemoryUntilSave => {
                unreachable!();
            }
        }
        state
            .history
            .write()
            .unwrap()
            .record_committed_transaction(specialized_head);
        let raced_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let raced_history = (
            state.history_status(),
            state.history_lengths_for_test(),
            state.history_head_id_for_test(true),
            state.history_head_id_for_test(false),
        );
        let raced_revisions = state.revision_state_for_test();
        let raced_publication = state.publication_state_for_test();
        release_tx.send(()).unwrap();

        let error = history_thread.join().unwrap().unwrap_err();

        assert!(matches!(error, MutationConflict::History(_)));
        assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            raced_data
        );
        assert_eq!(
            (
                state.history_status(),
                state.history_lengths_for_test(),
                state.history_head_id_for_test(true),
                state.history_head_id_for_test(false),
            ),
            raced_history
        );
        assert_eq!(state.revision_state_for_test(), raced_revisions);
        assert_eq!(state.publication_state_for_test(), raced_publication);
        assert_eq!(std::fs::read(&graph_file).unwrap(), before_file);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn unloaded_graph_history_routing_rejects_specialized_head_race() {
    let (state, root, _root_text, graph_path, resource, _) =
        durable_unloaded_history_fixture("RoutingPolicyRace");
    let graph_file = root.join(graph_path.as_str());
    let before_file = std::fs::read(&graph_file).unwrap();
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_publication = state.publication_state_for_test();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_history_after_routing_test_hook(std::sync::Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("bounded routing checkpoint release");
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_thread = observed.clone();
    let history_state = state.clone();
    let history_thread = std::thread::spawn(move || {
        history_state.undo_last_transaction_observed(
            &current_project_instance_id(&history_state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {
                observed_thread.store(true, std::sync::atomic::Ordering::SeqCst);
            },
        )
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("History reached routing checkpoint");
    let mut specialized_head = state.history.read().unwrap().next_undo().cloned().unwrap();
    specialized_head.history_id = crate::node_system::document::HistoryEntryId::new();
    specialized_head.persistence =
        crate::node_system::document::HistoryPersistencePolicy::DurableVariableEffects;
    specialized_head.variable_effect_snapshots = Some(Default::default());
    state
        .history
        .write()
        .unwrap()
        .record_committed_transaction(specialized_head);
    let raced_head = state.history_head_id_for_test(true);
    let raced_lengths = state.history_lengths_for_test();
    release_tx.send(()).unwrap();

    let error = history_thread.join().unwrap().unwrap_err();

    assert!(matches!(error, MutationConflict::History(_)));
    assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(state.history_head_id_for_test(true), raced_head);
    assert_eq!(state.history_lengths_for_test(), raced_lengths);
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(std::fs::read(&graph_file).unwrap(), before_file);
    assert!(!root.join(".yssbi-transaction").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_history_rejects_stale_function_owner_graph_revision() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-function-owner-revision-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let function_path = GraphResourcePath::new("functions/OwnerRevision.yssbi-function").unwrap();
    let function_key =
        crate::node_system::document::FunctionResourceKey(function_path.as_str().into());
    let resource = ResourceKey::Function(function_key);
    let mut project = ProjectData::new();
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("OwnerRevision", GraphDocumentKind::Function),
    );
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &function_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();
    state
        .project_data
        .write()
        .unwrap()
        .graphs
        .get_mut(&function_path)
        .unwrap()
        .document
        .revision = GraphRevision::new(1);
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(function_path.clone(), GraphRevision::new(1));
    crate::project::fixtures::write_state_graph(&state, &function_path).unwrap();
    state.unload_graph_resource(&function_path).unwrap();
    let graph_file = root.join(function_path.as_str());
    let before_file = std::fs::read(&graph_file).unwrap();
    let before_history = state.history_status();
    let before_publication = state.publication_state_for_test();
    let hook_state = state.clone();
    let hook_path = function_path.clone();
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        hook_state
            .graph_revisions
            .write()
            .unwrap()
            .insert(hook_path.clone(), GraphRevision::new(7));
    }));
    let mut observed = false;

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| observed = true,
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::History(_)));
    assert!(!observed);
    assert_eq!(std::fs::read(&graph_file).unwrap(), before_file);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(
        state
            .revision_state_for_test()
            .0
            .get(&function_path)
            .copied(),
        Some(GraphRevision::new(7))
    );
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mixed_residency_history_rejects_loaded_function_revision_race() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-loaded-function-race-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let event_path = GraphResourcePath::new("events/FunctionRace.yssbi-event").unwrap();
    let function_path =
        GraphResourcePath::new("functions/LoadedFunctionRace.yssbi-function").unwrap();
    let event_key = crate::node_system::document::GraphResourcePath(event_path.as_str().into());
    let function_key =
        crate::node_system::document::FunctionResourceKey(function_path.as_str().into());
    let event_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: node("yssbi.constant.int64"),
    }]);
    let function_patch = crate::node_system::document::FunctionDocumentPatch::new(
        Default::default(),
        test_signature(),
    );
    let mut project = ProjectData::new();
    project.graphs.insert(
        event_path.clone(),
        GraphResourceDocument::new("FunctionRace", GraphDocumentKind::Event),
    );
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("LoadedFunctionRace", GraphDocumentKind::Function),
    );
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &event_path).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &function_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
        OperationId::new(),
        vec![
            crate::node_system::document::ResourcePatch::graph(
                event_key.clone(),
                GraphRevision::INITIAL,
                event_patch,
            ),
            crate::node_system::document::ResourcePatch::function(
                function_key.clone(),
                GraphRevision::INITIAL,
                function_patch,
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
        data.graphs
            .get_mut(&function_path)
            .unwrap()
            .document
            .revision = GraphRevision::new(1);
        *state.history.write().unwrap() = history;
    }
    {
        let mut graph_revisions = state.graph_revisions.write().unwrap();
        graph_revisions.insert(event_path.clone(), GraphRevision::new(1));
        graph_revisions.insert(function_path.clone(), GraphRevision::new(1));
    }
    crate::project::fixtures::write_state_graph(&state, &event_path).unwrap();
    crate::project::fixtures::write_state_graph(&state, &function_path).unwrap();
    state.unload_graph_resource(&event_path).unwrap();
    let event_file = root.join(event_path.as_str());
    let function_file = root.join(function_path.as_str());
    let before_event = std::fs::read(&event_file).unwrap();
    let before_function = std::fs::read(&function_file).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
    }));
    let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_thread = observed.clone();
    let history_state = state.clone();
    let history_thread = std::thread::spawn(move || {
        history_state.undo_last_transaction_observed(
            &current_project_instance_id(&history_state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(event_key),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| observed_thread.store(true, std::sync::atomic::Ordering::SeqCst),
        )
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    state
        .project_data
        .write()
        .unwrap()
        .graphs
        .get_mut(&function_path)
        .unwrap()
        .function
        .as_mut()
        .unwrap()
        .revision = GraphRevision::new(7);
    let raced_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let raced_revisions = state.revision_state_for_test();
    let raced_history = state.history_status();
    let raced_publication = state.publication_state_for_test();
    release_tx.send(()).unwrap();

    assert!(matches!(
        history_thread.join().unwrap(),
        Err(MutationConflict::History(_))
    ));
    assert!(!observed.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        raced_data
    );
    assert_eq!(state.revision_state_for_test(), raced_revisions);
    assert_eq!(state.history_status(), raced_history);
    assert_eq!(state.publication_state_for_test(), raced_publication);
    assert_eq!(std::fs::read(&event_file).unwrap(), before_event);
    assert_eq!(std::fs::read(&function_file).unwrap(), before_function);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_function_history_preserves_embedded_abi_and_publishes_after_finalize() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-unloaded-function-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let function_path = GraphResourcePath::new("functions/DurableAbi.yssbi-function").unwrap();
    let function_key =
        crate::node_system::document::FunctionResourceKey(function_path.as_str().into());
    let resource = ResourceKey::Function(function_key.clone());
    let signature = crate::node_system::document::FunctionSignature {
        parameters: vec![
            crate::node_system::document::FunctionParameter {
                id: crate::node_system::document::FunctionParameterId("request_id".into()),
                name: "Request ID".into(),
                type_name: "string".into(),
            },
            crate::node_system::document::FunctionParameter {
                id: crate::node_system::document::FunctionParameterId("payload".into()),
                name: "Payload".into(),
                type_name: "json".into(),
            },
        ],
        return_type: Some("boolean".into()),
    };
    let mut project = ProjectData::new();
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Durable ABI", GraphDocumentKind::Function),
    );
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &function_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                signature.clone(),
            ),
            |_| {},
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &function_path).unwrap();
    state.unload_graph_resource(&function_path).unwrap();
    let before_publication = state.publication_state_for_test();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let undo_operation = OperationId::new();
    let (checkpoint_tx, checkpoint_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(Some(release_rx));
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        let Some(release_rx) = release_rx.lock().unwrap().take() else {
            return;
        };
        checkpoint_tx.send(()).unwrap();
        release_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
    }));
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let undo_state = state.clone();
    let undo_resource = resource.clone();
    let undo_thread = std::thread::spawn(move || {
        undo_state.undo_last_transaction_observed(
            &current_project_instance_id(&undo_state),
            "en-US",
            MutationRequest::new(
                undo_resource,
                GraphRevision::new(1),
                undo_operation,
                HistoryMutation {},
            ),
            move |result| observed_tx.send(result.clone()).unwrap(),
        )
    });

    checkpoint_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("Function History reached the post-disk/pre-authority checkpoint");
    assert!(matches!(
        observed_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert!(state.history_status().can_undo);
    assert!(!state.history_status().can_redo);
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    let staged_undo = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &function_path,
    )
    .unwrap();
    assert_eq!(
        staged_undo.function.as_ref().unwrap().revision,
        GraphRevision::new(2)
    );
    assert_eq!(
        staged_undo.function.as_ref().unwrap().signature,
        crate::node_system::document::FunctionSignature::default()
    );
    release_tx.send(()).unwrap();

    let undo = undo_thread.join().unwrap().unwrap();
    let observed_undo = observed_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(observed_undo, undo);
    assert_eq!(undo.operation_id, undo_operation);
    assert_eq!(undo.project_instance_id, project_instance_id.as_str());
    assert_eq!(undo.publication_revision, before_publication.1 + 1);
    assert_eq!(undo.deltas.len(), 1);
    assert_eq!(undo.deltas[0].resource, resource);
    assert_eq!(undo.deltas[0].from_revision, GraphRevision::new(1));
    assert_eq!(undo.deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo.deltas[0].caused_by, Some(undo_operation));
    assert_eq!(undo.history, state.history_status());
    assert_eq!(
        undo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        }
    );
    assert!(undo.projection_replacements.is_empty());
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    assert_eq!(
        state.revision_state_for_test().0[&function_path],
        GraphRevision::new(2),
        "the owner graph ledger mirrors the embedded Function revision without a Function ledger"
    );
    let undo_disk = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &function_path,
    )
    .unwrap();
    assert_eq!(
        undo_disk.function.as_ref().unwrap().revision,
        undo.deltas[0].to_revision
    );
    assert_eq!(
        undo_disk.function.as_ref().unwrap().signature,
        crate::node_system::document::FunctionSignature::default()
    );
    let hydrated_redo = state
        .prepare_history_for_test(
            false,
            MutationRequest::new(
                resource.clone(),
                undo.deltas[0].to_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
        )
        .unwrap();
    assert_eq!(
        hydrated_redo.before.functions[&function_key].revision,
        undo.deltas[0].to_revision
    );
    assert_eq!(
        hydrated_redo.before.functions[&function_key].signature,
        crate::node_system::document::FunctionSignature::default()
    );
    assert_eq!(
        hydrated_redo.after.functions[&function_key].signature,
        signature
    );
    drop(hydrated_redo);

    let redo_operation = OperationId::new();
    let mut redo_observed = Vec::new();
    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(2),
                redo_operation,
                HistoryMutation {},
            ),
            |result| redo_observed.push(result.clone()),
        )
        .unwrap();
    assert_eq!(redo_observed, vec![redo.clone()]);
    assert_eq!(redo.operation_id, redo_operation);
    assert_eq!(redo.project_instance_id, project_instance_id.as_str());
    assert_eq!(redo.publication_revision, undo.publication_revision + 1);
    assert_eq!(redo.deltas.len(), 1);
    assert_eq!(redo.deltas[0].resource, resource);
    assert_eq!(redo.deltas[0].from_revision, GraphRevision::new(2));
    assert_eq!(redo.deltas[0].to_revision, GraphRevision::new(3));
    assert_eq!(redo.deltas[0].caused_by, Some(redo_operation));
    assert_eq!(redo.history, state.history_status());
    assert_eq!(
        redo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        }
    );
    assert!(redo.projection_replacements.is_empty());
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    let redo_disk = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &function_path,
    )
    .unwrap();
    let redo_function = redo_disk.function.as_ref().unwrap();
    assert_eq!(redo_function.revision, redo.deltas[0].to_revision);
    assert_eq!(redo_disk.revision, redo.deltas[0].to_revision);
    assert_eq!(
        state.revision_state_for_test().0[&function_path],
        redo.deltas[0].to_revision
    );
    assert_eq!(redo_function.signature, signature);
    assert_eq!(
        redo_function
            .signature
            .parameters
            .iter()
            .map(|parameter| parameter.id.0.as_ref())
            .collect::<Vec<_>>(),
        vec!["request_id", "payload"]
    );
    assert_eq!(
        redo_function.signature.return_type.as_deref(),
        Some("boolean")
    );
    let hydrated_undo = state
        .prepare_history_for_test(
            true,
            MutationRequest::new(
                resource,
                redo.deltas[0].to_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
        )
        .unwrap();
    assert_eq!(
        hydrated_undo.before.functions[&function_key].revision,
        redo.deltas[0].to_revision
    );
    assert_eq!(
        hydrated_undo.before.functions[&function_key].signature,
        signature
    );
    drop(hydrated_undo);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_local_variable_history_preserves_scope_tombstones_and_loaded_only_projection() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-unloaded-local-variable-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let event_path = GraphResourcePath::new("events/LocalOwner.yssbi-event").unwrap();
    let function_path = GraphResourcePath::new("functions/LocalOwner.yssbi-function").unwrap();
    let loaded_path = GraphResourcePath::new("events/LoadedProjection.yssbi-event").unwrap();
    let loaded_key = crate::node_system::document::GraphResourcePath(loaded_path.as_str().into());
    let loaded_node = node("yssbi.constant.int64");
    let loaded_node_id = loaded_node.id;

    let mut created = test_variable("Created local");
    created.scope = crate::variable::VariableScope::Event {
        event_path: event_path.as_str().into(),
    };
    let mut updated_before = test_variable("Function local before");
    updated_before.scope = crate::variable::VariableScope::Function {
        function_path: function_path.as_str().into(),
    };
    let mut updated_after = updated_before.clone();
    updated_after.name = "Function local after".into();
    let mut removed = test_variable("Removed local");
    removed.scope = crate::variable::VariableScope::Event {
        event_path: event_path.as_str().into(),
    };
    let global_before = test_variable("Global before");
    let mut global_after = global_before.clone();
    global_after.name = "Global after".into();

    let variable_key = |id: crate::variable::VariableId| {
        crate::node_system::document::VariableResourceKey(format!("variables/{id}").into())
    };
    let created_key = variable_key(created.id);
    let updated_key = variable_key(updated_before.id);
    let removed_key = variable_key(removed.id);
    let global_key = variable_key(global_before.id);
    let mut project = ProjectData::new();
    for (path, name, kind) in [
        (&event_path, "Event local owner", GraphDocumentKind::Event),
        (
            &function_path,
            "Function local owner",
            GraphDocumentKind::Function,
        ),
        (&loaded_path, "Loaded projection", GraphDocumentKind::Event),
    ] {
        project
            .graphs
            .insert(path.clone(), GraphResourceDocument::new(name, kind));
    }
    project
        .variables
        .insert(updated_before.id, updated_before.clone());
    project.variables.insert(removed.id, removed.clone());
    project
        .variables
        .insert(global_before.id, global_before.clone());
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    for path in [&event_path, &function_path, &loaded_path] {
        crate::project::fixtures::write_graph(&project, &root_text, path).unwrap();
    }
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    state.variable_revisions.write().unwrap().insert(
        created.id,
        super::project_state::VariableRevisionEntry::deleted(GraphRevision::INITIAL),
    );
    let initial_operation = OperationId::new();
    let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
        initial_operation,
        vec![
            crate::node_system::document::ResourcePatch::graph(
                loaded_key.clone(),
                GraphRevision::INITIAL,
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: loaded_node.clone(),
                }]),
            ),
            crate::node_system::document::ResourcePatch::variable(
                created_key.clone(),
                GraphRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    None,
                    Some(serde_json::to_value(&created).unwrap()),
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                updated_key.clone(),
                GraphRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(&updated_before).unwrap()),
                    Some(serde_json::to_value(&updated_after).unwrap()),
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                removed_key.clone(),
                GraphRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(&removed).unwrap()),
                    None,
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                global_key.clone(),
                GraphRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(&global_before).unwrap()),
                    Some(serde_json::to_value(&global_after).unwrap()),
                ),
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
        *state.history.write().unwrap() = history;
    }
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(loaded_path.clone(), GraphRevision::new(1));
    crate::project::fixtures::write_project(&state.get_data().unwrap(), &root_text).unwrap();
    for path in [&event_path, &function_path, &loaded_path] {
        crate::project::fixtures::write_state_graph(&state, path).unwrap();
    }
    state.unload_graph_resource(&event_path).unwrap();
    state.unload_graph_resource(&function_path).unwrap();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    for id in [created.id, updated_before.id, removed.id] {
        assert!(!state.get_data().unwrap().variables.contains_key(&id));
    }
    assert_eq!(
        state.get_data().unwrap().variables[&global_before.id].name,
        global_after.name
    );

    let undo_operation = OperationId::new();
    let mut undo_observed = Vec::new();
    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(loaded_key.clone()),
                GraphRevision::new(1),
                undo_operation,
                HistoryMutation {},
            ),
            |result| undo_observed.push(result.clone()),
        )
        .unwrap();
    assert_eq!(undo_observed, vec![undo.clone()]);
    assert_eq!(undo.operation_id, undo_operation);
    assert_eq!(undo.project_instance_id, project_instance_id.as_str());
    assert_eq!(
        undo.deltas,
        vec![
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Graph(loaded_key.clone()),
                from_revision: GraphRevision::new(1),
                to_revision: GraphRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Graph(
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::RemoveNode {
                        node: loaded_node.clone(),
                    }]),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(created_key.clone()),
                from_revision: GraphRevision::new(1),
                to_revision: GraphRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&created).unwrap()),
                        None,
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(updated_key.clone()),
                from_revision: GraphRevision::new(1),
                to_revision: GraphRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&updated_after).unwrap()),
                        Some(serde_json::to_value(&updated_before).unwrap()),
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(removed_key.clone()),
                from_revision: GraphRevision::new(1),
                to_revision: GraphRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        None,
                        Some(serde_json::to_value(&removed).unwrap()),
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(global_key.clone()),
                from_revision: GraphRevision::new(1),
                to_revision: GraphRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&global_after).unwrap()),
                        Some(serde_json::to_value(&global_before).unwrap()),
                    ),
                ),
            },
        ]
    );
    assert_eq!(undo.history, state.history_status());
    assert_eq!(
        undo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![loaded_path.as_str().to_string()],
        }
    );
    assert_eq!(undo.projection_replacements.len(), 1);
    assert_eq!(
        undo.projection_replacements[0].graph_path.as_str(),
        loaded_path.as_str()
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&event_path));
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    for id in [created.id, updated_before.id, removed.id] {
        assert!(!state.get_data().unwrap().variables.contains_key(&id));
    }
    let created_revision = state.variable_revision_entry_for_test(&created.id).unwrap();
    let updated_revision = state
        .variable_revision_entry_for_test(&updated_before.id)
        .unwrap();
    let removed_revision = state.variable_revision_entry_for_test(&removed.id).unwrap();
    let global_revision = state
        .variable_revision_entry_for_test(&global_before.id)
        .unwrap();
    assert_eq!(created_revision.revision, GraphRevision::new(2));
    assert!(!created_revision.is_present());
    for revision in [updated_revision, removed_revision, global_revision] {
        assert_eq!(revision.revision, GraphRevision::new(2));
        assert!(revision.is_present());
    }
    let undo_event =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &event_path)
            .unwrap();
    let undo_function = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &function_path,
    )
    .unwrap();
    assert!(!undo_event.local_variables.contains_key(&created.id));
    assert_eq!(
        undo_event.local_variables[&removed.id].scope,
        crate::variable::VariableScope::Event {
            event_path: event_path.as_str().into(),
        }
    );
    assert_eq!(
        undo_function.local_variables[&updated_before.id].scope,
        crate::variable::VariableScope::Function {
            function_path: function_path.as_str().into(),
        }
    );
    assert_eq!(
        undo_function.local_variables[&updated_before.id].name,
        updated_before.name
    );
    let undo_globals = crate::project::project_io::parse_global_variables_document(
        &std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        undo_globals.variables[&global_before.id].name,
        global_before.name
    );

    let redo_operation = OperationId::new();
    let mut redo_observed = Vec::new();
    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(loaded_key.clone()),
                GraphRevision::new(2),
                redo_operation,
                HistoryMutation {},
            ),
            |result| redo_observed.push(result.clone()),
        )
        .unwrap();
    assert_eq!(redo_observed, vec![redo.clone()]);
    assert_eq!(redo.operation_id, redo_operation);
    assert_eq!(redo.project_instance_id, project_instance_id.as_str());
    assert_eq!(redo.publication_revision, undo.publication_revision + 1);
    assert_eq!(
        redo.deltas,
        vec![
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Graph(loaded_key.clone()),
                from_revision: GraphRevision::new(2),
                to_revision: GraphRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Graph(
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                        node: loaded_node.clone(),
                    }]),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(created_key.clone()),
                from_revision: GraphRevision::new(2),
                to_revision: GraphRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        None,
                        Some(serde_json::to_value(&created).unwrap()),
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(updated_key.clone()),
                from_revision: GraphRevision::new(2),
                to_revision: GraphRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&updated_before).unwrap()),
                        Some(serde_json::to_value(&updated_after).unwrap()),
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(removed_key.clone()),
                from_revision: GraphRevision::new(2),
                to_revision: GraphRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&removed).unwrap()),
                        None,
                    ),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(global_key.clone()),
                from_revision: GraphRevision::new(2),
                to_revision: GraphRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Variable(
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&global_before).unwrap()),
                        Some(serde_json::to_value(&global_after).unwrap()),
                    ),
                ),
            },
        ]
    );
    assert_eq!(redo.history, state.history_status());
    assert_eq!(
        redo.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![loaded_path.as_str().to_string()],
        }
    );
    assert_eq!(redo.projection_replacements.len(), 1);
    assert_eq!(
        redo.projection_replacements[0].graph_path.as_str(),
        loaded_path.as_str()
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&event_path));
    assert!(
        !state
            .get_data()
            .unwrap()
            .graphs
            .contains_key(&function_path)
    );
    for id in [created.id, updated_before.id, removed.id] {
        assert!(!state.get_data().unwrap().variables.contains_key(&id));
    }
    let created_revision = state.variable_revision_entry_for_test(&created.id).unwrap();
    let updated_revision = state
        .variable_revision_entry_for_test(&updated_before.id)
        .unwrap();
    let removed_revision = state.variable_revision_entry_for_test(&removed.id).unwrap();
    let global_revision = state
        .variable_revision_entry_for_test(&global_before.id)
        .unwrap();
    for revision in [created_revision, updated_revision, global_revision] {
        assert_eq!(revision.revision, GraphRevision::new(3));
        assert!(revision.is_present());
    }
    assert_eq!(removed_revision.revision, GraphRevision::new(3));
    assert!(!removed_revision.is_present());
    let redo_event =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &event_path)
            .unwrap();
    let redo_function = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &function_path,
    )
    .unwrap();
    assert_eq!(
        redo_event.local_variables[&created.id].scope,
        crate::variable::VariableScope::Event {
            event_path: event_path.as_str().into(),
        }
    );
    assert!(!redo_event.local_variables.contains_key(&removed.id));
    assert_eq!(
        redo_function.local_variables[&updated_before.id].scope,
        crate::variable::VariableScope::Function {
            function_path: function_path.as_str().into(),
        }
    );
    assert_eq!(
        redo_function.local_variables[&updated_before.id].name,
        updated_after.name
    );
    let redo_globals = crate::project::project_io::parse_global_variables_document(
        &std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        redo_globals.variables[&global_before.id].name,
        global_after.name
    );
    assert!(
        state.get_data().unwrap().graphs[&loaded_path]
            .document
            .nodes
            .contains_key(&loaded_node_id)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_edit_undo_redo_is_durable_and_keeps_graph_unloaded() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-unloaded-graph-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/DurableHistory.yssbi-event").unwrap();
    let resource = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        graph_path.as_str().into(),
    ));
    let inserted_node = node("yssbi.constant.int64");
    let inserted_node_id = inserted_node.id;
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("DurableHistory", GraphDocumentKind::Event),
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
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: inserted_node,
                }]),
            ),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &graph_path).unwrap();
    state.unload_graph_resource(&graph_path).unwrap();

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let undo_disk =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &graph_path)
            .unwrap();
    assert!(!undo_disk.document.nodes.contains_key(&inserted_node_id));
    assert_eq!(undo.deltas.len(), 1);
    assert_eq!(undo.deltas[0].from_revision, GraphRevision::new(1));
    assert_eq!(undo.deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo_disk.revision, undo.deltas[0].to_revision);
    assert_eq!(
        state.revision_state_for_test().0.get(&graph_path).copied(),
        Some(undo_disk.revision)
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    assert!(undo.projection_replacements.is_empty());

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                undo_disk.revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let redo_disk =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &graph_path)
            .unwrap();
    assert!(redo_disk.document.nodes.contains_key(&inserted_node_id));
    assert_eq!(redo.deltas.len(), 1);
    assert_eq!(redo.deltas[0].from_revision, undo_disk.revision);
    assert!(redo.deltas[0].to_revision > undo.deltas[0].to_revision);
    assert_eq!(redo_disk.revision, redo.deltas[0].to_revision);
    assert_eq!(
        state.revision_state_for_test().0.get(&graph_path).copied(),
        Some(redo_disk.revision)
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    assert!(redo.projection_replacements.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_history_staging_and_live_replace_faults_preserve_state() {
    for (label, fault) in [
        (
            "StagedSerializationFault",
            crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
        ),
        (
            "SecondLiveReplacementFault",
            crate::project::ProjectFilesystemFaultPoint::SecondLiveReplacement,
        ),
    ] {
        let (state, root, graph_path, resource, variable_id) =
            durable_graph_global_history_fixture(label);
        let graph_file = root.join(graph_path.as_str());
        let variables_file = root.join(crate::project::GLOBAL_VARIABLES_FILE);
        let before_graph = std::fs::read(&graph_file).unwrap();
        let before_variables = std::fs::read(&variables_file).unwrap();
        let before_loaded_data = state.get_data().unwrap();
        let before_function_revisions = before_loaded_data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph
                    .function
                    .as_ref()
                    .map(|function| (path.clone(), function.revision))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let before_data = serde_json::to_value(before_loaded_data).unwrap();
        let before_history = (
            state.history_status(),
            state.history_lengths_for_test(),
            state.history_head_id_for_test(true),
            state.history_head_id_for_test(false),
        );
        let before_revisions = state.revision_state_for_test();
        let before_variable_entry = state.variable_revision_entry_for_test(&variable_id);
        let before_publication = state.publication_state_for_test();
        state.set_project_filesystem_fault(Some(fault));
        let mut observed = false;

        let error = state
            .undo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    resource,
                    GraphRevision::new(1),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| observed = true,
            )
            .unwrap_err();

        assert!(matches!(error, MutationConflict::History(_)));
        assert!(!observed);
        assert_eq!(std::fs::read(&graph_file).unwrap(), before_graph);
        assert_eq!(std::fs::read(&variables_file).unwrap(), before_variables);
        let after_loaded_data = state.get_data().unwrap();
        let after_function_revisions = after_loaded_data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph
                    .function
                    .as_ref()
                    .map(|function| (path.clone(), function.revision))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(after_function_revisions, before_function_revisions);
        assert_eq!(
            serde_json::to_value(after_loaded_data).unwrap(),
            before_data
        );
        assert_eq!(
            (
                state.history_status(),
                state.history_lengths_for_test(),
                state.history_head_id_for_test(true),
                state.history_head_id_for_test(false),
            ),
            before_history
        );
        assert_eq!(state.revision_state_for_test(), before_revisions);
        assert_eq!(
            state.variable_revision_entry_for_test(&variable_id),
            before_variable_entry
        );
        assert_eq!(state.publication_state_for_test(), before_publication);
        assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
        assert!(!root.join(".yssbi-transaction").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn unloaded_graph_post_disk_commit_revision_mismatch_rolls_back() {
    let (state, root, _root_text, graph_path, resource, _) =
        durable_unloaded_history_fixture("PostDiskMismatch");
    let graph_file = root.join(graph_path.as_str());
    let before_file = std::fs::read(&graph_file).unwrap();
    let before_history = state.history_status();
    let before_publication = state.publication_state_for_test();
    let hook_state = state.clone();
    let hook_path = graph_path.clone();
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        hook_state
            .graph_revisions
            .write()
            .unwrap()
            .insert(hook_path.clone(), GraphRevision::new(7));
    }));
    let mut observed = false;

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| observed = true,
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::History(_)));
    assert!(!observed);
    assert_eq!(std::fs::read(&graph_file).unwrap(), before_file);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(
        state.revision_state_for_test().0.get(&graph_path).copied(),
        Some(GraphRevision::new(7))
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_post_disk_rollback_failure_enters_recovery_required() {
    let (state, root, _root_text, graph_path, resource, _) =
        durable_unloaded_history_fixture("RollbackRecovery");
    let hook_state = state.clone();
    let hook_path = graph_path.clone();
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        hook_state
            .graph_revisions
            .write()
            .unwrap()
            .insert(hook_path.clone(), GraphRevision::new(7));
    }));
    state.set_project_filesystem_rollback_fault(true);
    let mut observed = false;

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| observed = true,
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::RecoveryRequired(_)));
    assert_eq!(error.code(), "project_recovery_required");
    assert!(!observed);
    assert!(matches!(
        state.undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(7),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unloaded_graph_history_lease_excludes_coordinator_operation_until_finalize() {
    let (state, root, _root_text, graph_path, resource, _) =
        durable_unloaded_history_fixture("LeaseExclusion");
    let session = state.capture_project_session().unwrap();
    let coordinator = state.filesystem().clone();
    let (checkpoint_tx, checkpoint_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_history_after_preparation_test_hook(std::sync::Arc::new(move || {
        checkpoint_tx.send(()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("bounded after-hydration checkpoint release");
    }));
    let history_state = state.clone();
    let history_thread = std::thread::spawn(move || {
        history_state.undo_last_transaction_observed(
            &current_project_instance_id(&history_state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
    });
    checkpoint_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("History reached after-hydration/pre-filesystem checkpoint");

    let (lease_tx, lease_rx) = std::sync::mpsc::channel();
    let lease_thread = std::thread::spawn(move || {
        let lease = coordinator.acquire(session.root).unwrap();
        lease_tx.send(()).unwrap();
        drop(lease);
    });
    assert!(
        lease_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "coordinator operation entered while History still owned the preparation lease"
    );
    release_tx.send(()).unwrap();
    history_thread.join().unwrap().unwrap();
    lease_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("coordinator operation entered after History finalization");
    lease_thread.join().unwrap();
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mixed_residency_graph_history_is_atomic_and_preserves_residency() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-mixed-residency-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let loaded_path = GraphResourcePath::new("events/LoadedHistory.yssbi-event").unwrap();
    let unloaded_path = GraphResourcePath::new("events/UnloadedHistory.yssbi-event").unwrap();
    let loaded_key = crate::node_system::document::GraphResourcePath(loaded_path.as_str().into());
    let unloaded_key =
        crate::node_system::document::GraphResourcePath(unloaded_path.as_str().into());
    let loaded_node = node("yssbi.constant.int64");
    let unloaded_node = node("yssbi.constant.int64");
    let loaded_node_id = loaded_node.id;
    let unloaded_node_id = unloaded_node.id;
    let loaded_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: loaded_node,
    }]);
    let unloaded_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: unloaded_node,
    }]);
    let mut project = ProjectData::new();
    project.graphs.insert(
        loaded_path.clone(),
        GraphResourceDocument::new("LoadedHistory", GraphDocumentKind::Event),
    );
    project.graphs.insert(
        unloaded_path.clone(),
        GraphResourceDocument::new("UnloadedHistory", GraphDocumentKind::Event),
    );
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &loaded_path).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &unloaded_path).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    for (path, key, patch) in [
        (&loaded_path, &loaded_key, loaded_patch.clone()),
        (&unloaded_path, &unloaded_key, unloaded_patch.clone()),
    ] {
        state
            .apply_graph_patch(
                path,
                MutationRequest::new(
                    ResourceKey::Graph(key.clone()),
                    GraphRevision::INITIAL,
                    OperationId::new(),
                    patch,
                ),
            )
            .unwrap();
        crate::project::fixtures::write_state_graph(&state, path).unwrap();
    }
    let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
        OperationId::new(),
        vec![
            crate::node_system::document::ResourcePatch::graph(
                loaded_key.clone(),
                GraphRevision::INITIAL,
                loaded_patch,
            ),
            crate::node_system::document::ResourcePatch::graph(
                unloaded_key.clone(),
                GraphRevision::INITIAL,
                unloaded_patch,
            ),
        ],
    );
    *state.history.write().unwrap() = crate::node_system::document::ProjectHistory::default();
    state
        .history
        .write()
        .unwrap()
        .record_committed_transaction(transaction);
    state.unload_graph_resource(&unloaded_path).unwrap();

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(loaded_key.clone()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let undo_loaded =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &loaded_path)
            .unwrap();
    let undo_unloaded = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &unloaded_path,
    )
    .unwrap();
    assert!(!undo_loaded.document.nodes.contains_key(&loaded_node_id));
    assert!(!undo_unloaded.document.nodes.contains_key(&unloaded_node_id));
    assert!(
        !state.project_data.read().unwrap().graphs[&loaded_path]
            .document
            .nodes
            .contains_key(&loaded_node_id)
    );
    assert!(
        !state
            .project_data
            .read()
            .unwrap()
            .graphs
            .contains_key(&unloaded_path)
    );
    assert_eq!(undo.deltas.len(), 2);
    assert_eq!(undo.projection_replacements.len(), 1);
    assert_eq!(
        undo.projection_replacements[0].graph_path,
        loaded_path.as_str()
    );
    assert!(undo.deltas.iter().all(|delta| {
        delta.from_revision == GraphRevision::new(1) && delta.to_revision == GraphRevision::new(2)
    }));

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(loaded_key),
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let redo_loaded =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &loaded_path)
            .unwrap();
    let redo_unloaded = crate::project::project_io::load_project_graph_document_from_file(
        &root_text,
        &unloaded_path,
    )
    .unwrap();
    assert!(redo_loaded.document.nodes.contains_key(&loaded_node_id));
    assert!(redo_unloaded.document.nodes.contains_key(&unloaded_node_id));
    assert!(
        state.project_data.read().unwrap().graphs[&loaded_path]
            .document
            .nodes
            .contains_key(&loaded_node_id)
    );
    assert!(
        !state
            .project_data
            .read()
            .unwrap()
            .graphs
            .contains_key(&unloaded_path)
    );
    assert_eq!(redo.deltas.len(), 2);
    assert_eq!(redo.projection_replacements.len(), 1);
    assert!(redo.deltas.iter().all(|delta| {
        delta.from_revision == GraphRevision::new(2) && delta.to_revision == GraphRevision::new(3)
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mixed_residency_unloaded_graph_and_global_variable_commit_atomically() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-graph-global-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/GraphGlobal.yssbi-event").unwrap();
    let graph_key = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
    let inserted_node = node("yssbi.constant.int64");
    let inserted_node_id = inserted_node.id;
    let graph_patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted_node,
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
        GraphResourceDocument::new("GraphGlobal", GraphDocumentKind::Event),
    );
    project
        .variables
        .insert(variable_id, before_variable.clone());
    let root_text = root.to_string_lossy().into_owned();
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    crate::project::fixtures::write_graph(&project, &root_text, &graph_path).unwrap();
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
                GraphRevision::INITIAL,
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

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(graph_key),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let graph_disk =
        crate::project::project_io::load_project_graph_document_from_file(&root_text, &graph_path)
            .unwrap();
    let globals = crate::project::project_io::parse_global_variables_document(
        &std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
    )
    .unwrap();
    assert!(!graph_disk.document.nodes.contains_key(&inserted_node_id));
    let expected_variable = serde_json::to_value(&before_variable).unwrap();
    assert_eq!(
        serde_json::to_value(&globals.variables[&variable_id]).unwrap(),
        expected_variable
    );
    assert_eq!(
        serde_json::to_value(&state.get_data().unwrap().variables[&variable_id]).unwrap(),
        expected_variable
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&graph_path));
    assert_eq!(undo.deltas.len(), 2);
    assert!(undo.projection_replacements.is_empty());
    assert!(undo.deltas.iter().all(|delta| {
        delta.from_revision == GraphRevision::new(1) && delta.to_revision == GraphRevision::new(2)
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_reload_clears_history_status() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();
    assert!(state.history_status().can_undo);

    state.activate_project_fixture("replacement-project".into(), ProjectData::new());

    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: false,
        }
    );
}

#[test]
fn committed_editor_mutation_remains_observable_when_projection_fails() {
    let state = state_with_empty_graph();
    state.set_projection_test_hook(std::sync::Arc::new(|| {
        Err("injected projection failure".into())
    }));
    let mut observed = Vec::new();

    let error = state
        .apply_editor_graph_mutation_observed(
            &ProjectInstanceId::from_existing(state.project_instance_id()),
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
            |delta| observed.push(delta.clone()),
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::Projection(_)));
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].graph_path, document_path());
    assert_eq!(observed[0].from_revision, GraphRevision::INITIAL);
    assert_eq!(observed[0].to_revision, GraphRevision::new(1));
    assert_eq!(
        state.get_data().unwrap().graphs[&graph_path()]
            .document
            .revision,
        GraphRevision::new(1)
    );
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
        return_type: Some("float64".into()),
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
        base_revision,
        OperationId::new(),
        crate::node_system::document::FunctionDocumentPatch::new(before, after),
    )
}

fn assert_recovery_blocks_signature(
    state: &ProjectState,
    function_path: &GraphResourcePath,
    resource: ResourceKey,
) {
    let blocked = state.update_function_signature_observed(
        &current_project_instance_id(state),
        function_path,
        "en-US",
        function_signature_request(
            resource,
            GraphRevision::INITIAL,
            Default::default(),
            Default::default(),
        ),
        |_| panic!("recovery-gated mutation must not be observed"),
    );
    assert!(matches!(
        blocked,
        Err(MutationConflict::RecoveryRequired(_))
    ));
}

#[test]
fn committed_signature_undo_redo_return_and_observe_after_recovery_marker() {
    let (signature_state, signature_path, signature_caller, signature_resource) =
        function_state_with_caller("SignatureRecovery");
    let signature_marker = signature_state.project_recovery_marker();
    signature_state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        signature_marker.mark("injected recovery after committed receipt");
    }));
    let signature_observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let signature_events = std::sync::Arc::clone(&signature_observed);
    let signature_result = signature_state
        .update_function_signature_observed(
            &current_project_instance_id(&signature_state),
            &signature_path,
            "en-US",
            function_signature_request(
                signature_resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            move |result| signature_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    assert_eq!(signature_result.publication_revision, 1);
    assert_eq!(
        signature_observed.lock().unwrap().as_slice(),
        &[signature_result.clone()]
    );
    assert_eq!(
        signature_result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                signature_caller.as_str().to_string(),
                signature_path.as_str().to_string(),
            ],
        }
    );
    assert_recovery_blocks_signature(&signature_state, &signature_path, signature_resource);

    let (undo_state, undo_path, undo_caller, undo_resource) =
        function_state_with_caller("UndoRecovery");
    undo_state
        .update_function_signature_observed(
            &current_project_instance_id(&undo_state),
            &undo_path,
            "en-US",
            function_signature_request(
                undo_resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();
    let undo_marker = undo_state.project_recovery_marker();
    undo_state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        undo_marker.mark("injected recovery after committed receipt");
    }));
    let undo_observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let undo_events = std::sync::Arc::clone(&undo_observed);
    let undo_result = undo_state
        .undo_last_transaction_observed(
            &current_project_instance_id(&undo_state),
            "en-US",
            MutationRequest::new(
                undo_resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            move |result| undo_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    assert_eq!(undo_result.publication_revision, 2);
    assert_eq!(
        undo_observed.lock().unwrap().as_slice(),
        &[undo_result.clone()]
    );
    assert_eq!(
        undo_result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                undo_caller.as_str().to_string(),
                undo_path.as_str().to_string(),
            ],
        }
    );
    assert_recovery_blocks_signature(&undo_state, &undo_path, undo_resource);

    let (redo_state, redo_path, redo_caller, redo_resource) =
        function_state_with_caller("RedoRecovery");
    redo_state
        .update_function_signature_observed(
            &current_project_instance_id(&redo_state),
            &redo_path,
            "en-US",
            function_signature_request(
                redo_resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();
    redo_state
        .undo_last_transaction_observed(
            &current_project_instance_id(&redo_state),
            "en-US",
            MutationRequest::new(
                redo_resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let redo_marker = redo_state.project_recovery_marker();
    redo_state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        redo_marker.mark("injected recovery after committed receipt");
    }));
    let redo_observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let redo_events = std::sync::Arc::clone(&redo_observed);
    let redo_result = redo_state
        .redo_last_transaction_observed(
            &current_project_instance_id(&redo_state),
            "en-US",
            MutationRequest::new(
                redo_resource.clone(),
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            move |result| redo_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    assert_eq!(redo_result.publication_revision, 3);
    assert_eq!(
        redo_observed.lock().unwrap().as_slice(),
        &[redo_result.clone()]
    );
    assert_eq!(
        redo_result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                redo_caller.as_str().to_string(),
                redo_path.as_str().to_string(),
            ],
        }
    );
    assert_recovery_blocks_signature(&redo_state, &redo_path, redo_resource);
}

#[test]
fn committed_projection_failure_after_recovery_marker_returns_incomplete() {
    let (state, function_path, caller_path, resource) =
        function_state_with_caller("ProjectionRecovery");
    state.set_projection_test_hook(std::sync::Arc::new(|| {
        Err("injected projection failure".into())
    }));
    let marker = state.project_recovery_marker();
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        marker.mark("injected recovery after committed receipt");
    }));
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let events = std::sync::Arc::clone(&observed);

    let result = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource,
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            move |result| events.lock().unwrap().push(result.clone()),
        )
        .unwrap();

    assert_eq!(observed.lock().unwrap().as_slice(), &[result.clone()]);
    assert!(result.projection_replacements.is_empty());
    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
}

#[test]
fn committed_variable_effect_returns_canonical_result_after_recovery_marker() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-recovery-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let variable = test_variable("Recovery Variable");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let marker = state.project_recovery_marker();
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        marker.mark("injected recovery after committed receipt");
    }));
    let resource_id =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();

    let committed = state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: resource_id.clone(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    let result = committed.resource_mutation.unwrap();
    assert_eq!(result.publication_revision, 1);
    assert_eq!(result.deltas.len(), 1);
    assert_eq!(
        result.deltas[0].resource,
        ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
            resource_id.as_str().into()
        ))
    );
    assert_eq!(result.deltas[0].from_revision, GraphRevision::INITIAL);
    assert_eq!(result.deltas[0].to_revision, GraphRevision::new(1));
    assert_eq!(
        result.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        }
    );

    let mut updated = variable;
    updated.data_value = crate::graph::value::DataValue::Int64(2);
    let blocked = state.commit_variable_effects(
        &session_id,
        vec![crate::node_system::runtime::VariableWriteEffect {
            resource: resource_id,
            expected_revision: GraphRevision::new(1),
            before: updated,
            after: crate::graph::value::DataValue::Int64(3),
        }],
    );
    assert!(matches!(
        blocked,
        Err(VariableEffectCommitError::Persistence { .. })
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn projection_environment_capture_is_activation_ordered_and_coherent() {
    let root_a = std::env::temp_dir().join(format!(
        "yssbi-projection-environment-a-{}",
        uuid::Uuid::new_v4()
    ));
    let root_b = std::env::temp_dir().join(format!(
        "yssbi-projection-environment-b-{}",
        uuid::Uuid::new_v4()
    ));
    let project_with_database = |root: &std::path::Path, id: &str, column: &str| {
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(column => [1_i64, 2, 3]).unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();
        let mut project = ProjectData::new();
        project.databases.insert(
            id.into(),
            crate::database::DatabaseDecl {
                id: id.into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: Some(id.into()),
            },
        );
        project
    };
    let project_a = project_with_database(&root_a, "a", "column_a");
    let project_b = project_with_database(&root_b, "b", "column_b");
    let path_a = root_a.to_string_lossy().into_owned();
    let path_b = root_b.to_string_lossy().into_owned();
    let state = ProjectState::new();
    state.activate_project_fixture(path_a, project_a);

    let (path_locked_tx, path_locked_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    let first_capture = std::sync::atomic::AtomicBool::new(true);
    state.set_projection_environment_capture_test_hook(std::sync::Arc::new(move || {
        if first_capture.swap(false, std::sync::atomic::Ordering::AcqRel) {
            path_locked_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));
    let (capture_done_tx, capture_done_rx) = std::sync::mpsc::channel();
    let capture_state = state.clone();
    std::thread::spawn(move || {
        capture_done_tx
            .send(capture_state.capture_projection_environment_for_test())
            .unwrap();
    });
    path_locked_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    let (activation_started_tx, activation_started_rx) = std::sync::mpsc::channel();
    state.set_project_activation_test_hook(std::sync::Arc::new(move || {
        activation_started_tx.send(()).unwrap();
    }));
    let (activation_done_tx, activation_done_rx) = std::sync::mpsc::channel();
    let activation_state = state.clone();
    let path_b_for_activation = path_b.clone();
    std::thread::spawn(move || {
        activation_state.activate_project_fixture(path_b_for_activation, project_b);
        activation_done_tx.send(()).unwrap();
    });
    activation_started_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    release_capture_tx.send(()).unwrap();

    let capture = capture_done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("projection environment capture must not deadlock");
    activation_done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("activation must complete after coherent environment capture");
    let database_a = crate::node_system::plan::ResourceId::new("databases/a").unwrap();
    let database_b = crate::node_system::plan::ResourceId::new("databases/b").unwrap();
    match capture {
        Ok(environment) => {
            assert!(environment.database_schemas.contains_key(&database_a));
            assert!(!environment.database_schemas.contains_key(&database_b));
        }
        Err(error) => assert!(error.contains("stale_project_lifecycle")),
    }
    let current_root =
        NormalizedProjectRoot::from_project_path(state.get_path().as_deref().unwrap()).unwrap();
    let expected_root = NormalizedProjectRoot::from_project_path(&path_b).unwrap();
    assert_eq!(current_root, expected_root);
    let data = state.get_data().unwrap();
    assert!(data.databases.contains_key("b"));
    assert!(!data.databases.contains_key("a"));
    std::fs::remove_dir_all(root_a).unwrap();
    std::fs::remove_dir_all(root_b).unwrap();
}

#[test]
fn projection_environment_capture_rejects_store_from_overlapping_activation() {
    let root_a = std::env::temp_dir().join(format!(
        "yssbi-projection-overlap-a-{}",
        uuid::Uuid::new_v4()
    ));
    let root_b = std::env::temp_dir().join(format!(
        "yssbi-projection-overlap-b-{}",
        uuid::Uuid::new_v4()
    ));
    let project_with_database = |root: &std::path::Path, id: &str, column: &str| {
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(column => [1_i64, 2, 3]).unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();
        let mut project = ProjectData::new();
        project.databases.insert(
            id.into(),
            crate::database::DatabaseDecl {
                id: id.into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: Some(id.into()),
            },
        );
        project
    };
    let project_a = project_with_database(&root_a, "a", "column_a");
    let project_b = project_with_database(&root_b, "b", "column_b");
    let state = ProjectState::new();
    state.activate_project_fixture(root_a.to_string_lossy().into_owned(), project_a);
    let expected_session = state.capture_project_session().unwrap();

    let (path_data_released_tx, path_data_released_rx) = std::sync::mpsc::channel();
    let (resume_capture_tx, resume_capture_rx) = std::sync::mpsc::channel();
    let resume_capture_rx = std::sync::Mutex::new(resume_capture_rx);
    let first_capture = std::sync::atomic::AtomicBool::new(true);
    state.set_projection_environment_after_path_data_test_hook(std::sync::Arc::new(move || {
        if first_capture.swap(false, std::sync::atomic::Ordering::AcqRel) {
            path_data_released_tx.send(()).unwrap();
            resume_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));
    let (capture_done_tx, capture_done_rx) = std::sync::mpsc::channel();
    let capture_state = state.clone();
    std::thread::spawn(move || {
        capture_done_tx
            .send(
                capture_state
                    .capture_projection_environment_for_session_for_test(&expected_session),
            )
            .unwrap();
    });
    path_data_released_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    state.activate_project_fixture(root_b.to_string_lossy().into_owned(), project_b);
    resume_capture_tx.send(()).unwrap();
    let capture = capture_done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("overlapping capture must not deadlock");
    let error = match capture {
        Ok(_) => panic!("overlapping capture must reject mixed activation inputs"),
        Err(error) => error,
    };
    assert!(error.contains("stale_project_lifecycle"));

    let current_session = state.capture_project_session().unwrap();
    let environment = state
        .capture_projection_environment_for_session_for_test(&current_session)
        .unwrap();
    let database_a = crate::node_system::plan::ResourceId::new("databases/a").unwrap();
    let database_b = crate::node_system::plan::ResourceId::new("databases/b").unwrap();
    assert!(!environment.database_schemas.contains_key(&database_a));
    assert!(environment.database_schemas.contains_key(&database_b));
    assert!(state.get_data().unwrap().databases.contains_key("b"));
    std::fs::remove_dir_all(root_a).unwrap();
    std::fs::remove_dir_all(root_b).unwrap();
}

#[test]
fn committed_projection_uses_precommit_database_metadata_after_removal() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-committed-projection-metadata-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("captured_column" => [1_i64, 2, 3]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let function_path =
        GraphResourcePath::new("functions/MetadataSnapshot.yssbi-function").unwrap();
    let caller_path = GraphResourcePath::new("events/MetadataSnapshotCaller.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: Some("Main".into()),
        },
    );
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
    state
        .insert_graph(
            function_path.clone(),
            GraphResourceDocument::new("MetadataSnapshot", GraphDocumentKind::Function),
        )
        .unwrap();
    let mut caller =
        GraphResourceDocument::new("Metadata Snapshot Caller", GraphDocumentKind::Event);
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    caller.document.nodes.insert(call.id, call);
    let mut source = node("yssbi.dataframe.source.get");
    source.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    caller.document.nodes.insert(source.id, source);
    state.insert_graph(caller_path.clone(), caller).unwrap();
    let duckdb_for_hook = duckdb.clone();
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        std::fs::remove_file(&duckdb_for_hook).unwrap();
    }));

    let result = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();

    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
    assert_eq!(result.projection_replacements.len(), 2);
    assert!(!duckdb.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_resource_observer_and_response_serialize_identically() {
    let (state, function_path, _, resource) = function_state_with_caller("CanonicalResult");
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let signature_events = std::sync::Arc::clone(&observed);
    let signature = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource.clone(),
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            move |result| signature_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    let signature_observed = observed.lock().unwrap().pop().unwrap();
    assert_eq!(
        serde_json::to_value(signature).unwrap(),
        serde_json::to_value(signature_observed).unwrap()
    );
    assert!(observed.lock().unwrap().is_empty());

    let undo_events = std::sync::Arc::clone(&observed);
    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            move |result| undo_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    let undo_observed = observed.lock().unwrap().pop().unwrap();
    assert_eq!(
        serde_json::to_value(undo).unwrap(),
        serde_json::to_value(undo_observed).unwrap()
    );
    assert!(observed.lock().unwrap().is_empty());

    let redo_events = std::sync::Arc::clone(&observed);
    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            move |result| redo_events.lock().unwrap().push(result.clone()),
        )
        .unwrap();
    let redo_observed = observed.lock().unwrap().pop().unwrap();
    assert_eq!(
        serde_json::to_value(redo).unwrap(),
        serde_json::to_value(redo_observed).unwrap()
    );
    assert!(observed.lock().unwrap().is_empty());
}

#[test]
fn committed_graph_source_is_rejected_after_interleaved_undo() {
    let state = state_with_empty_graph();
    let (projection_started_tx, projection_started_rx) = std::sync::mpsc::channel();
    let (release_projection_tx, release_projection_rx) = std::sync::mpsc::channel();
    let release_projection_rx = std::sync::Mutex::new(release_projection_rx);
    let first_projection = std::sync::atomic::AtomicBool::new(true);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if first_projection.swap(false, std::sync::atomic::Ordering::AcqRel) {
            projection_started_tx.send(()).unwrap();
            release_projection_rx.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }));
    let mutation_project_instance_id =
        ProjectInstanceId::from_existing(state.project_instance_id());
    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        mutation_state.apply_editor_graph_mutation(
            &mutation_project_instance_id,
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
    });
    projection_started_rx.recv().unwrap();

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    release_projection_tx.send(()).unwrap();
    let error = mutation.join().unwrap().unwrap_err();

    assert!(matches!(error, MutationConflict::Projection(_)));
    assert_eq!(
        state.history_status(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: false,
            can_redo: true,
        }
    );
}

#[test]
fn history_status_waits_for_authoritative_publication() {
    let state = state_with_empty_graph();
    let (history_changed_tx, history_changed_rx) = std::sync::mpsc::channel();
    let (release_publication_tx, release_publication_rx) = std::sync::mpsc::channel();
    let release_publication_rx = std::sync::Mutex::new(release_publication_rx);
    state.set_authoritative_publication_test_hook(std::sync::Arc::new(move || {
        history_changed_tx.send(()).unwrap();
        release_publication_rx
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("bounded authoritative publication release");
    }));
    let mutation_state = state.clone();
    let (mutation_done_tx, mutation_done_rx) = std::sync::mpsc::channel();
    let mutation = std::thread::spawn(move || {
        let result = mutation_state.apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        );
        mutation_done_tx.send(()).unwrap();
        result
    });
    history_changed_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("mutation reached authoritative publication checkpoint");

    let (status_tx, status_rx) = std::sync::mpsc::channel();
    let status_state = state.clone();
    let status = std::thread::spawn(move || {
        status_tx.send(status_state.history_status()).unwrap();
    });
    assert!(
        status_rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err()
    );

    release_publication_tx.send(()).unwrap();
    mutation_done_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("mutation completed after publication release");
    mutation.join().unwrap().unwrap();
    assert_eq!(
        status_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("status completed after authoritative publication"),
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
    status.join().unwrap();
    assert_eq!(
        state.get_data().unwrap().graphs[&graph_path()]
            .document
            .revision,
        GraphRevision::new(1)
    );
}

#[test]
fn normalized_graph_lifecycle_routes_every_insert_through_project_state() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-lifecycle-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());

    let created = state
        .create_graph_resource_fixture("Lifecycle", GraphDocumentKind::Event)
        .unwrap();
    assert!(!state.get_data().unwrap().graphs.contains_key(&created));
    let loaded = load_graph(&state, &created).unwrap();
    assert_eq!(loaded.name, "Lifecycle");
    crate::project::fixtures::write_state_graph(&state, &created).unwrap();
    state.unload_graph_resource(&created);

    let duplicated = state.duplicate_graph_resource_fixture(&created).unwrap();
    assert_ne!(duplicated, created);
    assert!(!state.get_data().unwrap().graphs.contains_key(&duplicated));
    let project_instance_id = state.project_instance_id();
    let renamed = state
        .rename_graph_resource_fixture(&project_instance_id, &duplicated, "Lifecycle Copy Renamed")
        .unwrap();
    assert_ne!(renamed, duplicated);
    state.remove_graph_resource_fixture(&created).unwrap();
    state.remove_graph_resource_fixture(&renamed).unwrap();

    let index = crate::project::read_project_index(root.to_string_lossy().as_ref()).unwrap();
    assert!(index.graphs.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_duplicate_rebinds_self_identity_and_loaded_rename_is_authoritative() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-resource-identity-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let caller = state
        .create_graph_resource_fixture("Caller", GraphDocumentKind::Event)
        .unwrap();
    let function = state
        .create_graph_resource_fixture("Callee", GraphDocumentKind::Function)
        .unwrap();
    load_graph(&state, &caller).unwrap();
    load_graph(&state, &function).unwrap();
    let local_variable = state
        .add_variable(
            "Local Rate",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(9),
            "",
            crate::variable::VariableScope::Function {
                function_path: function.as_str().into(),
            },
            Vec::new(),
        )
        .unwrap();

    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function.as_str()),
    );
    state
        .apply_graph_patch(
            &caller,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    caller.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: call }]),
            ),
        )
        .unwrap();
    let duplicated = state.duplicate_graph_resource_fixture(&function).unwrap();
    let duplicate = load_graph(&state, &duplicated).unwrap();
    for shell in duplicate.document.nodes.values().filter(|node| {
        matches!(
            node.node_type.as_str(),
            "yssbi.project.function.entry" | "yssbi.project.function.return"
        )
    }) {
        assert_eq!(
            shell
                .parameters
                .iter()
                .find(|(key, _)| key.as_str() == "function")
                .and_then(|(_, value)| value.as_str()),
            Some(duplicated.as_str())
        );
    }

    let project_instance_id = state.project_instance_id();
    let caller_before_rename = state.get_data().unwrap().graphs[&caller].document.revision;
    let moved_before_rename = state.get_data().unwrap().graphs[&function]
        .document
        .revision;
    let renamed = state
        .rename_graph_resource_fixture(&project_instance_id, &function, "Renamed Callee")
        .unwrap();
    assert_eq!(renamed.publication.moves.len(), 1);
    assert_eq!(renamed.publication.moves[0].from, function.as_str());
    assert_eq!(renamed.publication.moves[0].to, renamed.path.as_str());
    let graph_deltas = renamed
        .publication
        .deltas
        .iter()
        .filter(|delta| matches!(delta.resource, ResourceKey::Graph(_)))
        .collect::<Vec<_>>();
    assert_eq!(graph_deltas.len(), 2);
    assert!(graph_deltas.iter().any(|delta| {
        delta.resource
            == ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                renamed.path.as_str().into(),
            ))
            && delta.from_revision == moved_before_rename
            && delta.to_revision == moved_before_rename.next()
    }));
    assert!(graph_deltas.iter().any(|delta| {
        delta.resource
            == ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                caller.as_str().into(),
            ))
            && delta.from_revision == caller_before_rename
            && delta.to_revision == caller_before_rename.next()
    }));
    assert!(renamed.publication.history.can_undo);
    let data = state.get_data().unwrap();
    let loaded_caller = &data.graphs[&caller];
    assert!(loaded_caller.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(renamed.as_str()))
    }));
    assert!(!loaded_caller.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(function.as_str()))
    }));
    assert_eq!(
        data.variables[&local_variable.id].scope,
        crate::variable::VariableScope::Function {
            function_path: renamed.as_str().into(),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn activation_releases_root_lease_before_run_drain() {
    let (state, root) = state_with_project_path("activation-root-lease");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let normalized = NormalizedProjectRoot::from_project_path(&root).unwrap();
    let filesystem = state.filesystem().clone();
    state.set_project_activation_test_hook(std::sync::Arc::new(move || {
        assert!(!filesystem.is_reserved_for_test(&normalized));
    }));

    state.activate_project_from_path(&root).unwrap();

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unloaded_rename_captures_source_revision_without_panicking() {
    let (state, root) = state_with_project_path("unloaded-rename-revision");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("Unloaded", GraphDocumentKind::Event)
        .unwrap();
    assert!(!state.get_data().unwrap().graphs.contains_key(&source));

    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Renamed Unloaded")
        .unwrap();

    assert_eq!(renamed.publication.moves[0].from, source.as_str());
    assert_eq!(renamed.publication.moves[0].to, renamed.path.as_str());
    assert_eq!(
        renamed.publication.deltas[0].from_revision,
        GraphRevision::INITIAL
    );
    assert_eq!(
        renamed.publication.deltas[0].to_revision,
        GraphRevision::new(1)
    );
    assert!(root.join(renamed.path.as_str()).is_file());
    assert!(!root.join(source.as_str()).exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn loaded_rename_undo_redo_restores_disk_authority_and_move_identity() {
    let (state, root) = state_with_project_path("loaded-rename-history");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("History Source", GraphDocumentKind::Event)
        .unwrap();
    let caller = state
        .create_graph_resource_fixture("History Caller", GraphDocumentKind::Event)
        .unwrap();
    load_graph(&state, &source).unwrap();
    load_graph(&state, &caller).unwrap();
    let mut reference = node("yssbi.test.reference");
    reference.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(source.as_str()),
    );
    state
        .apply_graph_patch(
            &caller,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    caller.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: reference,
                }]),
            ),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &caller).unwrap();
    let variable = state
        .add_variable(
            "Scoped",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(1),
            "",
            crate::variable::VariableScope::Event {
                event_path: source.as_str().into(),
            },
            Vec::new(),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &source).unwrap();

    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "History Renamed")
        .unwrap();
    let target = renamed.path.clone();
    let target_revision = renamed
        .publication
        .deltas
        .iter()
        .find(|delta| matches!(&delta.resource, ResourceKey::Graph(path) if path.0.as_ref() == target.as_str()))
        .unwrap()
        .to_revision;
    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    target.as_str().into(),
                )),
                target_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(renamed.publication.moves[0].name, "History Renamed");
    assert_eq!(undo.moves[0].from, target.as_str());
    assert_eq!(undo.moves[0].to, source.as_str());
    assert_eq!(undo.moves[0].name, "History Source");
    assert!(root.join(source.as_str()).is_file());
    assert!(!root.join(target.as_str()).exists());
    assert!(state.get_data().unwrap().graphs.contains_key(&source));
    assert!(
        state.get_data().unwrap().graphs[&caller]
            .document
            .nodes
            .values()
            .any(|node| {
                node.parameters
                    .values()
                    .any(|value| value.as_str() == Some(source.as_str()))
            })
    );
    assert_eq!(
        state.get_data().unwrap().variables[&variable.id].scope,
        variable.scope
    );

    let source_revision = undo
        .deltas
        .iter()
        .find(|delta| matches!(&delta.resource, ResourceKey::Graph(path) if path.0.as_ref() == source.as_str()))
        .unwrap()
        .to_revision;
    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    source.as_str().into(),
                )),
                source_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(redo.moves[0].from, source.as_str());
    assert_eq!(redo.moves[0].to, target.as_str());
    assert_eq!(redo.moves[0].name, "History Renamed");
    assert!(root.join(target.as_str()).is_file());
    assert!(!root.join(source.as_str()).exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn concurrent_history_append_during_rename_undo_rolls_back_disk_without_moving_history_head() {
    let (state, root) = state_with_project_path("rename-history-head-race");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("Move Head", GraphDocumentKind::Event)
        .unwrap();
    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Moved Head")
        .unwrap();
    let target = renamed.path.clone();
    let target_revision = renamed.publication.deltas[0].to_revision;
    let concurrent_state = state.clone();
    state.set_graph_move_history_io_checkpoint(std::sync::Arc::new(move || {
        concurrent_state.append_history_head_for_test();
    }));

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    target.as_str().into(),
                )),
                target_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| panic!("changed history head must not publish"),
        )
        .unwrap_err();

    assert!(error.to_string().contains("history head changed"));
    assert!(root.join(target.as_str()).is_file());
    assert!(!root.join(source.as_str()).exists());
    assert!(state.history_status().can_undo);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unloaded_caller_delta_revision_and_history_follow_graph_move() {
    let (state, root) = state_with_project_path("unloaded-caller-move-history");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("Unloaded Callee", GraphDocumentKind::Event)
        .unwrap();
    let caller = state
        .create_graph_resource_fixture("Unloaded Caller", GraphDocumentKind::Event)
        .unwrap();
    load_graph(&state, &caller).unwrap();
    let mut reference = node("yssbi.test.reference");
    reference.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(source.as_str()),
    );
    state
        .apply_graph_patch(
            &caller,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    caller.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: reference,
                }]),
            ),
        )
        .unwrap();
    crate::project::fixtures::write_state_graph(&state, &caller).unwrap();
    state.unload_graph_resource(&caller).unwrap();

    assert!(!state.get_data().unwrap().graphs.contains_key(&caller));
    assert_eq!(
        state.revision_state_for_test().0.get(&caller),
        Some(&GraphRevision::new(1))
    );

    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Renamed Callee")
        .unwrap();
    let target = renamed.path.clone();
    let caller_delta = renamed
        .publication
        .deltas
        .iter()
        .find(|delta| matches!(&delta.resource, ResourceKey::Graph(path) if path.0.as_ref() == caller.as_str()))
        .expect("unloaded caller delta");
    assert_eq!(caller_delta.from_revision, GraphRevision::new(1));
    assert_eq!(caller_delta.to_revision, GraphRevision::new(2));
    assert!(!state.get_data().unwrap().graphs.contains_key(&caller));
    let caller_after =
        load_project_graph_from_file(root.to_string_lossy().as_ref(), &caller).unwrap();
    assert!(caller_after.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(target.as_str()))
    }));

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    target.as_str().into(),
                )),
                renamed.publication.deltas[0].to_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let undo_caller = undo
        .deltas
        .iter()
        .find(|delta| matches!(&delta.resource, ResourceKey::Graph(path) if path.0.as_ref() == caller.as_str()))
        .expect("unloaded caller undo delta");
    assert_eq!(undo_caller.from_revision, GraphRevision::new(2));
    assert_eq!(undo_caller.to_revision, GraphRevision::new(3));
    let caller_undone =
        load_project_graph_from_file(root.to_string_lossy().as_ref(), &caller).unwrap();
    assert!(caller_undone.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(source.as_str()))
    }));

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    source.as_str().into(),
                )),
                undo.deltas[0].to_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let redo_caller = redo
        .deltas
        .iter()
        .find(|delta| matches!(&delta.resource, ResourceKey::Graph(path) if path.0.as_ref() == caller.as_str()))
        .expect("unloaded caller redo delta");
    assert_eq!(redo_caller.from_revision, GraphRevision::new(3));
    assert_eq!(redo_caller.to_revision, GraphRevision::new(4));
    let caller_redone =
        load_project_graph_from_file(root.to_string_lossy().as_ref(), &caller).unwrap();
    assert!(caller_redone.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(target.as_str()))
    }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unloaded_rename_undo_redo_restores_disk_identity() {
    let (state, root) = state_with_project_path("unloaded-rename-history");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let source = state
        .create_graph_resource_fixture("Unloaded History", GraphDocumentKind::Event)
        .unwrap();
    let renamed = state
        .rename_graph_resource_fixture(
            &state.project_instance_id(),
            &source,
            "Unloaded History Renamed",
        )
        .unwrap();
    let target = renamed.path.clone();
    let target_revision = renamed.publication.deltas[0].to_revision;

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    target.as_str().into(),
                )),
                target_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(undo.moves[0].to, source.as_str());
    assert!(root.join(source.as_str()).is_file());
    let source_revision = undo.deltas[0].to_revision;

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    source.as_str().into(),
                )),
                source_revision,
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(redo.moves[0].to, target.as_str());
    assert!(root.join(target.as_str()).is_file());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn recovery_gate_rejects_public_snapshots_queries_and_variable_mutations() {
    let (state, root) = state_with_project_path("recovery-public-authority");
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    state.project_recovery_marker().mark("recovery required");

    assert_eq!(
        state.get_data().unwrap_err().code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .add_variable(
                "blocked",
                crate::graph::value::DataType::Int64,
                crate::graph::value::DataValue::Int64(1),
                "",
                crate::variable::VariableScope::Global,
                Vec::new(),
            )
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );
    assert_eq!(
        state
            .read_project_index(&project_instance_id)
            .unwrap_err()
            .code(),
        "project_recovery_required"
    );

    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    assert!(state.get_data().is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_replacement_during_function_loading_cancels_before_old_resource_insert() {
    let old_root = std::env::temp_dir().join(format!(
        "yssbi-production-loading-old-{}",
        uuid::Uuid::new_v4()
    ));
    let new_root = std::env::temp_dir().join(format!(
        "yssbi-production-loading-new-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&old_root).unwrap();
    std::fs::create_dir_all(&new_root).unwrap();
    crate::project::fixtures::write_project(
        &ProjectData::new(),
        old_root.to_string_lossy().as_ref(),
    )
    .unwrap();
    crate::project::fixtures::write_project(
        &ProjectData::new(),
        new_root.to_string_lossy().as_ref(),
    )
    .unwrap();

    let state = ProjectState::new();
    state.activate_project_fixture(old_root.to_string_lossy().into_owned(), ProjectData::new());
    let event = state
        .create_graph_resource_fixture("Loading Caller", GraphDocumentKind::Event)
        .unwrap();
    let old_function = state
        .create_graph_resource_fixture("Loading Callee", GraphDocumentKind::Function)
        .unwrap();
    load_graph(&state, &event).unwrap();

    let (loading_tx, loading_rx) = std::sync::mpsc::channel();
    state.set_function_load_checkpoint(std::sync::Arc::new(
        move |cancellation: &crate::node_system::runtime::CancellationToken| {
            loading_tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !cancellation.is_cancelled() && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(cancellation.is_cancelled());
        },
    ));

    let executing_state = state.clone();
    let execution = std::thread::spawn(move || {
        executing_state.execute_graph_for_current_project_for_test(
            &event,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
    });
    loading_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();

    let replacement_state = state.clone();
    let replacement_path = new_root.to_string_lossy().into_owned();
    let replacement = std::thread::spawn(move || {
        replacement_state.activate_project_fixture(replacement_path, ProjectData::new());
    });

    let error = execution.join().unwrap().unwrap_err();
    assert!(
        error.contains("cancel"),
        "unexpected execution error: {error}"
    );
    replacement.join().unwrap();
    assert!(!state.get_data().unwrap().graphs.contains_key(&old_function));
    assert_eq!(
        state.capture_project_session().unwrap().root,
        NormalizedProjectRoot::from_project_path(&new_root).unwrap()
    );

    std::fs::remove_dir_all(old_root).unwrap();
    std::fs::remove_dir_all(new_root).unwrap();
}

#[test]
fn normalized_function_signature_update_is_undoable() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "normalized-function-signature-undo",
        ProjectData::new(),
    ));
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let path = GraphResourcePath::new("functions/Tax.yssbi-function").unwrap();
    state
        .insert_graph(
            path.clone(),
            GraphResourceDocument::new("Tax", GraphDocumentKind::Function),
        )
        .unwrap();
    let signature = crate::node_system::document::FunctionSignature {
        parameters: vec![crate::node_system::document::FunctionParameter {
            id: crate::node_system::document::FunctionParameterId("amount".into()),
            name: "Amount".into(),
            type_name: "float64".into(),
        }],
        return_type: Some("float64".into()),
    };

    let operation_id = OperationId::new();
    let result = state
        .update_function_signature_observed(
            &project_instance_id,
            &path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    path.as_str().into(),
                )),
                crate::node_system::document::ResourceRevision::INITIAL,
                operation_id,
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    signature.clone(),
                ),
            ),
            |_| {},
        )
        .unwrap();
    let delta = &result.deltas[0];
    assert_eq!(delta.from_revision.get(), 0);
    assert_eq!(delta.to_revision.get(), 1);
    assert_eq!(delta.caused_by, Some(operation_id));
    assert_eq!(
        state.get_data().unwrap().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        signature
    );
    state
        .undo_last_transaction_observed(
            &project_instance_id,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    path.as_str().into(),
                )),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        state.get_data().unwrap().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        crate::node_system::document::FunctionSignature::default()
    );
}

#[test]
fn revisioned_signature_undo_and_redo_reject_conflicts_and_return_deltas() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "revisioned-signature-history",
        ProjectData::new(),
    ));
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let path = GraphResourcePath::new("functions/Revisioned.yssbi-function").unwrap();
    state
        .insert_graph(
            path.clone(),
            GraphResourceDocument::new("Revisioned", GraphDocumentKind::Function),
        )
        .unwrap();
    let resource = ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
        path.as_str().into(),
    ));
    let signature = crate::node_system::document::FunctionSignature {
        parameters: vec![crate::node_system::document::FunctionParameter {
            id: crate::node_system::document::FunctionParameterId("value".into()),
            name: "Value".into(),
            type_name: "float64".into(),
        }],
        return_type: Some("float64".into()),
    };
    let patch = crate::node_system::document::FunctionDocumentPatch::new(
        Default::default(),
        signature.clone(),
    );
    let signature_operation = OperationId::new();
    let signature_result = state
        .update_function_signature_observed(
            &project_instance_id,
            &path,
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::INITIAL,
                signature_operation,
                patch.clone(),
            ),
            |_| {},
        )
        .unwrap();
    let signature_delta = &signature_result.deltas[0];
    assert_eq!(signature_delta.caused_by, Some(signature_operation));
    assert_eq!(signature_delta.from_revision, GraphRevision::INITIAL);
    assert_eq!(signature_delta.to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.update_function_signature_observed(
            &project_instance_id,
            &path,
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
            |_| {},
        ),
        Err(MutationConflict::StaleRevision { .. })
    ));

    let stale_undo = MutationRequest::new(
        resource.clone(),
        GraphRevision::INITIAL,
        OperationId::new(),
        HistoryMutation {},
    );
    assert!(matches!(
        state.undo_last_transaction_observed(&project_instance_id, "en-US", stale_undo, |_| {}),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let undo_operation = OperationId::new();
    let undo_result = state
        .undo_last_transaction_observed(
            &project_instance_id,
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                undo_operation,
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let undo_deltas = &undo_result.deltas;
    assert_eq!(undo_deltas.len(), 1);
    assert_eq!(undo_deltas[0].resource, resource);
    assert_eq!(undo_deltas[0].from_revision, GraphRevision::new(1));
    assert_eq!(undo_deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo_deltas[0].caused_by, Some(undo_operation));
    assert_eq!(
        state.get_data().unwrap().graphs[&path].document.revision,
        undo_deltas[0].to_revision
    );
    assert_eq!(
        state.revision_state_for_test().0[&path],
        undo_deltas[0].to_revision
    );

    let stale_redo = MutationRequest::new(
        undo_deltas[0].resource.clone(),
        GraphRevision::new(1),
        OperationId::new(),
        HistoryMutation {},
    );
    assert!(matches!(
        state.redo_last_transaction_observed(&project_instance_id, "en-US", stale_redo, |_| {}),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let redo_operation = OperationId::new();
    let redo_result = state
        .redo_last_transaction_observed(
            &project_instance_id,
            "en-US",
            MutationRequest::new(
                undo_deltas[0].resource.clone(),
                GraphRevision::new(2),
                redo_operation,
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let redo_deltas = &redo_result.deltas;
    assert_eq!(redo_deltas.len(), 1);
    assert_eq!(redo_deltas[0].from_revision, GraphRevision::new(2));
    assert_eq!(redo_deltas[0].to_revision, GraphRevision::new(3));
    assert_eq!(redo_deltas[0].caused_by, Some(redo_operation));
    assert_eq!(
        state.get_data().unwrap().graphs[&path].document.revision,
        redo_deltas[0].to_revision
    );
    assert_eq!(
        state.revision_state_for_test().0[&path],
        redo_deltas[0].to_revision
    );
    assert_eq!(
        state.get_data().unwrap().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        signature
    );
}

#[test]
fn signature_result_declares_function_and_caller_projection_paths_without_caller_delta() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "signature-result-projection-paths",
        ProjectData::new(),
    ));
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let function_path = GraphResourcePath::new("functions/Declared.yssbi-function").unwrap();
    let caller_path = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    state
        .insert_graph(
            function_path.clone(),
            GraphResourceDocument::new("Declared", GraphDocumentKind::Function),
        )
        .unwrap();
    let mut caller = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    caller.document.nodes.insert(call.id, call);
    state.insert_graph(caller_path.clone(), caller).unwrap();

    let operation_id = OperationId::new();
    let result = state
        .update_function_signature_observed(
            &project_instance_id,
            &function_path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                operation_id,
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    crate::node_system::document::FunctionSignature {
                        parameters: Vec::new(),
                        return_type: Some("Float64".into()),
                    },
                ),
            ),
            |_| {},
        )
        .unwrap();

    assert_eq!(result.deltas.len(), 1);
    assert_eq!(result.deltas[0].caused_by, Some(operation_id));
    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
    assert_eq!(
        result
            .projection_replacements
            .iter()
            .map(|replacement| replacement.graph_path.as_str())
            .collect::<Vec<_>>(),
        vec![caller_path.as_str(), function_path.as_str()]
    );
}

#[test]
fn concurrent_function_results_keep_commit_publication_order_without_locking_projection() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "concurrent-function-publication-order",
        ProjectData::new(),
    ));
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let first_path = GraphResourcePath::new("functions/First.yssbi-function").unwrap();
    let second_path = GraphResourcePath::new("functions/Second.yssbi-function").unwrap();
    let caller_path = GraphResourcePath::new("events/SharedCaller.yssbi-event").unwrap();
    for (path, name) in [(&first_path, "First"), (&second_path, "Second")] {
        state
            .insert_graph(
                path.clone(),
                GraphResourceDocument::new(name, GraphDocumentKind::Function),
            )
            .unwrap();
    }
    let mut caller = GraphResourceDocument::new("SharedCaller", GraphDocumentKind::Event);
    for function_path in [&first_path, &second_path] {
        let mut call = node("yssbi.project.function.call");
        call.parameters.insert(
            crate::node_system::protocol::ParameterKey::new("target").unwrap(),
            serde_json::json!(function_path.as_str()),
        );
        caller.document.nodes.insert(call.id, call);
    }
    state.insert_graph(caller_path, caller).unwrap();

    let rendezvous_timeout = std::time::Duration::from_secs(2);
    let (first_projection_tx, first_projection_rx) = std::sync::mpsc::channel();
    let (release_first_tx, release_first_rx) = std::sync::mpsc::channel();
    let release_first_rx = std::sync::Mutex::new(release_first_rx);
    let projection_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = std::sync::Arc::clone(&projection_calls);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            first_projection_tx
                .send(())
                .map_err(|error| format!("failed to announce the first projection: {error}"))?;
            release_first_rx
                .lock()
                .unwrap()
                .recv_timeout(rendezvous_timeout)
                .map_err(|error| {
                    format!("timed out waiting to release the first projection: {error}")
                })?;
        }
        Ok(())
    }));

    let (published_tx, published_rx) = std::sync::mpsc::channel();
    let spawn_signature = |path: GraphResourcePath, return_type: &'static str| {
        let mutation_state = state.clone();
        let project_instance_id = project_instance_id.clone();
        let published_tx = published_tx.clone();
        std::thread::spawn(move || {
            mutation_state.update_function_signature_observed(
                &project_instance_id,
                &path,
                "en-US",
                MutationRequest::new(
                    ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                        path.as_str().into(),
                    )),
                    GraphRevision::INITIAL,
                    OperationId::new(),
                    crate::node_system::document::FunctionDocumentPatch::new(
                        Default::default(),
                        crate::node_system::document::FunctionSignature {
                            parameters: Vec::new(),
                            return_type: Some(return_type.into()),
                        },
                    ),
                ),
                move |result| {
                    published_tx
                        .send(
                            serde_json::to_value(result).unwrap()["publicationRevision"]
                                .as_u64()
                                .unwrap(),
                        )
                        .expect("publication observer receiver must remain available");
                },
            )
        })
    };

    let first = spawn_signature(first_path, "Int64");
    first_projection_rx
        .recv_timeout(rendezvous_timeout)
        .expect("the first signature worker must reach projection");
    let second = spawn_signature(second_path, "Float64");
    assert_eq!(
        published_rx
            .recv_timeout(rendezvous_timeout)
            .expect("the second signature must publish while the first projection is blocked"),
        2,
        "the second commit must publish while the first projection is blocked",
    );
    release_first_tx
        .send(())
        .expect("the first projection hook must remain available");
    assert_eq!(
        published_rx
            .recv_timeout(rendezvous_timeout)
            .expect("the released first signature must publish"),
        1,
    );
    let second_result = second
        .join()
        .expect("the second signature worker must not panic")
        .expect("the second signature mutation must succeed");
    let first_result = first
        .join()
        .expect("the first signature worker must not panic")
        .expect("the first signature mutation must succeed");
    assert_eq!(second_result.publication_revision, 2);
    assert_eq!(first_result.publication_revision, 1);
}

#[test]
fn resource_publication_revision_restarts_for_a_replacement_project() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "resource-publication-revision-old",
        ProjectData::new(),
    ));
    let old_project_instance_id = state.capture_project_session().unwrap().instance_id;
    let path = GraphResourcePath::new("functions/Revisioned.yssbi-function").unwrap();
    let mutate =
        |state: &ProjectState, project_instance_id: &ProjectInstanceId, return_type: &str| {
            state
                .insert_graph(
                    path.clone(),
                    GraphResourceDocument::new("Revisioned", GraphDocumentKind::Function),
                )
                .unwrap();
            state
                .update_function_signature_observed(
                    project_instance_id,
                    &path,
                    "en-US",
                    MutationRequest::new(
                        ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                            path.as_str().into(),
                        )),
                        GraphRevision::INITIAL,
                        OperationId::new(),
                        crate::node_system::document::FunctionDocumentPatch::new(
                            Default::default(),
                            crate::node_system::document::FunctionSignature {
                                parameters: Vec::new(),
                                return_type: Some(return_type.into()),
                            },
                        ),
                    ),
                    |_| {},
                )
                .unwrap()
        };

    let previous = mutate(&state, &old_project_instance_id, "Int64");
    assert_eq!(previous.publication_revision, 1);
    let replacement_project = crate::project::fixtures::TempProject::activate(
        "resource-publication-revision-replacement",
        ProjectData::new(),
    );
    let replacement_root = replacement_project
        .state()
        .capture_project_session()
        .unwrap()
        .root;
    state.activate_project_fixture(
        replacement_root.as_path().to_string_lossy().into_owned(),
        ProjectData::new(),
    );
    let replacement_project_instance_id = state.capture_project_session().unwrap().instance_id;
    let replacement = mutate(&state, &replacement_project_instance_id, "Float64");
    assert_eq!(replacement.publication_revision, 1);
    assert_eq!(
        previous.project_instance_id,
        old_project_instance_id.as_str()
    );
    assert_eq!(
        replacement.project_instance_id,
        replacement_project_instance_id.as_str()
    );
    assert_ne!(
        previous.project_instance_id,
        replacement.project_instance_id
    );
    drop(replacement_project);
}

#[test]
fn delayed_old_project_result_keeps_its_original_instance_identity() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "delayed-old-project-result",
        ProjectData::new(),
    ));
    let old_project_instance_id = state.capture_project_session().unwrap().instance_id;
    let path = GraphResourcePath::new("functions/Delayed.yssbi-function").unwrap();
    state
        .insert_graph(
            path.clone(),
            GraphResourceDocument::new("Delayed", GraphDocumentKind::Function),
        )
        .unwrap();
    let rendezvous_timeout = std::time::Duration::from_secs(2);
    let (projection_started_tx, projection_started_rx) = std::sync::mpsc::channel();
    let (release_projection_tx, release_projection_rx) = std::sync::mpsc::channel();
    let release_projection_rx = std::sync::Mutex::new(release_projection_rx);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = std::sync::Arc::clone(&calls);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            projection_started_tx
                .send(())
                .map_err(|error| format!("failed to announce the delayed projection: {error}"))?;
            release_projection_rx
                .lock()
                .unwrap()
                .recv_timeout(rendezvous_timeout)
                .map_err(|error| {
                    format!("timed out waiting to release the delayed projection: {error}")
                })?;
        }
        Ok(())
    }));

    let (old_result_completed_tx, old_result_completed_rx) = std::sync::mpsc::channel();
    let old_state = state.clone();
    let old_path = path.clone();
    let old_worker_project_instance_id = old_project_instance_id.clone();
    let old = std::thread::spawn(move || {
        let result = old_state.update_function_signature_observed(
            &old_worker_project_instance_id,
            &old_path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    old_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    crate::node_system::document::FunctionSignature {
                        parameters: Vec::new(),
                        return_type: Some("Int64".into()),
                    },
                ),
            ),
            |_| {},
        );
        old_result_completed_tx
            .send(())
            .expect("the delayed-result completion receiver must remain available");
        result
    });
    projection_started_rx
        .recv_timeout(rendezvous_timeout)
        .expect("the old-project worker must reach the delayed projection hook");

    let replacement_project = crate::project::fixtures::TempProject::activate(
        "delayed-result-replacement",
        ProjectData::new(),
    );
    let replacement_root = replacement_project
        .state()
        .capture_project_session()
        .unwrap()
        .root;
    state.activate_project_fixture(
        replacement_root.as_path().to_string_lossy().into_owned(),
        ProjectData::new(),
    );
    let replacement_project_instance_id = state.capture_project_session().unwrap().instance_id;
    state
        .insert_graph(
            path.clone(),
            GraphResourceDocument::new("Delayed", GraphDocumentKind::Function),
        )
        .unwrap();
    let replacement = state
        .update_function_signature_observed(
            &replacement_project_instance_id,
            &path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    crate::node_system::document::FunctionSignature {
                        parameters: Vec::new(),
                        return_type: Some("Float64".into()),
                    },
                ),
            ),
            |_| {},
        )
        .unwrap();
    release_projection_tx
        .send(())
        .expect("the delayed projection hook must remain available");
    old_result_completed_rx
        .recv_timeout(rendezvous_timeout)
        .expect("the old-project mutation must complete after its projection is released");
    let delayed = old
        .join()
        .expect("the old-project signature worker must not panic")
        .expect("the old-project signature mutation must succeed");

    assert_eq!(delayed.publication_revision, 1);
    assert_eq!(replacement.publication_revision, 1);
    assert_eq!(
        delayed.project_instance_id,
        old_project_instance_id.as_str()
    );
    assert_eq!(
        replacement.project_instance_id,
        replacement_project_instance_id.as_str()
    );
    assert_ne!(delayed.project_instance_id, replacement.project_instance_id);
    drop(replacement_project);
}

#[test]
fn project_mutation_rejects_stale_revision_and_records_undo_history() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    let request = MutationRequest::new(
        ResourceKey::Graph(document_path()),
        GraphRevision::INITIAL,
        OperationId::new(),
        patch,
    );

    let event = state
        .apply_graph_patch(&graph_path(), request.clone())
        .unwrap();
    assert_eq!(event.from_revision, GraphRevision::INITIAL);
    assert_eq!(event.to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.apply_graph_patch(&graph_path(), request),
        Err(MutationConflict::StaleRevision { .. })
    ));

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let graph = state
        .get_data()
        .unwrap()
        .graphs
        .remove(&graph_path())
        .unwrap();
    assert!(graph.document.nodes.is_empty());
    assert_eq!(graph.document.revision, GraphRevision::new(2));
}

#[test]
fn project_projection_hydrates_localized_editor_dto() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let projection = state.graph_projection(&graph_path(), "zh-CN").unwrap();
    assert_eq!(projection.graph_path.as_ref(), graph_path().as_str());
    assert_eq!(projection.source_revision, 1);
    assert_eq!(projection.nodes.len(), 1);
    assert_eq!(
        projection.nodes[0].node_id.as_ref(),
        inserted.id.to_string()
    );
    assert!(!projection.nodes[0].display.title.is_empty());
}

#[test]
fn project_execution_publishes_persisted_function_plans() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-functions-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
    let event = state
        .create_graph_resource_fixture("Main", GraphDocumentKind::Event)
        .unwrap();
    load_graph(&state, &event).unwrap();
    let function = state
        .create_graph_resource_fixture("Helper", GraphDocumentKind::Function)
        .unwrap();
    let begin = state.get_data().unwrap().graphs[&event]
        .document
        .nodes
        .values()
        .find(|node| node.node_type.as_str() == "yssbi.project.event.begin")
        .unwrap()
        .id;
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function.as_str()),
    );
    let connection_id = crate::node_system::document::ConnectionId::new();
    let connection = crate::node_system::document::DocumentConnection {
        id: connection_id,
        output: crate::node_system::document::PortAddress::declared(
            begin,
            crate::node_system::protocol::PortKey::new("then").unwrap(),
        ),
        input: crate::node_system::document::PortAddress::declared(
            call.id,
            crate::node_system::protocol::PortKey::new("enter").unwrap(),
        ),
        order: None,
    };
    state
        .apply_graph_patch(
            &event,
            MutationRequest::new(
                crate::node_system::document::ResourceKey::Graph(
                    crate::node_system::document::GraphResourcePath(event.as_str().into()),
                ),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::InsertNode { node: call },
                    GraphDocumentOperation::InsertConnection { connection },
                ]),
            ),
        )
        .unwrap();

    state
        .execute_graph_for_current_project_for_test(
            &event,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_execution_uses_replaced_persisted_function_body_and_current_generation() {
    use crate::node_system::analysis::ResourceKey as AnalysisResourceKey;
    use crate::node_system::document::{
        ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
        FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, OrderKey,
        PortAddress, PortInstanceId,
    };
    use crate::node_system::protocol::{ParameterKey, PortKey};

    let state = ProjectState::new();
    let function_path = GraphResourcePath::new("functions/Current.yssbi-function").unwrap();
    let event_path = GraphResourcePath::new("events/CurrentCaller.yssbi-event").unwrap();
    let parameter_id = FunctionParameterId("amount".into());
    let return_id = FunctionParameterId("return".into());
    let mut input_variable = test_variable("Input");
    input_variable.data_value = crate::graph::value::DataValue::Int64(41);
    let mut first_offset = test_variable("First Offset");
    first_offset.data_value = crate::graph::value::DataValue::Int64(1);
    let mut second_offset = test_variable("Second Offset");
    second_offset.data_value = crate::graph::value::DataValue::Int64(2);
    let mut output_variable = test_variable("Output");
    output_variable.data_value = crate::graph::value::DataValue::Int64(0);
    let mut project = ProjectData::new();
    for variable in [
        input_variable.clone(),
        first_offset.clone(),
        second_offset.clone(),
        output_variable.clone(),
    ] {
        project.variables.insert(variable.id, variable);
    }
    let root = std::env::temp_dir().join(format!(
        "yssbi-structured-control-round2-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project);

    let port = |node_id, template: &str, instance: u128| {
        PortAddress::instance(
            node_id,
            PortKey::new(template).unwrap(),
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(instance)),
        )
    };
    let binding = |parameter: &FunctionParameterId, order: &str| DynamicPortBinding::Resolved {
        origin: DynamicMemberLocator::FunctionParameter {
            function: crate::node_system::document::GraphResourcePath(
                function_path.as_str().into(),
            ),
            parameter: parameter.clone(),
        },
        order: OrderKey(order.into()),
    };
    let connection = |output: PortAddress, input: PortAddress| DocumentConnection {
        id: ConnectionId::new(),
        output,
        input,
        order: None,
    };
    let declared = |node_id, key: &str| PortAddress::declared(node_id, PortKey::new(key).unwrap());

    let mut function = GraphResourceDocument::new("Current", GraphDocumentKind::Function);
    function.function = Some(FunctionDocument::new(FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id.clone(),
            name: "Amount".into(),
            type_name: "int64".into(),
        }],
        return_type: Some("int64".into()),
    }));
    let mut entry = node("yssbi.project.function.entry");
    entry.parameters.insert(
        ParameterKey::new("function").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let mut return_node = node("yssbi.project.function.return");
    return_node.parameters.insert(
        ParameterKey::new("function").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let body = node("yssbi.numeric.add.int64");
    let mut first_offset_source = node("yssbi.project.variable.get");
    first_offset_source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", first_offset.id)),
    );
    let mut second_offset_source = node("yssbi.project.variable.get");
    second_offset_source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", second_offset.id)),
    );
    let entry_parameter = port(entry.id, "parameters", 1);
    let return_result = port(return_node.id, "results", 2);
    function
        .document
        .port_bindings
        .insert(entry_parameter.clone(), binding(&parameter_id, "a"));
    function
        .document
        .port_bindings
        .insert(return_result.clone(), binding(&return_id, "b"));
    let body_offset_connection = connection(
        declared(first_offset_source.id, "value"),
        declared(body.id, "right"),
    );
    function.document.connections = [
        connection(
            declared(entry.id, "then"),
            declared(return_node.id, "enter"),
        ),
        connection(entry_parameter.clone(), declared(body.id, "left")),
        body_offset_connection.clone(),
        connection(declared(body.id, "result"), return_result.clone()),
    ]
    .into_iter()
    .map(|connection| (connection.id, connection))
    .collect();
    function.document.nodes = [
        entry.clone(),
        body.clone(),
        first_offset_source.clone(),
        second_offset_source.clone(),
        return_node.clone(),
    ]
    .into_iter()
    .map(|node| (node.id, node))
    .collect();
    state.insert_graph(function_path.clone(), function).unwrap();

    let mut event = GraphResourceDocument::new("Current Caller", GraphDocumentKind::Event);
    let begin = node("yssbi.project.event.begin");
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    let mut source = node("yssbi.project.variable.get");
    source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", input_variable.id)),
    );
    let mut output = node("yssbi.project.variable.set");
    output.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", output_variable.id)),
    );
    let call_argument = port(call.id, "arguments", 3);
    let call_result = port(call.id, "results", 4);
    event
        .document
        .port_bindings
        .insert(call_argument.clone(), binding(&parameter_id, "a"));
    event
        .document
        .port_bindings
        .insert(call_result.clone(), binding(&return_id, "b"));
    let event_connections = [
        connection(declared(begin.id, "then"), declared(call.id, "enter")),
        connection(declared(call.id, "then"), declared(output.id, "enter")),
        connection(declared(source.id, "value"), call_argument.clone()),
        connection(call_result, declared(output.id, "value")),
    ];
    event.document.connections = event_connections
        .into_iter()
        .map(|connection| (connection.id, connection))
        .collect();
    event.document.nodes = [begin, source, call, output]
        .into_iter()
        .map(|node| (node.id, node))
        .collect();
    state.insert_graph(event_path.clone(), event).unwrap();
    crate::project::fixtures::write_state_graph(&state, &function_path).unwrap();
    crate::project::fixtures::write_state_graph(&state, &event_path).unwrap();

    let data = state.get_data().unwrap();
    let resources =
        super::project_state::compile_resources_from_data(&data, Default::default()).unwrap();
    let registry = crate::node_system::catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let compiler = crate::node_system::compiler::GraphCompiler::with_interface_resolvers(
        registry.as_ref(),
        &resources,
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    );
    let function_graph = &data.graphs[&function_path].document;
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                crate::node_system::document::GraphResourcePath(function_path.as_str().into()),
                function_graph,
            ),
            &crate::node_system::compiler::CompileCancellationToken::new(),
        )
        .unwrap();
    let diagnostic_codes = products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(
        products.plan.is_some(),
        "persisted function diagnostics: {diagnostic_codes:?}"
    );
    let event_graph = &data.graphs[&event_path].document;
    let event_products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                crate::node_system::document::GraphResourcePath(event_path.as_str().into()),
                event_graph,
            ),
            &crate::node_system::compiler::CompileCancellationToken::new(),
        )
        .unwrap();
    let event_diagnostics = event_products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect::<Vec<_>>();
    assert!(
        event_products.plan.is_some(),
        "persisted event diagnostics: {event_diagnostics:#?}"
    );
    drop(data);

    let first = state
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let first_version = first.provenance.basis.resource_versions
        [&AnalysisResourceKey::new(function_path.as_str())]
        .clone();
    assert_eq!(
        state.get_data().unwrap().variables[&output_variable.id].data_value,
        crate::graph::value::DataValue::Int64(42)
    );
    state
        .apply_graph_patch(
            &function_path,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::RemoveConnection {
                        connection: body_offset_connection,
                    },
                    GraphDocumentOperation::InsertConnection {
                        connection: connection(
                            declared(second_offset_source.id, "value"),
                            declared(body.id, "right"),
                        ),
                    },
                ]),
            ),
        )
        .unwrap();
    let second = state
        .execute_graph_for_current_project_for_test(
            &event_path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let second_version = &second.provenance.basis.resource_versions
        [&AnalysisResourceKey::new(function_path.as_str())];
    assert_ne!(&first_version, second_version);
    assert_ne!(first.provenance.compile_id, second.provenance.compile_id);
    assert_eq!(
        state.get_data().unwrap().variables[&output_variable.id].data_value,
        crate::graph::value::DataValue::Int64(43)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reversed_persisted_function_insertion_publishes_equivalent_callable_generation() {
    use crate::node_system::document::{
        ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
        FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, OrderKey,
        PortAddress, PortInstanceId,
    };
    use crate::node_system::plan::FunctionPlanHandle;
    use crate::node_system::protocol::{ParameterKey, PortKey};
    use crate::node_system::runtime::FunctionPlanProvider;

    let path_a = GraphResourcePath::new("functions/A.yssbi-function").unwrap();
    let path_b = GraphResourcePath::new("functions/B.yssbi-function").unwrap();
    let event_path = GraphResourcePath::new("events/Chain.yssbi-event").unwrap();
    let parameter_id = FunctionParameterId("amount".into());
    let return_id = FunctionParameterId("return".into());
    let port = |node_id, template: &str, instance: u128| {
        PortAddress::instance(
            node_id,
            PortKey::new(template).unwrap(),
            PortInstanceId::from_uuid(uuid::Uuid::from_u128(instance)),
        )
    };
    let declared = |node_id, key: &str| PortAddress::declared(node_id, PortKey::new(key).unwrap());
    let connection = |output: PortAddress, input: PortAddress| DocumentConnection {
        id: ConnectionId::new(),
        output,
        input,
        order: None,
    };
    let binding = |path: &GraphResourcePath, parameter: &FunctionParameterId, order: &str| {
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: crate::node_system::document::GraphResourcePath(path.as_str().into()),
                parameter: parameter.clone(),
            },
            order: OrderKey(order.into()),
        }
    };
    let signature = || FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id.clone(),
            name: "Amount".into(),
            type_name: "int64".into(),
        }],
        return_type: Some("int64".into()),
    };
    let make_function = |path: &GraphResourcePath,
                         target: Option<&GraphResourcePath>,
                         instance_base: u128| {
        let mut resource = GraphResourceDocument::new(path.as_str(), GraphDocumentKind::Function);
        resource.function = Some(FunctionDocument::new(signature()));
        let mut entry = node("yssbi.project.function.entry");
        entry.parameters.insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.as_str()),
        );
        let mut return_node = node("yssbi.project.function.return");
        return_node.parameters.insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.as_str()),
        );
        let entry_parameter = port(entry.id, "parameters", instance_base);
        let return_result = port(return_node.id, "results", instance_base + 1);
        resource
            .document
            .port_bindings
            .insert(entry_parameter.clone(), binding(path, &parameter_id, "a"));
        resource
            .document
            .port_bindings
            .insert(return_result.clone(), binding(path, &return_id, "b"));
        let mut nodes = vec![entry.clone(), return_node.clone()];
        let connections = if let Some(target) = target {
            let mut call = node("yssbi.project.function.call");
            call.parameters.insert(
                ParameterKey::new("target").unwrap(),
                serde_json::json!(target.as_str()),
            );
            let call_argument = port(call.id, "arguments", instance_base + 2);
            let call_result = port(call.id, "results", instance_base + 3);
            resource
                .document
                .port_bindings
                .insert(call_argument.clone(), binding(target, &parameter_id, "a"));
            resource
                .document
                .port_bindings
                .insert(call_result.clone(), binding(target, &return_id, "b"));
            let connections = vec![
                connection(declared(entry.id, "then"), declared(call.id, "enter")),
                connection(declared(call.id, "then"), declared(return_node.id, "enter")),
                connection(entry_parameter, call_argument),
                connection(call_result, return_result),
            ];
            nodes.push(call);
            connections
        } else {
            vec![
                connection(
                    declared(entry.id, "then"),
                    declared(return_node.id, "enter"),
                ),
                connection(entry_parameter, return_result),
            ]
        };
        resource.document.nodes = nodes.into_iter().map(|node| (node.id, node)).collect();
        resource.document.connections = connections
            .into_iter()
            .map(|connection| (connection.id, connection))
            .collect();
        resource
    };
    let function_a = make_function(&path_a, Some(&path_b), 100);
    let function_b = make_function(&path_b, None, 200);
    let input_variable = {
        let mut variable = test_variable("Input");
        variable.data_value = crate::graph::value::DataValue::Int64(7);
        variable
    };
    let output_variable = {
        let mut variable = test_variable("Output");
        variable.data_value = crate::graph::value::DataValue::Int64(0);
        variable
    };
    let mut event = GraphResourceDocument::new("Chain", GraphDocumentKind::Event);
    let begin = node("yssbi.project.event.begin");
    let mut source = node("yssbi.project.variable.get");
    source.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", input_variable.id)),
    );
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        ParameterKey::new("target").unwrap(),
        serde_json::json!(path_a.as_str()),
    );
    let mut output = node("yssbi.project.variable.set");
    output.parameters.insert(
        ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", output_variable.id)),
    );
    let argument = port(call.id, "arguments", 300);
    let result = port(call.id, "results", 301);
    event
        .document
        .port_bindings
        .insert(argument.clone(), binding(&path_a, &parameter_id, "a"));
    event
        .document
        .port_bindings
        .insert(result.clone(), binding(&path_a, &return_id, "b"));
    event.document.nodes = [begin.clone(), source.clone(), call.clone(), output.clone()]
        .into_iter()
        .map(|node| (node.id, node))
        .collect();
    event.document.connections = [
        connection(declared(begin.id, "then"), declared(call.id, "enter")),
        connection(declared(call.id, "then"), declared(output.id, "enter")),
        connection(declared(source.id, "value"), argument),
        connection(result, declared(output.id, "value")),
    ]
    .into_iter()
    .map(|connection| (connection.id, connection))
    .collect();

    let run = |reverse: bool| {
        let root = std::env::temp_dir().join(format!(
            "yssbi-structured-control-order-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut project = ProjectData::new();
        project
            .variables
            .insert(input_variable.id, input_variable.clone());
        project
            .variables
            .insert(output_variable.id, output_variable.clone());
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let entries = if reverse {
            vec![
                (path_b.clone(), function_b.clone()),
                (path_a.clone(), function_a.clone()),
            ]
        } else {
            vec![
                (path_a.clone(), function_a.clone()),
                (path_b.clone(), function_b.clone()),
            ]
        };
        for (path, function) in entries {
            state.insert_graph(path.clone(), function).unwrap();
            crate::project::fixtures::write_state_graph(&state, &path).unwrap();
        }
        state
            .insert_graph(event_path.clone(), event.clone())
            .unwrap();
        crate::project::fixtures::write_state_graph(&state, &event_path).unwrap();

        let data = state.get_data().unwrap();
        let resources =
            super::project_state::compile_resources_from_data(&data, Default::default()).unwrap();
        drop(data);
        let (registry, store, session) = {
            let store = state.project_store.read().unwrap();
            (
                store.node_registry.clone(),
                store.function_plans.clone(),
                store.project_session_id.clone(),
            )
        };
        let mut parameters = crate::node_system::runtime::CompiledParameterStore::new();
        let generation = super::project_state::publish_function_plans(
            registry.as_ref(),
            store.as_ref(),
            &resources,
            session,
            &crate::node_system::analysis::NOOP_TRACE_SINK,
            &crate::node_system::compiler::CompileCancellationToken::new(),
            &mut parameters,
        )
        .unwrap();
        let published = [&path_a, &path_b]
            .into_iter()
            .map(|path| {
                let function = generation
                    .get_function(&FunctionPlanHandle::new(path.as_str()).unwrap())
                    .unwrap()
                    .unwrap();
                assert_eq!(function.plan.provenance, function.abi.provenance);
                (
                    function.plan.provenance.graph_path.clone(),
                    function.plan.provenance.basis.clone(),
                )
            })
            .collect::<Vec<_>>();

        state
            .execute_graph_for_current_project_for_test(
                &event_path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &NOOP_RUN_EVENT_SINK,
            )
            .unwrap();
        let value = state.get_data().unwrap().variables[&output_variable.id]
            .data_value
            .clone();
        std::fs::remove_dir_all(root).unwrap();
        (generation.plan_count(), published, value)
    };

    let forward = run(false);
    let reverse = run(true);
    assert_eq!(forward.0, 2);
    assert_eq!(forward.0, reverse.0);
    assert_eq!(forward.1, reverse.1);
    assert_eq!(forward.2, crate::graph::value::DataValue::Int64(7));
    assert_eq!(forward.2, reverse.2);
}

#[test]
fn production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes() {
    let project = temp_project_with_empty_graph("compiler-shell-diagnostics");
    let state = project.state();
    let first = node("yssbi.project.function.entry");
    let second = node("yssbi.project.function.entry");
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode { node: first },
        GraphDocumentOperation::InsertNode { node: second },
    ]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert!(error.contains("compiler.node.scope_mismatch"));
    assert!(error.contains("compiler.node.managed_singleton"));
}

#[test]
fn production_relational_backend_executes_project_dataframe_source() {
    use crate::node_system::runtime::RelationalBackend;
    let dataframe = polars::df!("value" => [1_i64, 2, 3]).unwrap();
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("relational-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_database(resource.clone(), std::sync::Arc::new(dataframe)),
    );
    let requirement = crate::node_system::plan::CompiledResourceRequirement {
        resource: resource.clone(),
        kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
        access: crate::node_system::plan::ResourceAccess::Shared,
        optional: false,
    };
    let resources =
        crate::node_system::runtime::RunResourceSet::acquire(&[requirement], &provider).unwrap();
    let cancellation = crate::node_system::runtime::CancellationToken::new();
    let resource_owner = crate::node_system::runtime::RunResourceOwner::new(
        crate::node_system::analysis::RunId::new(1),
        crate::node_system::runtime::RunResourceBudgets::default(),
        cancellation.clone(),
    )
    .unwrap();
    let context = crate::node_system::runtime::RelationalContext {
        run_id: crate::node_system::analysis::RunId::new(1),
        resources: &resources,
        resource_owner: &resource_owner,
        cancellation: &cancellation,
        deadline: None,
    };
    let plan = crate::node_system::plan::CompiledRelationalPlan {
        fragment_order: Box::new([]),
        operators: Box::new([
            crate::node_system::plan::RelationalOperator::Source {
                resource,
                relation: "main".into(),
            },
            crate::node_system::plan::RelationalOperator::Limit {
                input: crate::node_system::plan::RelationalOperatorIndex::new(0),
                rows: 2,
            },
        ]),
        fragment_roots: Box::new([]),
        roots: Box::new([crate::node_system::plan::RelationalOperatorIndex::new(1)]),
        pushdown_hints: Box::new([crate::node_system::plan::RelationalPushdownHint::Limit {
            source: crate::node_system::plan::RelationalOperatorIndex::new(0),
            rows: 2,
        }]),
    };

    let result = crate::node_system::runtime::ProductionRelationalBackend::default()
        .execute(&context, &plan, &[])
        .unwrap();
    let crate::node_system::runtime::RuntimeValue::Scalar(
        crate::node_system::protocol::Value::Object(columns),
    ) = &result.outputs[0]
    else {
        panic!("expected relational dataframe output")
    };
    assert_eq!(
        columns["value"],
        crate::node_system::protocol::Value::List(vec![
            crate::node_system::protocol::Value::Integer(1),
            crate::node_system::protocol::Value::Integer(2),
        ])
    );
}

#[test]
fn project_activation_publishes_declared_duckdb_runtime_and_relational_access() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-duckdb-run-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    let mut project_data = ProjectData::new();
    project_data.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: Some("Main".into()),
        },
    );
    crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
        .unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("value" => [11_i64, 22, 33]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

    let data = state.project_data.read().unwrap();
    let snapshots = crate::project::project_state::snapshot_project_resources(
        &state,
        data.variables.clone(),
        data.databases.clone(),
    )
    .unwrap();
    drop(data);
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(snapshots.runtime);
    use crate::node_system::runtime::ResourceProvider;
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("databases/main").unwrap(),
            kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let dataframe = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .load_dataframe()
        .unwrap()
        .unwrap();
    assert_eq!(
        dataframe
            .column("value")
            .unwrap()
            .i64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        vec![11, 22, 33]
    );
    assert!(
        state
            .project_store
            .read()
            .unwrap()
            .databases
            .contains_key("main")
    );

    drop(lease);
    drop(provider);
    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_resource_snapshot_supplies_plot_sink() {
    use crate::node_system::runtime::ResourceProvider;
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("plot-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_plot_sink(std::sync::Arc::new(ProductionPlotSink)),
    );
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("yssbi.runtime.plot_sink").unwrap(),
            kind: crate::node_system::plan::ResourceKind::ExternalArtifact,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let sink = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .plot_sink()
        .unwrap();
    assert_eq!(
        sink.publish(crate::node_system::runtime::PlotKind::Line, "payload")
            .unwrap()
            .as_ref(),
        "payload"
    );
}

#[test]
fn project_execution_refuses_blocking_analysis() {
    let (state, root) = active_state_with_empty_graph("blocking-analysis");
    let invalid = node("yssbi.test.missing");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: invalid }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert!(error.contains("blocking diagnostics"));
    assert!(error.contains("compiler.node.unknown"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_variable_get_executes_against_authoritative_resource() {
    let project = temp_project_with_empty_graph("project-variable-execution");
    let state = project.state();
    let variable = state
        .add_variable(
            "authoritative",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(41),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", variable.id)),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: variable_node,
                }]),
            ),
        )
        .unwrap();

    let result = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    assert!(result.run_id.get() > 0);
}

#[test]
fn demanded_variable_get_preflights_only_its_retained_resource_and_releases_lease() {
    let project = temp_project_with_empty_graph("demanded-variable-resource");
    let state = project.state();
    let first = state
        .add_variable(
            "first",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(1),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let second = state
        .add_variable(
            "second",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(2),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let mut first_get = node("yssbi.project.variable.get");
    first_get.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", first.id)),
    );
    let first_node = first_get.id;
    let mut second_get = node("yssbi.project.variable.get");
    let second_node = second_get.id;
    second_get.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", second.id)),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::InsertNode { node: first_get },
                    GraphDocumentOperation::InsertNode { node: second_get },
                ]),
            ),
        )
        .unwrap();
    let first_resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", first.id)).unwrap();
    let second_resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", second.id)).unwrap();
    let requirement = |resource| crate::node_system::plan::CompiledResourceRequirement {
        resource,
        kind: crate::node_system::plan::ResourceKind::ExternalArtifact,
        access: crate::node_system::plan::ResourceAccess::Shared,
        optional: false,
    };
    let observer = crate::node_system::runtime::ProjectResourceLeaseObserver::default()
        .with_forced_unavailable(second_resource.clone());
    state.set_project_resource_lease_observer(observer.clone());
    let invalid_demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                crate::node_system::document::NodeId::new(),
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let invalid = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &invalid_demand,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert!(invalid.starts_with("invalid_execution_demand:"));
    assert_eq!(observer.acquired(), 0);

    let demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                first_node,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let run = state
        .execute_graph_for_current_project_for_test(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
        .unwrap();

    assert_eq!(run.values.len(), 1);
    assert_eq!(
        observer.validated_requirements(),
        vec![vec![requirement(first_resource.clone())].into_boxed_slice()]
    );
    assert_eq!(observer.acquire_attempt_ids(), vec![first_resource]);
    assert_eq!(observer.acquired(), 1);
    assert_eq!(observer.dropped(), 1);
    assert_eq!(observer.active(), 0);

    let unavailable_observer = crate::node_system::runtime::ProjectResourceLeaseObserver::default()
        .with_forced_unavailable(second_resource.clone());
    state.set_project_resource_lease_observer(unavailable_observer.clone());
    let unavailable_demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                second_node,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    let events = DemandRunEvents::default();

    let unavailable = state
        .execute_graph_for_current_project_for_test(&graph_path(), &unavailable_demand, &events)
        .unwrap_err();

    assert!(unavailable.contains("unavailable"), "{unavailable}");
    assert_eq!(
        unavailable_observer.validated_requirements(),
        vec![vec![requirement(second_resource.clone())].into_boxed_slice()],
    );
    assert_eq!(
        unavailable_observer.acquire_attempt_ids(),
        vec![second_resource],
    );
    assert_eq!(unavailable_observer.acquired(), 0);
    assert_eq!(unavailable_observer.dropped(), 0);
    assert_eq!(unavailable_observer.active(), 0);
    assert!(events.0.lock().unwrap().iter().all(|event| !matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::OperationStarted { .. }
    )));
}

fn tabular_variable(
    name: &str,
    scope: crate::variable::VariableScope,
    values: &str,
) -> crate::variable::VariableInstance {
    let id = crate::variable::VariableId::new();
    let mut variable = crate::variable::VariableInstance {
        id,
        name: name.into(),
        data_type: crate::graph::value::DataType::DataFrame,
        data_value: crate::graph::value::DataValue::DataFrame(values.into()),
        tabular: None,
        description: String::new(),
        scope,
        tags: Vec::new(),
    };
    crate::tabular::normalize_variable_tabular(&mut variable).unwrap();
    variable
}

fn cached_i64_column(state: &ProjectState, variable_id: crate::variable::VariableId) -> Vec<i64> {
    let handle = crate::tabular::variable_handle(&variable_id);
    state.project_store.read().unwrap().variable_tabular[&handle]
        .dataframe
        .column("value")
        .unwrap()
        .i64()
        .unwrap()
        .into_no_null_iter()
        .collect()
}

fn commit_tabular_effect(
    state: &ProjectState,
    variable: &crate::variable::VariableInstance,
    values: &str,
) -> Result<VariableEffectCommitResult, VariableEffectCommitError> {
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    state.commit_variable_effects(
        &session_id,
        vec![crate::node_system::runtime::VariableWriteEffect {
            resource: crate::node_system::plan::ResourceId::new(format!(
                "variables/{}",
                variable.id
            ))
            .unwrap(),
            expected_revision: GraphRevision::INITIAL,
            before: variable.clone(),
            after: crate::graph::value::DataValue::DataFrame(values.into()),
        }],
    )
}

#[test]
fn global_variable_effect_undo_redo_remains_equal_to_reloaded_disk() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-global-variable-effect-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
        format!("variables/{}", variable.id).into(),
    ));
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::project::load_project_from_file(&root_text)
            .unwrap()
            .variables[&variable.id]
            .data_value
    );

    state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let canonical = state.get_variable(&variable.id).unwrap().unwrap();
    let reloaded = crate::project::load_project_from_file(&root_text).unwrap();
    assert_eq!(
        serde_json::to_value(&canonical).unwrap(),
        serde_json::to_value(&reloaded.variables[&variable.id]).unwrap()
    );
    assert_eq!(
        canonical.data_value,
        crate::graph::value::DataValue::Int64(2)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_variable_effect_undo_redo_remains_equal_to_reloaded_disk() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-local-variable-effect-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let graph_path = GraphResourcePath::new("events/Local.yssbi-event").unwrap();
    let mut variable = test_variable("Local Rate");
    variable.scope = crate::variable::VariableScope::Event {
        event_path: graph_path.as_str().into(),
    };
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Local", GraphDocumentKind::Event),
    );
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
        format!("variables/{}", variable.id).into(),
    ));
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                GraphRevision::new(2),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();

    let canonical = state.get_variable(&variable.id).unwrap().unwrap();
    let reloaded: crate::project::project_io::GraphDocument =
        serde_json::from_slice(&std::fs::read(root.join(graph_path.as_str())).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&canonical).unwrap(),
        serde_json::to_value(&reloaded.local_variables[&variable.id]).unwrap()
    );
    assert_eq!(
        canonical.data_value,
        crate::graph::value::DataValue::Int64(2)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn durable_variable_history_conflict_rolls_disk_back_without_authority_transfer() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-history-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let conflict_state = state.clone();
    state.set_mutation_publication_test_hook(std::sync::Arc::new(move || {
        conflict_state.append_history_head_for_test();
    }));

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{}", variable.id).into(),
                )),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::History(_)));
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(2)
    );
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );
    assert!(state.history_status().can_undo);
    assert!(!state.history_status().can_redo);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tabular_variable_effect_success_updates_global_and_local_authority_and_cache() {
    for (label, scope, graph_path) in [
        ("global", crate::variable::VariableScope::Global, None),
        (
            "local",
            crate::variable::VariableScope::Event {
                event_path: "events/Tabular.yssbi-event".into(),
            },
            Some(GraphResourcePath::new("events/Tabular.yssbi-event").unwrap()),
        ),
    ] {
        let root = std::env::temp_dir().join(format!(
            "yssbi-{label}-tabular-variable-effect-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root_text = root.to_string_lossy().into_owned();
        let variable = tabular_variable("Table", scope, r#"{"value":[1,2]}"#);
        let mut project = ProjectData::new();
        if let Some(path) = &graph_path {
            project.graphs.insert(
                path.clone(),
                GraphResourceDocument::new("Tabular", GraphDocumentKind::Event),
            );
        }
        project.variables.insert(variable.id, variable.clone());
        crate::project::fixtures::write_project(&project, &root_text).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root_text, project);
        assert_eq!(cached_i64_column(&state, variable.id), vec![1, 2]);

        commit_tabular_effect(&state, &variable, r#"{"value":[7,8,9]}"#).unwrap();

        let canonical = state.get_variable(&variable.id).unwrap().unwrap();
        assert_eq!(
            canonical.data_value,
            crate::graph::value::DataValue::DataFrame(crate::tabular::variable_handle(
                &variable.id
            ))
        );
        assert_eq!(
            canonical.tabular.unwrap().to_json().unwrap(),
            r#"{"value":[7,8,9]}"#
        );
        assert_eq!(cached_i64_column(&state, variable.id), vec![7, 8, 9]);
        assert_eq!(
            state.variable_revisions.read().unwrap()[&variable.id].revision,
            GraphRevision::new(1)
        );
        let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        ));
        state
            .undo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    GraphRevision::new(1),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(cached_i64_column(&state, variable.id), vec![1, 2]);
        state
            .redo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    resource,
                    GraphRevision::new(2),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(cached_i64_column(&state, variable.id), vec![7, 8, 9]);
        let disk_variable = if let Some(path) = &graph_path {
            let document: crate::project::project_io::GraphDocument =
                serde_json::from_slice(&std::fs::read(root.join(path.as_str())).unwrap()).unwrap();
            document.local_variables[&variable.id].clone()
        } else {
            crate::project::load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .variables[&variable.id]
                .clone()
        };
        assert_eq!(
            serde_json::to_value(state.get_variable(&variable.id).unwrap().unwrap()).unwrap(),
            serde_json::to_value(disk_variable).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_tabular_variable_effect_changes_neither_authority_disk_nor_cache() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-failed-tabular-variable-effect-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = tabular_variable(
        "Local Table",
        crate::variable::VariableScope::Event {
            event_path: "events/Tabular.yssbi-event".into(),
        },
        r#"{"value":[1,2]}"#,
    );
    let graph_path = GraphResourcePath::new("events/Tabular.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Tabular", GraphDocumentKind::Event),
    );
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(graph_path.as_str())).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let authority_before = state.get_variable(&variable.id).unwrap().unwrap();
    let cache_before = cached_i64_column(&state, variable.id);

    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
    ));
    assert!(commit_tabular_effect(&state, &variable, r#"{"value":[7,8,9]}"#).is_err());

    assert_eq!(
        serde_json::to_value(state.get_variable(&variable.id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(authority_before).unwrap()
    );
    assert_eq!(cached_i64_column(&state, variable.id), cache_before);
    assert_eq!(
        std::fs::read(root.join(graph_path.as_str())).unwrap(),
        disk_before
    );
    assert_eq!(
        state.variable_revisions.read().unwrap()[&variable.id].revision,
        GraphRevision::INITIAL
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_commit_is_revisioned_and_undoable() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-commit-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let committed = state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource,
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    assert_eq!(committed.variable_ids.as_ref(), &[variable.id]);
    let event_result = committed.resource_mutation.clone().unwrap();
    assert_eq!(event_result.publication_revision, 1);
    assert_eq!(event_result.deltas.len(), 1);
    assert_eq!(event_result.deltas[0].from_revision, GraphRevision::INITIAL);
    assert_eq!(event_result.deltas[0].to_revision, GraphRevision::new(1));
    assert_eq!(
        event_result.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
    assert!(event_result.projection_replacements.is_empty());
    assert_eq!(
        event_result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        }
    );

    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(2)
    ));

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{}", variable.id).into(),
                )),
                GraphRevision::new(1),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(1)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_persistence_failure_rolls_back_before_publication() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-transaction-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();

    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let effect = crate::node_system::runtime::VariableWriteEffect {
        resource,
        expected_revision: GraphRevision::INITIAL,
        before: variable.clone(),
        after: crate::graph::value::DataValue::Int64(2),
    };
    let history_before = state.history_status();
    let active_project_instance_id = state.capture_project_session().unwrap().instance_id;

    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
    ));
    let error = state
        .commit_variable_effects(&session_id, vec![effect.clone()])
        .unwrap_err();

    assert!(matches!(
        error,
        VariableEffectCommitError::Persistence { .. }
    ));
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(1)
    );
    assert_eq!(state.history_status(), history_before);
    let failed_index = state
        .read_project_index(&active_project_instance_id)
        .unwrap();
    assert_eq!(failed_index.publication_revision, 0);
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );

    let committed = state
        .commit_variable_effects(&session_id, vec![effect])
        .unwrap();
    let resource_mutation = committed.resource_mutation.as_ref().unwrap();
    assert_eq!(resource_mutation.publication_revision, 1);
    assert_eq!(
        resource_mutation.deltas[0].from_revision,
        GraphRevision::INITIAL
    );
    assert_eq!(
        resource_mutation.deltas[0].to_revision,
        GraphRevision::new(1)
    );
    let success_index = state
        .read_project_index(&active_project_instance_id)
        .unwrap();
    assert_eq!(success_index.publication_revision, 1);
    assert_eq!(
        crate::project::load_project_from_file(&root_text)
            .unwrap()
            .variables[&variable.id]
            .data_value,
        crate::graph::value::DataValue::Int64(2)
    );

    let function_path = GraphResourcePath::new("functions/Next.yssbi-function").unwrap();
    state
        .insert_graph(
            function_path.clone(),
            GraphResourceDocument::new("Next", GraphDocumentKind::Function),
        )
        .unwrap();
    let next = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    function_path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    crate::node_system::document::FunctionSignature {
                        parameters: Vec::new(),
                        return_type: Some("Int64".into()),
                    },
                ),
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(next.publication_revision, 2);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deadline_during_variable_effect_authority_gate_rolls_back_disk_and_state() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-deadline-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Deadline Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let history_before = state.history_status();
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    state.set_mutation_publication_test_hook(std::sync::Arc::new(|| {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }));
    let cancellation = crate::node_system::runtime::CancellationToken::new();

    let error = state
        .commit_variable_effects_for_run(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
            &cancellation,
            Some(crate::node_system::runtime::RunDeadline::after(
                std::time::Duration::from_millis(5),
            )),
        )
        .unwrap_err();

    assert_eq!(
        error,
        crate::node_system::runtime::RunError::DeadlineExceeded {
            phase: crate::node_system::runtime::RunPhase::ResultPublication,
        }
    );
    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(1)
    ));
    assert_eq!(state.history_status(), history_before);
    assert_eq!(
        state
            .read_project_index(&project_instance_id)
            .unwrap()
            .publication_revision,
        0
    );
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_authority_assignment_panic_restores_every_authoritative_projection() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-authority-panic-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Panic Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let cache_before = state
        .project_store
        .read()
        .unwrap()
        .variable_tabular
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let history_before = state.history_status();
    let history_lengths_before = state.history_lengths_for_test();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();

    let assignment_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let assignment_count_for_hook = std::sync::Arc::clone(&assignment_count);
    state.set_variable_authority_assignment_panic_for_test(std::sync::Arc::new(move || {
        if assignment_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
            panic!("injected variable authority assignment panic")
        }
    }));

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = state.commit_variable_effects_for_run(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
            &crate::node_system::runtime::CancellationToken::new(),
            None,
        );
    }));

    assert!(panic.is_err());
    assert_eq!(
        assignment_count.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(
        state
            .project_store
            .read()
            .unwrap()
            .variable_tabular
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        cache_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.history_lengths_for_test(), history_lengths_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_variable_effect_commit_returns_structured_revision_conflict() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let stale_effect = crate::node_system::runtime::VariableWriteEffect {
        resource,
        expected_revision: GraphRevision::INITIAL,
        before: variable.clone(),
        after: crate::graph::value::DataValue::Int64(2),
    };
    let winning_effect = crate::node_system::runtime::VariableWriteEffect {
        after: crate::graph::value::DataValue::Int64(3),
        ..stale_effect.clone()
    };
    state
        .commit_variable_effects(&session_id, vec![winning_effect])
        .unwrap();

    let error = state
        .commit_variable_effects(&session_id, vec![stale_effect])
        .unwrap_err();
    assert!(matches!(
        error,
        VariableEffectCommitError::Conflict {
            resource: ResourceKey::Variable(_),
            ..
        }
    ));
    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::graph::value::DataValue::Int64(3)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

fn active_state_with_empty_graph(label: &str) -> (ProjectState, std::path::PathBuf) {
    let (state, root) = state_with_project_path(label);
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
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: constant,
                }]),
            ),
        )
        .unwrap();
    (state, root)
}

#[test]
fn unrelated_resource_mutation_preserves_published_compilation() {
    let used = test_variable("Used");
    let unrelated = test_variable("Unrelated");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.variables.insert(unrelated.id, unrelated.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "exact-resource-publication-freshness",
        data,
    );
    let state = project.state();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    let original = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;

    state
        .update_variable(
            &unrelated.id,
            Some("Unrelated changed".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let after_unrelated = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;
    assert_eq!(after_unrelated, original);

    state
        .update_variable(
            &used.id,
            Some("Used changed".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let after_dependency = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;
    assert_ne!(after_dependency, original);

    for dependency in [
        "functions/Used.yssbi-function",
        "variables/00000000-0000-0000-0000-000000000701",
        "databases/used",
    ] {
        let coordinator = crate::node_system::compiler::CompileCoordinator::<&str, &str>::new();
        let request_basis = crate::node_system::compiler::compilation_basis(
            GraphRevision::INITIAL,
            crate::node_system::registry::RegistryFingerprint::from_bytes([7; 32]),
            Default::default(),
        );
        let task = match coordinator.request(document_path(), request_basis.clone()) {
            crate::node_system::compiler::ScheduleOutcome::Start(task) => task,
            crate::node_system::compiler::ScheduleOutcome::Coalesced { .. } => unreachable!(),
        };
        let dependency_key = crate::node_system::analysis::ResourceKey::new(dependency);
        let unrelated_key = crate::node_system::analysis::ResourceKey::new("variables/unrelated");
        let version_one = crate::node_system::analysis::ResourceVersion::new("1");
        let mut current_versions = std::collections::BTreeMap::from([
            (dependency_key.clone(), version_one.clone()),
            (
                unrelated_key.clone(),
                crate::node_system::analysis::ResourceVersion::new("1"),
            ),
        ]);
        let final_basis = crate::node_system::compiler::compilation_basis(
            GraphRevision::INITIAL,
            request_basis.registry_fingerprint.clone(),
            std::collections::BTreeMap::from([(dependency_key.clone(), version_one)]),
        );
        coordinator.publish_tracked(
            &task,
            &request_basis,
            &current_versions,
            &final_basis,
            crate::node_system::compiler::CompileProducts {
                analysis: "analysis",
                has_blocking_diagnostics: false,
                plan: Some("plan"),
            },
        );
        coordinator.finish(&document_path(), task.compile_id);

        current_versions.insert(
            unrelated_key,
            crate::node_system::analysis::ResourceVersion::new("2"),
        );
        let reused = coordinator
            .get_current_tracked(&document_path(), &request_basis, &current_versions)
            .unwrap();
        assert_eq!(reused.0.compile_id, task.compile_id);

        current_versions.insert(
            dependency_key,
            crate::node_system::analysis::ResourceVersion::new("2"),
        );
        assert!(
            coordinator
                .get_current_tracked(&document_path(), &request_basis, &current_versions)
                .is_none()
        );
        let replacement = match coordinator.request(document_path(), request_basis.clone()) {
            crate::node_system::compiler::ScheduleOutcome::Start(task) => task,
            crate::node_system::compiler::ScheduleOutcome::Coalesced { .. } => unreachable!(),
        };
        assert_ne!(replacement.compile_id, task.compile_id);
    }
}

#[test]
fn fast_compile_candidate_reuse_skips_full_resource_snapshot_construction() {
    let (state, root) = active_state_with_valid_constant_graph("fast-reuse-resource-snapshot");
    for index in 0..32 {
        let path =
            GraphResourcePath::new(format!("functions/Unrelated{index}.yssbi-function")).unwrap();
        state
            .insert_graph(
                path,
                GraphResourceDocument::new(
                    format!("Unrelated {index}"),
                    GraphDocumentKind::Function,
                ),
            )
            .unwrap();
    }
    let before = super::project_state::compile_resource_snapshot_constructions();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    let after_compile = super::project_state::compile_resource_snapshot_constructions();
    assert_eq!(
        after_compile - before,
        1,
        "a real compile builds one snapshot"
    );

    state.graph_projection(&graph_path(), "en-US").unwrap();
    assert_eq!(
        super::project_state::compile_resource_snapshot_constructions(),
        after_compile,
        "fast candidate reuse must not construct the full project snapshot"
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn projection_and_execution_reuse_one_compile_product() {
    let (state, root) = active_state_with_valid_constant_graph("projection-execution-reuse");
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_requested_and_preview_demands_reuse_one_basis_with_distinct_digests() {
    let (state, root) = active_state_with_valid_constant_graph("demand-variant-reuse");
    let first_node = state.get_data().unwrap().graphs[&graph_path()]
        .document
        .nodes
        .keys()
        .next()
        .copied()
        .unwrap();
    let mut second = node("yssbi.constant.int64");
    second.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(9),
    );
    let second_node = second.id;
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: second }]),
            ),
        )
        .unwrap();
    let output = |node_id| crate::node_system::plan::GraphOutputRef {
        graph_path: document_path(),
        port: crate::node_system::document::PortAddress::declared(
            node_id,
            crate::node_system::protocol::PortKey::new("value").unwrap(),
        ),
    };
    let demand = |output| crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([output]),
        include_default_results: false,
    };
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    let default_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let first_output = output(first_node);
    let first_events = DemandRunEvents::default();
    let first_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &demand(first_output.clone()),
            &first_events,
        )
        .unwrap();
    let second_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &demand(output(second_node)),
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let preview_events = DemandRunEvents::default();
    let preview_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::PinPreview {
                output: first_output.clone(),
                generation: 17,
            },
            &preview_events,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1
    );
    assert_eq!(
        default_run.provenance.compile_id,
        first_run.provenance.compile_id
    );
    assert_eq!(
        first_run.provenance.compile_id,
        second_run.provenance.compile_id
    );
    assert_eq!(
        first_run.provenance.compile_id,
        preview_run.provenance.compile_id
    );
    assert_eq!(first_run.values.len(), 1);
    assert_eq!(second_run.values.len(), 1);
    let first_events = first_events.0.lock().unwrap();
    assert_eq!(
        first_events
            .iter()
            .filter(|event| matches!(
                event.kind,
                crate::node_system::runtime::RunEventKind::OperationStarted { .. }
            ))
            .count(),
        1,
    );
    assert!(first_events.iter().any(|event| matches!(
        &event.kind,
        crate::node_system::runtime::RunEventKind::OutputReady { output, .. }
            if output == &first_output
    )));
    assert_ne!(
        default_run.correlation.selection_digest,
        first_run.correlation.selection_digest
    );
    assert_ne!(
        first_run.correlation.selection_digest,
        second_run.correlation.selection_digest
    );
    assert_ne!(
        first_run.correlation.selection_digest,
        preview_run.correlation.selection_digest,
    );
    let preview_events = preview_events.0.lock().unwrap();
    assert!(preview_events.iter().all(|event| !matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::ResultReady { .. }
    )));
    assert_eq!(
        preview_events
            .iter()
            .filter(|event| matches!(
                &event.kind,
                crate::node_system::runtime::RunEventKind::OutputReady {
                    output,
                    generation: Some(17),
                    ..
                } if output == &first_output
            ))
            .count(),
        1,
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn graph_basis_replacement_discards_old_demand_variants() {
    let (state, root) = active_state_with_valid_constant_graph("demand-variant-invalidation");
    let node_id = state.get_data().unwrap().graphs[&graph_path()]
        .document
        .nodes
        .keys()
        .next()
        .copied()
        .unwrap();
    let demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::node_system::document::PortAddress::declared(
                node_id,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    state
        .execute_graph_for_current_project_for_test(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
        .unwrap();
    let (old_compile_id, old_variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();
    assert_eq!(old_variants, 1);

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let (new_compile_id, new_variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();

    assert_ne!(old_compile_id, new_compile_id);
    assert_eq!(new_variants, 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_authority_generation_change_preserves_execution_authority() {
    let (state, root) = active_state_with_valid_constant_graph("variant-authority-mismatch");
    let observed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observed_for_hook = std::sync::Arc::clone(&observed);
    let authority_state = state.clone();
    state.set_execution_before_final_gate_test_hook(std::sync::Arc::new(move || {
        *observed_for_hook.lock().unwrap() =
            authority_state.published_variant_cache_state_for_test(&graph_path());
        authority_state
            .mutation_publication
            .lock()
            .unwrap()
            .advance_authority_generation();
    }));
    let events = DemandRunEvents::default();

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();
    let (compile_id, variants) = observed.lock().unwrap().unwrap();
    assert_eq!(variants, 1);
    assert_eq!(
        state.published_compile_ids_for_test(&graph_path()),
        Some((compile_id, Some(compile_id))),
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn execution_and_projection_reuse_one_compile_product() {
    let (state, root) = active_state_with_valid_constant_graph("execution-projection-reuse");
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn blocking_recompile_clears_published_execution_plan() {
    let (state, root) = active_state_with_valid_constant_graph("blocking-recompile");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (valid_compile_id, valid_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(valid_plan_id, Some(valid_compile_id));
    let coordinator = state.compile_coordinator.read().unwrap().clone();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.test.missing"),
                }]),
            ),
        )
        .unwrap();

    assert!(!coordinator.contains_slot_for_test(&document_path()));
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (blocking_compile_id, blocking_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_ne!(blocking_compile_id, valid_compile_id);
    assert_eq!(blocking_plan_id, None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn newer_graph_basis_replaces_older_published_plan() {
    let (state, root) = active_state_with_valid_constant_graph("stale-compile-plan");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (first_compile_id, first_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(first_plan_id, Some(first_compile_id));

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();

    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));
    let stale_state = state.clone();
    let stale = std::thread::spawn(move || stale_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    release_gate_tx.send(()).unwrap();
    stale.join().unwrap().unwrap();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(2),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (current_compile_id, current_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_ne!(current_compile_id, first_compile_id);
    assert_eq!(current_plan_id, Some(current_compile_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn graph_unload_invalidates_compile_slot() {
    let (state, root) = active_state_with_valid_constant_graph("unload-compile-slot");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    assert!(coordinator.contains_slot_for_test(&document_path()));

    state.unload_graph_resource(&graph_path()).unwrap();

    assert!(!coordinator.contains_slot_for_test(&document_path()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_body_mutations_stale_dependent_callers_without_eager_slot_eviction() {
    for entry in ["mutation", "patch"] {
        let (state, function_path, caller_path, _) =
            function_state_with_caller(&format!("FunctionBody{entry}"));
        state.graph_projection(&caller_path, "en-US").unwrap();
        let original_compile_id = state
            .published_compile_ids_for_test(&caller_path)
            .unwrap()
            .0;
        let coordinator = state.compile_coordinator.read().unwrap().clone();
        let caller_document_path =
            crate::node_system::document::GraphResourcePath(caller_path.as_str().into());
        assert!(coordinator.contains_slot_for_test(&caller_document_path));
        let function_document_path =
            crate::node_system::document::GraphResourcePath(function_path.as_str().into());

        let request_resource = ResourceKey::Graph(function_document_path);
        if entry == "mutation" {
            state
                .apply_graph_mutation(
                    &function_path,
                    MutationRequest::new(
                        request_resource,
                        GraphRevision::INITIAL,
                        OperationId::new(),
                        GraphMutation::CreateNode {
                            node: node("yssbi.constant.int64"),
                        },
                    ),
                )
                .unwrap();
        } else {
            state
                .apply_graph_patch(
                    &function_path,
                    MutationRequest::new(
                        request_resource,
                        GraphRevision::INITIAL,
                        OperationId::new(),
                        GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                            node: node("yssbi.constant.int64"),
                        }]),
                    ),
                )
                .unwrap();
        }

        assert!(
            coordinator.contains_slot_for_test(&caller_document_path),
            "{entry} eagerly evicted a dependent caller slot"
        );
        state.graph_projection(&caller_path, "en-US").unwrap();
        assert_ne!(
            state
                .published_compile_ids_for_test(&caller_path)
                .unwrap()
                .0,
            original_compile_id,
            "{entry} reused a caller whose exact function dependency changed",
        );
    }
}

#[test]
fn project_replacement_detaches_old_compile_generation_and_populated_variants() {
    let (state, root) = active_state_with_valid_constant_graph("replace-compile-generation");
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let (detached_compile_id, variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();
    assert_eq!(variants, 1);
    let detached = state.compile_coordinator.read().unwrap().clone();
    assert!(detached.contains_slot_for_test(&document_path()));

    state.activate_project_fixture("replacement-project".into(), ProjectData::new());

    let current = state.compile_coordinator.read().unwrap().clone();
    assert!(!std::sync::Arc::ptr_eq(&detached, &current));
    assert!(!detached.contains_slot_for_test(&document_path()));
    assert!(!current.contains_slot_for_test(&document_path()));
    assert!(
        state
            .published_variant_cache_state_for_test(&graph_path())
            .is_none()
    );
    assert!(detached_compile_id.get() > 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_graph_mutation_rejects_environment_from_older_authority_generation() {
    let state = state_with_empty_graph();
    let (capture_paused_tx, capture_paused_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    let capture_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture_count_for_hook = std::sync::Arc::clone(&capture_count);
    state.set_projection_environment_after_path_data_test_hook(std::sync::Arc::new(move || {
        if capture_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            capture_paused_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        mutation_state.apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
    });
    capture_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .insert_graph(
            GraphResourcePath::new("events/Unrelated.yssbi-event").unwrap(),
            GraphResourceDocument::new("Unrelated", GraphDocumentKind::Event),
        )
        .unwrap();
    release_capture_tx.send(()).unwrap();

    match mutation.join().unwrap() {
        Ok(_) => {
            assert_eq!(capture_count.load(std::sync::atomic::Ordering::Acquire), 2);
            assert_eq!(
                state.get_data().unwrap().graphs[&graph_path()]
                    .document
                    .revision,
                GraphRevision::new(1)
            );
        }
        Err(MutationConflict::Projection(_)) => {
            assert_eq!(
                state.get_data().unwrap().graphs[&graph_path()]
                    .document
                    .revision,
                GraphRevision::INITIAL
            );
        }
        Err(error) => panic!("unexpected mutation error: {error}"),
    }
}

#[test]
fn graph_projection_retries_when_authority_changes_during_metadata_capture() {
    let (state, root) = active_state_with_valid_constant_graph("graph-projection-capture");
    let capture_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture_count_for_hook = std::sync::Arc::clone(&capture_count);
    let (capture_paused_tx, capture_paused_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    state.set_projection_environment_after_path_data_test_hook(std::sync::Arc::new(move || {
        if capture_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            capture_paused_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    capture_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.graphs
            .get_mut(&graph_path())
            .unwrap()
            .document
            .revision = GraphRevision::new(2);
        publication.advance_authority_generation();
    }
    release_capture_tx.send(()).unwrap();

    let projection = projection.join().unwrap().unwrap();
    assert_eq!(projection.source_revision, 2);
    let captures = capture_count.load(std::sync::atomic::Ordering::Acquire);
    assert!(
        captures >= 2,
        "expected invalidated capture to be retried, observed {captures} capture(s)"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_source_keeps_captured_authority_across_unrelated_generation_aba() {
    let (state, function_path, caller_path, resource) =
        function_state_with_caller("CompileSourceAba");
    let authority_state = state.clone();
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        let mut publication = authority_state.mutation_publication.lock().unwrap();
        publication.advance_authority_generation();
        publication.advance_authority_generation();
    }));

    let result = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource,
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();

    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
    assert_eq!(result.projection_replacements.len(), 2);
}

#[test]
fn function_insert_uses_max_incoming_or_retained_successor_and_reports_overflow() {
    let state = ProjectState::new();
    state.activate_project_fixture("function-insert-revision".into(), ProjectData::new());
    let path = GraphResourcePath::new("functions/Insert.yssbi-function").unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(path.clone(), GraphRevision::new(7));

    let mut low = GraphResourceDocument::new("Insert", GraphDocumentKind::Function);
    low.document.revision = GraphRevision::new(3);
    let inserted = state.insert_graph(path.clone(), low).unwrap();
    assert_eq!(inserted.document.revision, GraphRevision::new(8));
    assert_eq!(
        inserted.function.as_ref().unwrap().revision,
        inserted.document.revision
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(8)
    );

    let mut high = GraphResourceDocument::new("Insert", GraphDocumentKind::Function);
    high.document.revision = GraphRevision::new(12);
    let inserted = state.insert_graph(path.clone(), high).unwrap();
    assert_eq!(inserted.document.revision, GraphRevision::new(12));
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(12)
    );

    let overflow = GraphResourcePath::new("functions/Overflow.yssbi-function").unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(overflow.clone(), GraphRevision::new(u64::MAX));
    let before_generation = state.authority_generation_for_test();
    let error = state
        .insert_graph(
            overflow.clone(),
            GraphResourceDocument::new("Overflow", GraphDocumentKind::Function),
        )
        .unwrap_err();
    assert_eq!(error.code(), "resource_revision_overflow");
    assert!(!state.get_data().unwrap().graphs.contains_key(&overflow));
    assert_eq!(
        state.graph_revisions.read().unwrap()[&overflow],
        GraphRevision::new(u64::MAX)
    );
    assert_eq!(state.authority_generation_for_test(), before_generation);
}

#[test]
fn function_patch_remove_and_reinsert_keep_authoritative_revisions_coherent() {
    let (state, root) = state_with_project_path("function-patch-reinsert");
    let path = GraphResourcePath::new("functions/Reinsert.yssbi-function").unwrap();
    let mut original = GraphResourceDocument::new("Reinsert", GraphDocumentKind::Function);
    original.document.revision = GraphRevision::new(4);
    state.insert_graph(path.clone(), original).unwrap();
    let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        path.as_str().into(),
    ));
    let remove_context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![key.clone()],
        expected_revisions: [(key.clone(), GraphRevision::new(4))].into_iter().collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };

    let removed = state
        .apply_resource_document_patch(
            &remove_context,
            ResourceDocumentPatch::RemoveGraph {
                path: path.clone(),
                revision: GraphRevision::new(4),
            },
        )
        .unwrap();
    assert_eq!(removed.deltas[0].to_revision, GraphRevision::new(5));
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(5)
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&path));

    let insert_context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: [key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let mut incoming = GraphResourceDocument::new("Reinsert", GraphDocumentKind::Function);
    incoming.document.revision = GraphRevision::new(1);
    let inserted = state
        .apply_resource_document_patch(
            &insert_context,
            ResourceDocumentPatch::InsertGraph {
                path: path.clone(),
                resource: incoming,
            },
        )
        .unwrap();

    let revision = GraphRevision::new(6);
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        revision
    );
    assert_eq!(state.graph_revisions.read().unwrap()[&path], revision);
    assert_eq!(inserted.deltas[0].to_revision, revision);
    assert_eq!(
        inserted.projection_replacements[0]
            .projection
            .source_revision,
        revision.get()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_move_into_tombstone_keeps_document_ledger_delta_and_projection_revision_equal() {
    let (state, root) = state_with_project_path("function-move-target-tombstone");
    let from = GraphResourcePath::new("functions/Before.yssbi-function").unwrap();
    let to = GraphResourcePath::new("functions/After.yssbi-function").unwrap();
    let mut source = GraphResourceDocument::new("Before", GraphDocumentKind::Function);
    source.document.revision = GraphRevision::new(2);
    state.insert_graph(from.clone(), source.clone()).unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(to.clone(), GraphRevision::new(9));
    let source_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        from.as_str().into(),
    ));
    let target_key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        to.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![source_key.clone()],
        expected_revisions: [(source_key, GraphRevision::new(2))].into_iter().collect(),
        expected_absent_resources: [target_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let mut moved = source.clone();
    moved.name = "After".into();
    moved.document.revision = GraphRevision::new(3);

    let result = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: source,
                moved,
                referenced_graphs_before: Default::default(),
                referenced_graphs: Default::default(),
                loaded_referenced_graphs: Default::default(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap();

    let revision = GraphRevision::new(10);
    let data = state.get_data().unwrap();
    let moved = &data.graphs[&to];
    assert_eq!(moved.document.revision, revision);
    assert_eq!(moved.function.as_ref().unwrap().revision, revision);
    assert_eq!(state.graph_revisions.read().unwrap()[&to], revision);
    assert_eq!(result.deltas[0].to_revision, revision);
    assert_eq!(
        result.projection_replacements[0].projection.source_revision,
        revision.get()
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&from],
        GraphRevision::new(3)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_load_over_retained_revision_keeps_document_ledger_and_projection_equal() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "function-load-retained-revision",
        ProjectData::new(),
    ));
    let path = GraphResourcePath::new("functions/Loaded.yssbi-function").unwrap();
    let mut persisted = GraphResourceDocument::new("Loaded", GraphDocumentKind::Function);
    persisted.document.revision = GraphRevision::new(2);
    state.insert_graph(path.clone(), persisted).unwrap();
    crate::project::fixtures::write_state_graph(&state, &path).unwrap();
    state.unload_graph_resource(&path).unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(path.clone(), GraphRevision::new(8));
    let instance = state.capture_project_session().unwrap().instance_id;

    let projection = state
        .load_graph_projection(&instance, &path, 1, "en-US")
        .unwrap();

    let revision = GraphRevision::new(9);
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        revision
    );
    assert_eq!(state.graph_revisions.read().unwrap()[&path], revision);
    assert_eq!(projection.source_revision, revision.get());

    let reloaded = state
        .load_graph_projection(&instance, &path, 2, "en-US")
        .unwrap();
    let reload_revision = GraphRevision::new(10);
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, reload_revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        reload_revision
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        reload_revision
    );
    assert_eq!(reloaded.source_revision, reload_revision.get());
}

#[test]
fn project_resource_authority_tracks_missing_tombstones_and_prevents_aba() {
    let state = ProjectState::new();
    state.activate_project_fixture("resource-authority-state".into(), ProjectData::new());
    let function_path = GraphResourcePath::new("functions/Authority.yssbi-function").unwrap();
    let function_key = crate::node_system::analysis::ResourceKey::new(function_path.as_str());
    let variable = test_variable("Authority Variable");
    let variable_id = variable.id;
    let variable_key =
        crate::node_system::analysis::ResourceKey::new(format!("variables/{variable_id}"));
    let database_key = crate::node_system::analysis::ResourceKey::new("databases/authority");
    let keys = || {
        vec![
            function_key.clone(),
            variable_key.clone(),
            database_key.clone(),
        ]
    };

    let missing = state.authoritative_resource_states_for_test(keys());
    assert!(
        missing.values().all(
            |state| state == &crate::node_system::analysis::ResourceObservedState::Absent(None)
        )
    );

    let function = GraphResourceDocument::new("Authority", GraphDocumentKind::Function);
    state
        .insert_graph(function_path.clone(), function.clone())
        .unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.variables.insert(variable_id, variable.clone());
        data.databases.insert(
            "authority".into(),
            crate::database::DatabaseDecl {
                id: "authority".into(),
                engine: crate::database::DatabaseEngine::InMemory {
                    name: "authority".into(),
                },
                schema_version: 1,
                required: false,
                name: Some("Authority".into()),
            },
        );
        state.variable_revisions.write().unwrap().insert(
            variable_id,
            super::project_state::VariableRevisionEntry::present(GraphRevision::new(1)),
        );
        state
            .database_authority_revisions
            .write()
            .unwrap()
            .insert("authority".into(), 1);
        publication.advance_authority_generation();
    }
    let present = state.authoritative_resource_states_for_test(keys());
    assert!(present.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Present(_)
    )));

    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.graphs.remove(&function_path);
        data.variables.remove(&variable_id);
        data.databases.remove("authority");
        let mut graph_revisions = state.graph_revisions.write().unwrap();
        let function_tombstone = graph_revisions[&function_path].next();
        graph_revisions.insert(function_path.clone(), function_tombstone);
        let mut variable_revisions = state.variable_revisions.write().unwrap();
        let variable_tombstone = variable_revisions[&variable_id].revision.next();
        variable_revisions.insert(
            variable_id,
            super::project_state::VariableRevisionEntry::deleted(variable_tombstone),
        );
        let mut database_revisions = state.database_authority_revisions.write().unwrap();
        *database_revisions.get_mut("authority").unwrap() += 1;
        publication.advance_authority_generation();
    }
    let tombstones = state.authoritative_resource_states_for_test(keys());
    assert!(tombstones.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Absent(Some(_))
    )));

    state.insert_graph(function_path.clone(), function).unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.variables.insert(variable_id, variable);
        data.databases.insert(
            "authority".into(),
            crate::database::DatabaseDecl {
                id: "authority".into(),
                engine: crate::database::DatabaseEngine::InMemory {
                    name: "authority".into(),
                },
                schema_version: 1,
                required: false,
                name: Some("Authority".into()),
            },
        );
        let mut variable_revisions = state.variable_revisions.write().unwrap();
        let next_variable = variable_revisions[&variable_id].revision.next();
        variable_revisions.insert(
            variable_id,
            super::project_state::VariableRevisionEntry::present(next_variable),
        );
        *state
            .database_authority_revisions
            .write()
            .unwrap()
            .get_mut("authority")
            .unwrap() += 1;
        publication.advance_authority_generation();
    }
    let recreated = state.authoritative_resource_states_for_test(keys());
    assert!(recreated.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Present(_)
    )));
    assert_ne!(
        present, recreated,
        "same-content recreation must not reuse versions"
    );
}

#[test]
fn captured_source_compiles_once_across_unrelated_mutation() {
    let unrelated = test_variable("Unrelated capture mutation");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut constant = node("yssbi.constant.int64");
    constant.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(7),
    );
    graph.document.nodes.insert(constant.id, constant);
    let mut data = ProjectData::new();
    data.variables.insert(unrelated.id, unrelated.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-captured-source-unrelated-mutation",
        data,
    );
    let state = project.state();
    let hook_state = state.clone();
    let hook_variable = unrelated.id;
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        assert_eq!(
            attempt, 0,
            "stale captured source entered a Start/cancel loop"
        );
        hook_state
            .update_variable(
                &hook_variable,
                Some("Unrelated changed after capture".into()),
                None,
                None,
                None,
                None,
            )
            .unwrap();
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1,
        "the first captured source must compile and publish exactly once"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 1);
}

#[test]
fn captured_dependency_mutation_rejects_publish_and_recompiles_current() {
    let used = test_variable("Captured dependency");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-captured-source-dependency-mutation",
        data,
    );
    let state = project.state();
    let hook_state = state.clone();
    let hook_variable = used.id;
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if attempt == 0 {
            hook_state
                .update_variable(
                    &hook_variable,
                    Some("Dependency changed after capture".into()),
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
        } else if attempt > 1 {
            panic!("stale dependency source was compiled more than once");
        }
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2,
        "the captured compile must finish, fail publication, and recompile current authority"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 2);
}

#[test]
fn stale_lifecycle_after_source_capture_returns_without_compiling() {
    let (state, root) = active_state_with_valid_constant_graph("compile-source-stale-lifecycle");
    let mut replacement_data = ProjectData::new();
    replacement_data.graphs.insert(
        graph_path(),
        GraphResourceDocument::new("Replacement", GraphDocumentKind::Event),
    );
    let replacement = crate::project::fixtures::TempProject::activate(
        "compile-source-stale-lifecycle-replacement",
        replacement_data.clone(),
    );
    let replacement_root = replacement.state().capture_project_session().unwrap().root;
    let hook_state = state.clone();
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        assert_eq!(attempt, 0, "stale lifecycle entered a compile retry loop");
        hook_state.activate_project_fixture(
            replacement_root.as_path().to_string_lossy().into_owned(),
            replacement_data.clone(),
        );
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    let error = state.graph_projection(&graph_path(), "en-US").unwrap_err();

    assert!(error.contains("stale_project_lifecycle"), "{error}");
    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        0,
        "stale lifecycle must return before real compilation"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn compile_capture_retries_when_authority_changes_during_metadata_capture() {
    let (state, root) = active_state_with_valid_constant_graph("compile-capture-generation");
    let capture_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture_count_for_hook = std::sync::Arc::clone(&capture_count);
    let (capture_paused_tx, capture_paused_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    state.set_compile_capture_after_environment_test_hook(std::sync::Arc::new(move || {
        if capture_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            capture_paused_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let execution_state = state.clone();
    let execution = std::thread::spawn(move || {
        execution_state.execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
    });
    capture_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        let graph = data.graphs.get_mut(&graph_path()).unwrap();
        graph.document.revision = GraphRevision::new(2);
        publication.advance_authority_generation();
    }
    release_capture_tx.send(()).unwrap();

    execution.join().unwrap().unwrap();
    let captures = capture_count.load(std::sync::atomic::Ordering::Acquire);
    assert!(
        captures >= 2,
        "expected invalidated capture to be retried, observed {captures} capture(s)"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn publish_gate_ignores_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("compile-publish-gate");
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_gate_tx.send(()).unwrap();

    projection.join().unwrap().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fast_path_gate_revalidates_dependency_mutation_after_candidate_capture() {
    let used = test_variable("Used");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-fast-path-exact-dependency-gate",
        data,
    );
    let state = project.state();
    let before = crate::node_system::compiler::compile_snapshot_invocations();
    state.graph_projection(&graph_path(), "en-US").unwrap();

    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .update_variable(
            &used.id,
            Some("Used changed after candidate capture".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    release_gate_tx.send(()).unwrap();
    projection.join().unwrap().unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2,
        "the stale fast-path candidate must be rejected and recompiled"
    );
}

#[test]
fn exact_authority_capture_and_candidate_lookup_are_one_atomic_gate() {
    let used = test_variable("Atomic Used");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project =
        crate::project::fixtures::TempProject::activate("compile-exact-capture-lookup-gate", data);
    let state = project.state();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let original = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;

    let (captured_tx, captured_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    let first = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_after_exact_authority_capture_test_hook(std::sync::Arc::new(move || {
        if first.swap(false, std::sync::atomic::Ordering::AcqRel) {
            captured_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    captured_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    let mutation_state = state.clone();
    let (mutation_done_tx, mutation_done_rx) = std::sync::mpsc::channel();
    let mutation = std::thread::spawn(move || {
        mutation_state
            .update_variable(
                &used.id,
                Some("Changed after exact capture".into()),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        mutation_done_tx.send(()).unwrap();
    });
    assert!(
        mutation_done_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );

    release_tx.send(()).unwrap();
    projection.join().unwrap().unwrap();
    mutation.join().unwrap();
    mutation_done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    assert_ne!(
        state
            .published_compile_ids_for_test(&graph_path())
            .unwrap()
            .0,
        original,
    );
}

#[test]
fn fast_path_gate_ignores_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("compile-fast-path-gate");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_gate_tx.send(()).unwrap();

    projection.join().unwrap().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn coalesced_waiters_ignore_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("coalesced-stale-termination");
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let first_state = state.clone();
    let first = std::thread::spawn(move || first_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    let (waiter_paused_tx, waiter_paused_rx) = std::sync::mpsc::channel();
    let (release_waiter_tx, release_waiter_rx) = std::sync::mpsc::channel();
    let release_waiter_rx = std::sync::Mutex::new(release_waiter_rx);
    state.set_compile_coalesced_before_wait_test_hook(std::sync::Arc::new(move || {
        waiter_paused_tx.send(()).unwrap();
        release_waiter_rx.lock().unwrap().recv().unwrap();
    }));
    let second_state = state.clone();
    let second = std::thread::spawn(move || second_state.graph_projection(&graph_path(), "en-US"));
    waiter_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_waiter_tx.send(()).unwrap();
    release_gate_tx.send(()).unwrap();

    for result in [first.join().unwrap(), second.join().unwrap()] {
        result.unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn different_basis_request_compiles_after_authoritative_invalidation() {
    let (state, root) = active_state_with_valid_constant_graph("pending-latest-publication");
    let before = crate::node_system::compiler::compile_snapshot_invocations();
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let active_state = state.clone();
    let active = std::thread::spawn(move || active_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    release_gate_tx.send(()).unwrap();
    active.join().unwrap().unwrap();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();
    let latest = state.graph_projection(&graph_path(), "en-US").unwrap();
    assert_eq!(latest.source_revision, 2);

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn execution_rejects_exact_dependency_change_after_plan_before_run() {
    let used = test_variable("Execution Authority");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "execution-exact-dependency-authority",
        data,
    );
    let state = project.state();
    let mutation_state = state.clone();
    state.set_execution_before_final_gate_test_hook(std::sync::Arc::new(move || {
        mutation_state
            .update_variable(
                &used.id,
                Some("Changed before run".into()),
                None,
                None,
                None,
                None,
            )
            .unwrap();
    }));
    let run_entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let run_entered_for_hook = std::sync::Arc::clone(&run_entered);
    state.set_execution_before_run_test_hook(std::sync::Arc::new(move || {
        run_entered_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();

    assert!(
        error.to_string().contains("stale"),
        "unexpected error: {error}"
    );
    assert!(!run_entered.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn function_resource_version_changes_with_graph_body() {
    let function_path = GraphResourcePath::new("functions/Fingerprint.yssbi-function").unwrap();
    let mut data = ProjectData::new();
    data.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Fingerprint", GraphDocumentKind::Function),
    );
    let key = crate::node_system::analysis::ResourceKey::new(function_path.as_str());
    let before = compile_resources_from_data(&data, Default::default())
        .unwrap()
        .versions[&key]
        .clone();
    let graph = data.graphs.get_mut(&function_path).unwrap();
    graph.document.revision = GraphRevision::new(1);
    let body_node = node("yssbi.constant.int64");
    graph.document.nodes.insert(body_node.id, body_node);
    let after = compile_resources_from_data(&data, Default::default())
        .unwrap()
        .versions[&key]
        .clone();

    assert_ne!(before, after);
}

#[test]
fn database_resource_version_changes_with_resolved_column_type() {
    let declaration = crate::database::DatabaseDecl {
        id: "main".into(),
        engine: crate::database::DatabaseEngine::InMemory {
            name: "main".into(),
        },
        schema_version: 1,
        required: true,
        name: Some("Main".into()),
    };
    let mut data = ProjectData::new();
    data.databases.insert("main".into(), declaration);
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let key = crate::node_system::analysis::ResourceKey::new("databases/main");
    let schema = |dtype: &str| {
        std::collections::BTreeMap::from([(
            resource.clone(),
            vec![crate::schema::ColumnInfoDTO {
                name: "value".into(),
                dtype: dtype.into(),
            }],
        )])
    };

    let int_version = compile_resources_from_data(&data, schema("Int64"))
        .unwrap()
        .versions[&key]
        .clone();
    let string_version = compile_resources_from_data(&data, schema("String"))
        .unwrap()
        .versions[&key]
        .clone();

    assert_ne!(int_version, string_version);
}

#[test]
fn project_execution_runs_valid_plan_through_run_executor() {
    let (state, root) = active_state_with_valid_constant_graph("valid-plan-execution");
    let result = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    assert!(result.run_id.get() > 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_observability_execute_graph_records_compile_and_run_for_current_session() {
    use crate::node_system::analysis::{SpanKind, SpanStatus};

    let (state, root) = active_state_with_valid_constant_graph("production-observability");
    let (session_id, trace_sink) = {
        let store = state.project_store.read().unwrap();
        (
            store.project_session_id.clone(),
            std::sync::Arc::clone(&store.trace_sink),
        )
    };

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();

    let records = trace_sink.records();
    assert!(!records.is_empty());
    assert!(records.iter().all(|record| {
        record.event.correlation.project_session_id == session_id
            && record.event.correlation.graph_path.0.as_ref() == graph_path().as_str()
    }));
    for kind in [SpanKind::Snapshot, SpanKind::Analysis, SpanKind::Lowering] {
        assert!(records.iter().any(|record| {
            record.event.kind == kind && record.event.status == SpanStatus::Succeeded
        }));
    }
    assert!(records.iter().any(|record| {
        record.event.kind == SpanKind::Run && record.event.status == SpanStatus::Started
    }));
    assert!(records.iter().any(|record| {
        record.event.kind == SpanKind::Run && record.event.status == SpanStatus::Succeeded
    }));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_observability_project_replacement_installs_empty_distinct_sink() {
    let (state, root) = active_state_with_valid_constant_graph("trace-sink-replacement");
    let old_sink = {
        let store = state.project_store.read().unwrap();
        std::sync::Arc::clone(&store.trace_sink)
    };
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    assert!(!old_sink.records().is_empty());

    state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());

    let new_sink = {
        let store = state.project_store.read().unwrap();
        std::sync::Arc::clone(&store.trace_sink)
    };
    assert!(!std::sync::Arc::ptr_eq(&old_sink, &new_sink));
    assert!(new_sink.records().is_empty());
    assert!(!old_sink.records().is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

struct SourceRenameLimitFixture {
    state: ProjectState,
    root: std::path::PathBuf,
    path: GraphResourcePath,
    nodes: [DocumentNode; 3],
    connections: [crate::node_system::document::DocumentConnection; 2],
    rename_result_name: String,
    limit_result_name: String,
}

impl SourceRenameLimitFixture {
    fn new(label: &str) -> Self {
        use crate::node_system::document::{ConnectionId, DocumentConnection, PortAddress};
        use crate::node_system::protocol::{ParameterKey, PortKey};

        let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(
            "old_name" => [11_i64, 22, 33, 44],
            "untouched" => [101_i64, 202, 303, 404],
        )
        .unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

        let mut project_data = ProjectData::new();
        project_data.databases.insert(
            "main".into(),
            crate::database::DatabaseDecl {
                id: "main".into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: Some("Main".into()),
            },
        );
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

        let mut source = node("yssbi.dataframe.source.get");
        source.parameters.insert(
            ParameterKey::new("dataframe").unwrap(),
            serde_json::json!("databases/main"),
        );
        let mut rename = node("yssbi.dataframe.rename");
        rename.parameters.insert(
            ParameterKey::new("from").unwrap(),
            serde_json::json!("old_name"),
        );
        rename.parameters.insert(
            ParameterKey::new("to").unwrap(),
            serde_json::json!("new_name"),
        );
        let mut limit = node("yssbi.dataframe.limit");
        limit
            .parameters
            .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
        let rename_result_name = format!("node.{}.result", rename.id);
        let limit_result_name = format!("node.{}.result", limit.id);

        let source_to_rename = DocumentConnection {
            id: ConnectionId::new(),
            output: PortAddress::declared(source.id, PortKey::new("dataframe").unwrap()),
            input: PortAddress::declared(rename.id, PortKey::new("source").unwrap()),
            order: None,
        };
        let rename_to_limit = DocumentConnection {
            id: ConnectionId::new(),
            output: PortAddress::declared(rename.id, PortKey::new("result").unwrap()),
            input: PortAddress::declared(limit.id, PortKey::new("source").unwrap()),
            order: None,
        };

        Self {
            state,
            root,
            path: GraphResourcePath::new("events/SourceRenameLimit.yssbi-event").unwrap(),
            nodes: [source, rename, limit],
            connections: [source_to_rename, rename_to_limit],
            rename_result_name,
            limit_result_name,
        }
    }

    fn document(&self, reversed: bool) -> GraphResourceDocument {
        let mut graph = GraphResourceDocument::new("Source Rename Limit", GraphDocumentKind::Event);
        if reversed {
            for node in self.nodes.iter().rev() {
                graph.document.nodes.insert(node.id, node.clone());
            }
            for connection in self.connections.iter().rev() {
                graph
                    .document
                    .connections
                    .insert(connection.id, connection.clone());
            }
        } else {
            for node in &self.nodes {
                graph.document.nodes.insert(node.id, node.clone());
            }
            for connection in &self.connections {
                graph
                    .document
                    .connections
                    .insert(connection.id, connection.clone());
            }
        }
        graph
    }
}

#[test]
fn project_execute_graph_runs_builtin_dataframe_source_rename_limit() {
    use crate::node_system::plan::{
        RelationalOperator, RelationalOperatorIndex, RelationalRename, ResourceId,
    };
    use crate::node_system::protocol::Value;
    use crate::node_system::runtime::{ProductionRelationalObserver, RuntimeValue};

    let fixture = SourceRenameLimitFixture::new("project-source-rename-limit");
    fixture
        .state
        .insert_graph(fixture.path.clone(), fixture.document(false))
        .unwrap();
    let observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&observer));

    let result = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative Source -> Rename -> Limit graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.relational_subplans.len(), 1);
    let plan = &observation.relational_subplans[0].compiled_plan;
    assert_eq!(
        plan.operators.as_ref(),
        &[
            RelationalOperator::Source {
                resource: ResourceId::new("databases/main").unwrap(),
                relation: "databases/main".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalRename {
                    from: "old_name".into(),
                    to: "new_name".into(),
                }]),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(1),
                rows: 2,
            },
        ]
    );
    assert_eq!(
        plan.roots.as_ref(),
        &[
            RelationalOperatorIndex::new(1),
            RelationalOperatorIndex::new(2),
        ]
    );
    assert_eq!(
        observation.relational_result_bindings,
        vec![
            (
                fixture.rename_result_name.as_str().into(),
                RelationalOperatorIndex::new(1),
            ),
            (
                fixture.limit_result_name.as_str().into(),
                RelationalOperatorIndex::new(2),
            ),
        ]
    );

    let RuntimeValue::Scalar(Value::Object(rename_columns)) = result
        .values
        .get(fixture.rename_result_name.as_str())
        .expect("Rename result must be exposed")
    else {
        panic!("expected Rename dataframe output")
    };
    assert!(!rename_columns.contains_key("old_name"));
    assert_eq!(
        rename_columns,
        &[
            (
                "new_name".into(),
                Value::List(vec![
                    Value::Integer(11),
                    Value::Integer(22),
                    Value::Integer(33),
                    Value::Integer(44),
                ]),
            ),
            (
                "untouched".into(),
                Value::List(vec![
                    Value::Integer(101),
                    Value::Integer(202),
                    Value::Integer(303),
                    Value::Integer(404),
                ]),
            ),
        ]
        .into_iter()
        .collect()
    );

    let RuntimeValue::Scalar(Value::Object(limit_columns)) = result
        .values
        .get(fixture.limit_result_name.as_str())
        .expect("Limit result must be exposed")
    else {
        panic!("expected Limit dataframe output")
    };
    assert!(!limit_columns.contains_key("old_name"));
    assert_eq!(
        limit_columns,
        &[
            (
                "new_name".into(),
                Value::List(vec![Value::Integer(11), Value::Integer(22)]),
            ),
            (
                "untouched".into(),
                Value::List(vec![Value::Integer(101), Value::Integer(202)]),
            ),
        ]
        .into_iter()
        .collect()
    );
    assert!(plan.pushdown_hints.is_empty());
    assert_eq!(observation.scan_limits, vec![None]);

    drop(fixture.state);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn project_execute_graph_source_rename_limit_is_insertion_order_independent() {
    use crate::node_system::runtime::ProductionRelationalObserver;

    let fixture = SourceRenameLimitFixture::new("project-source-rename-limit-order");
    let forward_document = fixture.document(false);
    let reversed_document = fixture.document(true);
    assert_eq!(forward_document.document, reversed_document.document);

    fixture
        .state
        .insert_graph(fixture.path.clone(), forward_document)
        .unwrap();
    let forward_observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&forward_observer));
    let forward = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("forward insertion graph executes");

    fixture
        .state
        .insert_graph(fixture.path.clone(), reversed_document)
        .unwrap();
    let reversed_observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&reversed_observer));
    let mut reversed = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("reversed insertion graph executes");

    assert_eq!(forward_observer.snapshot(), reversed_observer.snapshot());
    assert_ne!(forward.run_id, reversed.run_id);
    assert_eq!(forward.correlation.run_id, Some(forward.run_id));
    assert_eq!(reversed.correlation.run_id, Some(reversed.run_id));
    assert_ne!(forward.correlation.run_id, reversed.correlation.run_id);
    assert_eq!(
        forward.correlation.compile_id,
        forward.provenance.compile_id
    );
    assert_eq!(
        reversed.correlation.compile_id,
        reversed.provenance.compile_id
    );
    assert_ne!(
        forward.provenance.compile_id,
        reversed.provenance.compile_id
    );
    assert_ne!(
        forward.correlation.compile_id,
        reversed.correlation.compile_id
    );
    reversed.run_id = forward.run_id;
    reversed.provenance.compile_id = forward.provenance.compile_id;
    reversed.correlation.run_id = forward.correlation.run_id;
    reversed.correlation.compile_id = forward.correlation.compile_id;
    assert_eq!(forward, reversed);

    drop(fixture.state);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn predicate_cancellation_publishes_no_result_and_releases_project_resources() {
    use crate::node_system::document::{ConnectionId, DocumentConnection, PortAddress};
    use crate::node_system::protocol::{ParameterKey, PortKey};
    use crate::node_system::runtime::{
        ProductionRelationalBackend, ProductionRelationalCheckpoint, ProjectResourceLeaseObserver,
    };

    let fixture = SourceRenameLimitFixture::new("project-predicate-cancellation");
    let source = fixture.nodes[0].clone();
    let rename = fixture.nodes[1].clone();
    let mut filter = node("yssbi.dataframe.filter.rows");
    filter.parameters.insert(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "old_name",
            "operator": "greaterThan",
            "value": { "type": "integer", "value": "20" }
        }),
    );
    let mut project = node("yssbi.dataframe.project");
    project.parameters.insert(
        ParameterKey::new("columns").unwrap(),
        serde_json::json!(["old_name", "untouched"]),
    );
    let connect = |output_node, output, input_node, input| {
        let id = ConnectionId::new();
        DocumentConnection {
            id,
            output: PortAddress::declared(output_node, PortKey::new(output).unwrap()),
            input: PortAddress::declared(input_node, PortKey::new(input).unwrap()),
            order: None,
        }
    };
    let connections = [
        connect(source.id, "dataframe", filter.id, "source"),
        connect(filter.id, "result", project.id, "source"),
        connect(project.id, "result", rename.id, "source"),
    ];
    let mut graph = GraphResourceDocument::new("Predicate cancellation", GraphDocumentKind::Event);
    for node in [source, filter, project, rename] {
        graph.document.nodes.insert(node.id, node);
    }
    for connection in connections {
        graph.document.connections.insert(connection.id, connection);
    }
    fixture
        .state
        .insert_graph(fixture.path.clone(), graph)
        .unwrap();

    let (checkpoint_tx, checkpoint_rx) = std::sync::mpsc::sync_channel(1);
    let checkpoint: std::sync::Arc<
        dyn Fn(ProductionRelationalCheckpoint, &crate::node_system::runtime::CancellationToken)
            + Send
            + Sync,
    > = std::sync::Arc::new(move |point, cancellation| {
        if point == ProductionRelationalCheckpoint::PredicateEvaluation {
            checkpoint_tx.try_send(()).unwrap();
            cancellation.cancel();
        }
    });
    fixture
        .state
        .set_production_relational_backend_factory(std::sync::Arc::new(move || {
            std::sync::Arc::new(
                ProductionRelationalBackend::default()
                    .with_test_checkpoint(std::sync::Arc::clone(&checkpoint)),
            ) as std::sync::Arc<dyn crate::node_system::runtime::RelationalBackend>
        }));
    let leases = ProjectResourceLeaseObserver::default();
    fixture
        .state
        .set_project_resource_lease_observer(leases.clone());
    let events = DemandRunEvents::default();

    let error = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap_err();

    checkpoint_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("predicate checkpoint was reached");
    assert!(matches!(
        error.run_error(),
        Some(crate::node_system::runtime::RunError::Cancelled)
    ));
    let events = events.0.lock().unwrap();
    assert!(events.iter().any(|event| matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::RunCancelled
    )));
    assert!(events.iter().all(|event| !matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::ResultReady { .. }
            | crate::node_system::runtime::RunEventKind::RunCompleted
    )));
    drop(events);
    let store = fixture.state.project_store.read().unwrap();
    assert_eq!(store.runs.active_run_count(), 0);
    assert_eq!(store.results.source_count(), 0);
    drop(store);
    assert!(leases.acquired() > 0);
    assert_eq!(leases.acquired(), leases.dropped());
    assert_eq!(leases.active(), 0);

    drop(fixture.state);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn project_execution_preserves_relational_codes_in_errors_and_terminal_events() {
    struct FailingBackend(crate::node_system::runtime::RelationalErrorCode);

    impl crate::node_system::runtime::RelationalBackend for FailingBackend {
        fn execute(
            &self,
            _: &crate::node_system::runtime::RelationalContext<'_>,
            _: &crate::node_system::plan::CompiledRelationalPlan,
            _: &[crate::node_system::runtime::RuntimeValue],
        ) -> Result<
            crate::node_system::runtime::RelationalExecution,
            crate::node_system::runtime::RelationalError,
        > {
            Err(crate::node_system::runtime::RelationalError::new(
                self.0,
                "sensitive backend detail",
            ))
        }
    }

    for (relational_code, run_code, public_message) in [
        (
            crate::node_system::runtime::RelationalErrorCode::HintInvalid,
            crate::node_system::runtime::RunErrorCode::RelationalHintInvalid,
            "relational pushdown metadata is invalid",
        ),
        (
            crate::node_system::runtime::RelationalErrorCode::TypeMismatch,
            crate::node_system::runtime::RunErrorCode::RelationalTypeMismatch,
            "relational types do not match",
        ),
    ] {
        let fixture = SourceRenameLimitFixture::new("project-relational-error-code");
        fixture
            .state
            .insert_graph(fixture.path.clone(), fixture.document(false))
            .unwrap();
        fixture
            .state
            .set_production_relational_backend_factory(std::sync::Arc::new(move || {
                std::sync::Arc::new(FailingBackend(relational_code))
                    as std::sync::Arc<dyn crate::node_system::runtime::RelationalBackend>
            }));
        let events = DemandRunEvents::default();

        let error = fixture
            .state
            .execute_graph_for_current_project_for_test(
                &fixture.path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &events,
            )
            .unwrap_err();

        assert_eq!(error.to_string(), public_message);
        assert!(!error.to_string().contains("sensitive backend detail"));
        assert!(matches!(
            error.run_error(),
            Some(crate::node_system::runtime::RunError::RelationalFailed {
                code,
                ..
            }) if *code == relational_code
        ));
        let events = events.0.lock().unwrap();
        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::node_system::runtime::RunEventKind::RunErrored { outcome }
                if outcome.code() == run_code
        )));
        assert!(events.iter().all(|event| !matches!(
            event.kind,
            crate::node_system::runtime::RunEventKind::ResultReady { .. }
                | crate::node_system::runtime::RunEventKind::RunCompleted
        )));

        drop(events);
        drop(fixture.state);
        std::fs::remove_dir_all(fixture.root).unwrap();
    }
}

#[test]
fn project_execute_graph_runs_builtin_dataframe_source_limit() {
    use crate::node_system::document::{ConnectionId, DocumentConnection, PortAddress};
    use crate::node_system::protocol::{ParameterKey, PortKey, Value};
    use crate::node_system::runtime::{ProductionRelationalObserver, RuntimeValue};

    let root = std::env::temp_dir().join(format!(
        "yssbi-project-relational-e2e-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("value" => [11_i64, 22, 33, 44]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let mut project_data = ProjectData::new();
    project_data.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: Some("Main".into()),
        },
    );
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

    let mut source = node("yssbi.dataframe.source.get");
    source.parameters.insert(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    let mut limit = node("yssbi.dataframe.limit");
    let result_name = format!("node.{}.result", limit.id);
    limit
        .parameters
        .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
    let connection_id = ConnectionId::new();
    let connection = DocumentConnection {
        id: connection_id,
        output: PortAddress::declared(source.id, PortKey::new("dataframe").unwrap()),
        input: PortAddress::declared(limit.id, PortKey::new("source").unwrap()),
        order: None,
    };
    let mut graph = GraphResourceDocument::new("Relational", GraphDocumentKind::Event);
    graph.document.nodes.insert(source.id, source);
    graph.document.nodes.insert(limit.id, limit);
    graph.document.connections.insert(connection_id, connection);
    let path = GraphResourcePath::new("events/Relational.yssbi-event").unwrap();
    state.insert_graph(path.clone(), graph).unwrap();

    let observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    state.set_production_relational_observer(std::sync::Arc::clone(&observer));

    let result = state
        .execute_graph_for_current_project_for_test(
            &path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative relational graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.scan_limits, vec![Some(2)]);
    assert_eq!(
        result.values.get(result_name.as_str()),
        Some(&RuntimeValue::Scalar(Value::Object(
            [(
                "value".into(),
                Value::List(vec![Value::Integer(11), Value::Integer(22)]),
            )]
            .into_iter()
            .collect(),
        )))
    );

    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductionChainOutput {
    Filter,
    Project,
    Limit,
}

struct ProductionRelationalChainFixture {
    state: ProjectState,
    root: std::path::PathBuf,
    path: GraphResourcePath,
    nodes: [DocumentNode; 5],
}

impl ProductionRelationalChainFixture {
    fn new(label: &str, reverse_uuid_order: bool) -> Self {
        use crate::node_system::document::{ConnectionId, DocumentConnection, NodeId, PortAddress};
        use crate::node_system::protocol::{ParameterKey, PortKey};

        let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(
            "amount" => [10_i64, 20, 30, 40, 50],
            "region" => [Some("east"), None, Some("west"), Some("north"), Some("south")],
            "active" => [true, false, true, false, true],
        )
        .unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

        let ids = if reverse_uuid_order {
            [5_u128, 4, 3, 2, 1]
        } else {
            [1_u128, 2, 3, 4, 5]
        };
        let mut source = node("yssbi.dataframe.source.get");
        source.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[0]));
        source.parameters.insert(
            ParameterKey::new("dataframe").unwrap(),
            serde_json::json!("databases/main"),
        );
        let mut filter = node("yssbi.dataframe.filter.rows");
        filter.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[1]));
        filter.parameters.insert(
            ParameterKey::new("predicate").unwrap(),
            serde_json::json!({
                "column": "amount",
                "operator": "greaterThan",
                "value": { "type": "integer", "value": "10" }
            }),
        );
        let mut project = node("yssbi.dataframe.project");
        project.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[2]));
        project.parameters.insert(
            ParameterKey::new("columns").unwrap(),
            serde_json::json!(["amount", "region"]),
        );
        let mut rename = node("yssbi.dataframe.rename");
        rename.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[3]));
        rename.parameters.insert(
            ParameterKey::new("from").unwrap(),
            serde_json::json!("amount"),
        );
        rename.parameters.insert(
            ParameterKey::new("to").unwrap(),
            serde_json::json!("selected_amount"),
        );
        let mut limit = node("yssbi.dataframe.limit");
        limit.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[4]));
        limit
            .parameters
            .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
        let nodes = [source, filter, project, rename, limit];

        let mut graph = GraphResourceDocument::new("Production chain", GraphDocumentKind::Event);
        for node in nodes.iter().rev() {
            graph.document.nodes.insert(node.id, node.clone());
        }
        let links = [
            (0, "dataframe", 1, "source"),
            (1, "result", 2, "source"),
            (2, "result", 3, "source"),
            (3, "result", 4, "source"),
        ];
        for (offset, (output_node, output, input_node, input)) in links.into_iter().enumerate() {
            let id = ConnectionId::from_uuid(uuid::Uuid::from_u128(100 + offset as u128));
            graph.document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    output: PortAddress::declared(
                        nodes[output_node].id,
                        PortKey::new(output).unwrap(),
                    ),
                    input: PortAddress::declared(
                        nodes[input_node].id,
                        PortKey::new(input).unwrap(),
                    ),
                    order: None,
                },
            );
        }

        let path = GraphResourcePath::new("events/ProductionChain.yssbi-event").unwrap();
        let mut project_data = ProjectData::new();
        project_data.databases.insert(
            "main".into(),
            crate::database::DatabaseDecl {
                id: "main".into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: Some("Main".into()),
            },
        );
        project_data.graphs.insert(path.clone(), graph);
        crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
            .unwrap();
        crate::project::fixtures::write_graph(
            &project_data,
            root.to_string_lossy().as_ref(),
            &path,
        )
        .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

        let fixture = Self {
            state,
            root,
            path,
            nodes,
        };
        fixture.assert_persisted_parameters();
        fixture
    }

    fn node_id(&self, output: ProductionChainOutput) -> crate::node_system::document::NodeId {
        self.nodes[match output {
            ProductionChainOutput::Filter => 1,
            ProductionChainOutput::Project => 2,
            ProductionChainOutput::Limit => 4,
        }]
        .id
    }

    fn output_ref(
        &self,
        output: ProductionChainOutput,
    ) -> crate::node_system::plan::GraphOutputRef {
        crate::node_system::plan::GraphOutputRef {
            graph_path: crate::node_system::document::GraphResourcePath(self.path.as_str().into()),
            port: crate::node_system::document::PortAddress::declared(
                self.node_id(output),
                crate::node_system::protocol::PortKey::new("result").unwrap(),
            ),
        }
    }

    fn demand(&self, output: ProductionChainOutput) -> crate::node_system::plan::ExecutionDemand {
        crate::node_system::plan::ExecutionDemand::Outputs {
            outputs: Box::new([self.output_ref(output)]),
            include_default_results: false,
        }
    }

    fn install(
        &self,
        observer: &std::sync::Arc<crate::node_system::runtime::ProductionRelationalObserver>,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        self.state
            .set_production_relational_observer(std::sync::Arc::clone(observer));
        self.state
            .set_project_resource_lease_observer(leases.clone());
    }

    fn assert_persisted_parameters(&self) {
        use crate::node_system::protocol::ParameterKey;

        let reloaded = ProjectState::new();
        reloaded.activate_project_from_path(&self.root).unwrap();
        let loaded = load_graph(&reloaded, &self.path).unwrap();
        let graph = &loaded.document;
        assert_eq!(
            graph.nodes[&self.nodes[0].id].parameters,
            [(
                ParameterKey::new("dataframe").unwrap(),
                serde_json::json!("databases/main")
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[1].id].parameters,
            [(
                ParameterKey::new("predicate").unwrap(),
                serde_json::json!({
                    "column": "amount",
                    "operator": "greaterThan",
                    "value": { "type": "integer", "value": "10" }
                })
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[2].id].parameters,
            [(
                ParameterKey::new("columns").unwrap(),
                serde_json::json!(["amount", "region"])
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[3].id].parameters,
            [
                (
                    ParameterKey::new("from").unwrap(),
                    serde_json::json!("amount")
                ),
                (
                    ParameterKey::new("to").unwrap(),
                    serde_json::json!("selected_amount"),
                ),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[4].id].parameters,
            [(ParameterKey::new("rows").unwrap(), serde_json::json!(2))]
                .into_iter()
                .collect()
        );
    }

    fn expected_value(output: ProductionChainOutput) -> crate::node_system::runtime::RuntimeValue {
        use crate::node_system::protocol::Value;
        use crate::node_system::runtime::RuntimeValue;

        let amount_name = if output == ProductionChainOutput::Limit {
            "selected_amount"
        } else {
            "amount"
        };
        let amount_values = if output == ProductionChainOutput::Limit {
            vec![Value::Integer(20), Value::Integer(30)]
        } else {
            vec![
                Value::Integer(20),
                Value::Integer(30),
                Value::Integer(40),
                Value::Integer(50),
            ]
        };
        let region_values = if output == ProductionChainOutput::Limit {
            vec![Value::Null, Value::String("west".into())]
        } else {
            vec![
                Value::Null,
                Value::String("west".into()),
                Value::String("north".into()),
                Value::String("south".into()),
            ]
        };
        let mut columns = std::collections::BTreeMap::from([
            (amount_name.into(), Value::List(amount_values)),
            ("region".into(), Value::List(region_values)),
        ]);
        if output == ProductionChainOutput::Filter {
            columns.insert(
                "active".into(),
                Value::List(vec![
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Bool(true),
                ]),
            );
        }
        RuntimeValue::Scalar(Value::Object(columns))
    }

    fn assert_common_success(
        &self,
        output: ProductionChainOutput,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        assert_eq!(result.values.len(), 1);
        assert_eq!(
            result.values.values().next(),
            Some(&Self::expected_value(output))
        );
        let expected_output = self.output_ref(output);
        let expected_name = format!("node.{}.result", self.node_id(output));
        let events = events.0.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::node_system::runtime::RunEventKind::RunCompleted
                ))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::node_system::runtime::RunEventKind::ResultReady { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::node_system::runtime::RunEventKind::OutputReady { .. }
                ))
                .count(),
            1
        );
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            crate::node_system::runtime::RunEventKind::ResultReady { name, .. }
                if name.as_ref() == expected_name
        )));
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            crate::node_system::runtime::RunEventKind::OutputReady { output, .. }
                if output == &expected_output
        )));
        drop(events);
        let store = self.state.project_store.read().unwrap();
        assert_eq!(store.runs.active_run_count(), 0);
        assert_eq!(
            store.results.source_count(),
            1,
            "final-only demand must not expose an intermediate readable source",
        );
        drop(store);
        assert_eq!(leases.acquired(), 1);
        assert_eq!(leases.dropped(), 1);
        assert_eq!(leases.active(), 0);
    }

    fn assert_final_only_acceptance(
        &self,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        observer: &crate::node_system::runtime::ProductionRelationalObserver,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        use crate::node_system::plan::{RelationalOperator, RelationalOperatorIndex};

        self.assert_common_success(ProductionChainOutput::Limit, result, events, leases);
        let observation = observer.snapshot();
        assert_eq!(observation.relational_islands, Some(1));
        assert_eq!(observation.backend_invocations, 1);
        assert_eq!(observation.relational_subplans.len(), 1);
        let plan = &observation.relational_subplans[0].compiled_plan;
        assert_eq!(plan.operators.len(), 5);
        assert_eq!(plan.roots.as_ref(), &[RelationalOperatorIndex::new(4)]);
        assert!(matches!(
            plan.operators[0],
            RelationalOperator::Source { .. }
        ));
        assert!(
            matches!(plan.operators[1], RelationalOperator::Filter { input, .. } if input == RelationalOperatorIndex::new(0))
        );
        assert!(
            matches!(plan.operators[2], RelationalOperator::Project { input, .. } if input == RelationalOperatorIndex::new(1))
        );
        assert!(
            matches!(plan.operators[3], RelationalOperator::Rename { input, .. } if input == RelationalOperatorIndex::new(2))
        );
        assert!(
            matches!(plan.operators[4], RelationalOperator::Limit { input, rows: 2 } if input == RelationalOperatorIndex::new(3))
        );

        let dataframes = observer.materialized_dataframes();
        assert_eq!(dataframes.len(), 1);
        let dataframe = &dataframes[0];
        assert_eq!(
            dataframe
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["selected_amount", "region"]
        );
        assert_eq!(
            dataframe.column("selected_amount").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            dataframe.column("region").unwrap().dtype(),
            &polars::prelude::DataType::String
        );
        assert_eq!(dataframe.column("selected_amount").unwrap().null_count(), 0);
        assert_eq!(dataframe.column("region").unwrap().null_count(), 1);
        assert_eq!(
            dataframe
                .column("selected_amount")
                .unwrap()
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            vec![20, 30]
        );
        assert_eq!(
            dataframe
                .column("region")
                .unwrap()
                .str()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![None, Some("west")]
        );
    }

    fn assert_exact_preview_plan(
        &self,
        output: ProductionChainOutput,
        observer: &crate::node_system::runtime::ProductionRelationalObserver,
    ) {
        use crate::node_system::plan::{
            RelationalExpression, RelationalLiteral, RelationalOperator, RelationalOperatorIndex,
            RelationalProjection, ResourceId,
        };

        let source = RelationalOperator::Source {
            resource: ResourceId::new("databases/main").unwrap(),
            relation: "databases/main".into(),
        };
        let filter = RelationalOperator::Filter {
            input: RelationalOperatorIndex::new(0),
            predicate: RelationalExpression::GreaterThan(
                Box::new(RelationalExpression::Column("amount".into())),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                    10,
                ))),
            ),
        };
        let project = RelationalOperator::Project {
            input: RelationalOperatorIndex::new(1),
            columns: Box::new([
                RelationalProjection {
                    name: "amount".into(),
                    expression: RelationalExpression::Column("amount".into()),
                },
                RelationalProjection {
                    name: "region".into(),
                    expression: RelationalExpression::Column("region".into()),
                },
            ]),
        };
        let (operators, root) = match output {
            ProductionChainOutput::Filter => {
                (vec![source, filter], RelationalOperatorIndex::new(1))
            }
            ProductionChainOutput::Project => (
                vec![source, filter, project],
                RelationalOperatorIndex::new(2),
            ),
            ProductionChainOutput::Limit => panic!("Limit is not a preview prefix"),
        };
        let observation = observer.snapshot();
        let plan = &observation.relational_subplans[0].compiled_plan;

        assert_eq!(plan.operators.as_ref(), operators.as_slice());
        assert_eq!(plan.roots.as_ref(), &[root]);
        assert_eq!(
            observation.relational_result_bindings,
            vec![(
                format!("node.{}.result", self.node_id(output)).into_boxed_str(),
                root,
            )]
        );
        assert_eq!(
            plan.fragment_roots
                .iter()
                .map(|binding| binding.operator)
                .collect::<Vec<_>>(),
            (0..operators.len())
                .map(|index| RelationalOperatorIndex::new(index as u32))
                .collect::<Vec<_>>()
        );
        assert!(plan.operators.iter().all(|operator| !matches!(
            operator,
            RelationalOperator::Rename { .. } | RelationalOperator::Limit { .. }
        )));
    }

    fn assert_preview(
        &self,
        output: ProductionChainOutput,
        expected_operators: usize,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        observer: &crate::node_system::runtime::ProductionRelationalObserver,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        self.assert_common_success(output, result, events, leases);
        let observation = observer.snapshot();
        assert_eq!(observation.relational_islands, Some(1));
        assert_eq!(observation.backend_invocations, 1);
        let plan = &observation.relational_subplans[0].compiled_plan;
        assert_eq!(plan.operators.len(), expected_operators);
        assert_eq!(plan.roots.len(), 1);
        assert_eq!(plan.roots[0].index(), expected_operators - 1);
        assert_eq!(plan.fragment_order.len(), expected_operators);
        assert_eq!(plan.fragment_roots.len(), expected_operators);
        assert_eq!(observer.materialized_dataframes().len(), 1);
        self.assert_exact_preview_plan(output, observer);
    }

    fn normalized_execution(
        &self,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        observer: &crate::node_system::runtime::ProductionRelationalObserver,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) -> serde_json::Value {
        self.assert_common_success(ProductionChainOutput::Limit, result, events, leases);
        let plan = observer.execution_plan();
        let role = |node_id| {
            self.nodes
                .iter()
                .position(|node| node.id == node_id)
                .map(|index| ["source", "filter", "project", "rename", "limit"][index])
                .unwrap()
        };
        serde_json::json!({
            "operators": plan.relational_subplans[0].compiled_plan.operators,
            "roots": plan.relational_subplans[0].compiled_plan.roots,
            "hints": plan.relational_subplans[0].compiled_plan.pushdown_hints,
            "resources": plan.resources,
            "operations": plan.operations.iter().map(|operation| serde_json::json!({
                "sourceRole": role(operation.source_node_id),
                "nodeType": operation.source_node_type_id,
                "kernel": operation.kernel,
                "inputConsumption": operation.inputs.iter().map(|input| input.consumption).collect::<Vec<_>>(),
                "outputProduction": operation.outputs.iter().map(|output| output.production).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "results": plan.results.iter().map(|result| serde_json::json!({
                "role": role(result.output.port.node_id),
                "port": result.output.port.port,
            })).collect::<Vec<_>>(),
            "values": format!("{:?}", result.values.values().collect::<Vec<_>>()),
            "eventKinds": {
                "completed": events.0.lock().unwrap().iter().filter(|event| matches!(event.kind, crate::node_system::runtime::RunEventKind::RunCompleted)).count(),
                "resultReady": events.0.lock().unwrap().iter().filter(|event| matches!(event.kind, crate::node_system::runtime::RunEventKind::ResultReady { .. })).count(),
                "outputReady": events.0.lock().unwrap().iter().filter(|event| matches!(event.kind, crate::node_system::runtime::RunEventKind::OutputReady { .. })).count(),
            },
            "leases": [leases.acquired(), leases.dropped(), leases.active()],
        })
    }

    fn assert_bounded_cancellation(
        &self,
        expected_checkpoint: crate::node_system::runtime::ProductionRelationalCheckpoint,
    ) {
        use crate::node_system::runtime::{
            CancellationToken, ProductionRelationalBackend, ProductionRelationalCheckpoint,
            ProjectResourceLeaseObserver, RelationalBackend, RunError, RunEventKind,
        };

        let (checkpoint_tx, checkpoint_rx) = std::sync::mpsc::sync_channel(1);
        let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sent_for_hook = std::sync::Arc::clone(&sent);
        let checkpoint = std::sync::Arc::new(
            move |point: ProductionRelationalCheckpoint, cancellation: &CancellationToken| {
                if point == expected_checkpoint
                    && !sent_for_hook.swap(true, std::sync::atomic::Ordering::AcqRel)
                {
                    checkpoint_tx.try_send(()).unwrap();
                    cancellation.cancel();
                }
            },
        );
        self.state
            .set_production_relational_backend_factory(std::sync::Arc::new(move || {
                std::sync::Arc::new(
                    ProductionRelationalBackend::default().with_test_checkpoint(checkpoint.clone()),
                ) as std::sync::Arc<dyn RelationalBackend>
            }));
        let leases = ProjectResourceLeaseObserver::default();
        self.state
            .set_project_resource_lease_observer(leases.clone());
        let state = self.state.clone();
        let path = self.path.clone();
        let demand = self.demand(ProductionChainOutput::Limit);
        let events = std::sync::Arc::new(DemandRunEvents::default());
        let thread_events = std::sync::Arc::clone(&events);
        let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
        let execution = std::thread::spawn(move || {
            result_tx
                .send(state.execute_graph_for_current_project_for_test(
                    &path,
                    &demand,
                    thread_events.as_ref(),
                ))
                .unwrap();
        });

        checkpoint_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("requested relational checkpoint was reached");
        let error = result_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("cancelled execution terminated within the bound")
            .unwrap_err();
        execution.join().unwrap();
        assert!(matches!(error.run_error(), Some(RunError::Cancelled)));
        let events = events.0.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, RunEventKind::RunCancelled))
                .count(),
            1
        );
        assert!(events.iter().all(|event| !matches!(
            event.kind,
            RunEventKind::ResultReady { .. }
                | RunEventKind::OutputReady { .. }
                | RunEventKind::RunCompleted
        )));
        drop(events);
        let store = self.state.project_store.read().unwrap();
        assert_eq!(store.runs.active_run_count(), 0);
        assert_eq!(store.results.source_count(), 0);
        drop(store);
        assert_eq!(leases.acquired(), 1);
        assert_eq!(leases.dropped(), 1);
        assert_eq!(leases.active(), 0);
    }

    fn assert_defensive_failure(&self, code: crate::node_system::runtime::RelationalErrorCode) {
        use crate::node_system::runtime::{
            ProjectResourceLeaseObserver, RelationalBackend, RunEventKind,
        };

        struct ForcedFailure(crate::node_system::runtime::RelationalErrorCode);
        impl RelationalBackend for ForcedFailure {
            fn execute(
                &self,
                _: &crate::node_system::runtime::RelationalContext<'_>,
                _: &crate::node_system::plan::CompiledRelationalPlan,
                _: &[crate::node_system::runtime::RuntimeValue],
            ) -> Result<
                crate::node_system::runtime::RelationalExecution,
                crate::node_system::runtime::RelationalError,
            > {
                Err(crate::node_system::runtime::RelationalError::new(
                    self.0,
                    "defensive acceptance failure",
                ))
            }
        }

        self.state
            .set_production_relational_backend_factory(std::sync::Arc::new(move || {
                std::sync::Arc::new(ForcedFailure(code)) as std::sync::Arc<dyn RelationalBackend>
            }));
        let leases = ProjectResourceLeaseObserver::default();
        self.state
            .set_project_resource_lease_observer(leases.clone());
        let events = DemandRunEvents::default();
        let error = self
            .state
            .execute_graph_for_current_project_for_test(
                &self.path,
                &self.demand(ProductionChainOutput::Limit),
                &events,
            )
            .unwrap_err();
        assert!(matches!(
            error.run_error(),
            Some(crate::node_system::runtime::RunError::RelationalFailed {
                code: actual,
                ..
            }) if *actual == code
        ));
        let events = events.0.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, RunEventKind::RunErrored { .. }))
                .count(),
            1
        );
        assert!(events.iter().all(|event| !matches!(
            event.kind,
            RunEventKind::ResultReady { .. }
                | RunEventKind::OutputReady { .. }
                | RunEventKind::RunCompleted
        )));
        drop(events);
        let store = self.state.project_store.read().unwrap();
        assert_eq!(store.runs.active_run_count(), 0);
        assert_eq!(store.results.source_count(), 0);
        drop(store);
        assert_eq!(leases.acquired(), 1);
        assert_eq!(leases.dropped(), 1);
        assert_eq!(leases.active(), 0);
    }

    fn cleanup(self) {
        drop(self.state);
        std::fs::remove_dir_all(self.root).unwrap();
    }
}

fn parameterized_static_ui_route_fixture() -> serde_json::Value {
    use crate::node_system::document::{ConnectionId, NodePosition, PortAddress};
    use crate::node_system::protocol::ParameterKey;

    fn normalize_string(value: &mut serde_json::Value, from: &str, to: &str) {
        match value {
            serde_json::Value::String(text) if text.contains(from) => {
                *text = text.replace(from, to)
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    normalize_string(value, from, to);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values_mut() {
                    normalize_string(value, from, to);
                }
            }
            _ => {}
        }
    }

    let fixture = ProductionRelationalChainFixture::new("ui-route-fixture", false);
    let session = fixture.state.capture_project_session().unwrap();
    let mut catalog = fixture
        .state
        .localized_catalog_snapshot(&session.instance_id, "en-US")
        .unwrap();
    let project_item = catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == "yssbi.dataframe.project")
        .unwrap()
        .clone();
    let project_category = catalog
        .categories
        .iter()
        .find(|category| category.category_id == project_item.category_id)
        .unwrap()
        .clone();
    catalog.project_instance_id = "fixture-project".into();
    catalog.categories = vec![project_category];
    catalog.items = vec![project_item.clone()];

    let prepare_result = fixture
        .state
        .apply_editor_graph_mutation(
            &session.instance_id,
            &fixture.path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    fixture.path.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::from_uuid(uuid::Uuid::from_u128(199)),
                EditorGraphMutationDto::Disconnect {
                    connection_id: ConnectionId::from_uuid(uuid::Uuid::from_u128(100)),
                },
            ),
        )
        .unwrap();
    let initial_projection = prepare_result.projection_replacement.projection;
    let create_operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(200));
    let create_mutation = EditorGraphMutationDto::CreateNode {
        descriptor: project_item.creation,
        position: NodePosition { x: 320.0, y: 180.0 },
        user_label: None,
    };
    let before_ids = fixture.state.get_data().unwrap().graphs[&fixture.path]
        .document
        .nodes
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let create_result = fixture
        .state
        .apply_editor_graph_mutation(
            &session.instance_id,
            &fixture.path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    fixture.path.as_str().into(),
                )),
                GraphRevision::new(1),
                create_operation_id,
                create_mutation.clone(),
            ),
        )
        .unwrap();
    let created_node_id = fixture.state.get_data().unwrap().graphs[&fixture.path]
        .document
        .nodes
        .keys()
        .find(|node_id| !before_ids.contains(node_id))
        .copied()
        .unwrap();
    assert_eq!(
        fixture.state.get_data().unwrap().graphs[&fixture.path]
            .document
            .nodes[&created_node_id]
            .node_type
            .as_str(),
        "yssbi.dataframe.project"
    );
    assert!(
        fixture.state.get_data().unwrap().graphs[&fixture.path]
            .document
            .nodes[&created_node_id]
            .parameters
            .is_empty()
    );

    let source_output = PortAddress::declared(
        fixture.nodes[0].id,
        crate::node_system::protocol::PortKey::new("dataframe").unwrap(),
    );
    let project_input = PortAddress::declared(
        created_node_id,
        crate::node_system::protocol::PortKey::new("source").unwrap(),
    );
    let connect_operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(201));
    let connect_mutation = EditorGraphMutationDto::Connect {
        output: source_output.clone().into(),
        input: project_input.clone().into(),
        order: None,
    };
    let connect_result = fixture
        .state
        .apply_editor_graph_mutation(
            &session.instance_id,
            &fixture.path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    fixture.path.as_str().into(),
                )),
                GraphRevision::new(2),
                connect_operation_id,
                connect_mutation.clone(),
            ),
        )
        .unwrap();
    let connected_projection =
        serde_json::to_value(&connect_result.projection_replacement).unwrap();
    let connected_node = connected_projection["projection"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["nodeId"] == created_node_id.to_string())
        .unwrap();
    let connected_editor = connected_node["parameterEditors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|editor| editor["key"] == "columns")
        .unwrap();
    assert_eq!(connected_editor["configuration"]["available"], true);
    assert_eq!(
        connected_editor["configuration"]["value"],
        serde_json::json!([])
    );
    assert_eq!(
        connected_editor["configuration"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["amount", "region", "active"]
    );

    let submit_operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(202));
    let submit_mutation = EditorGraphMutationDto::SetParameters {
        node_id: created_node_id,
        parameters: [(
            ParameterKey::new("columns").unwrap(),
            serde_json::json!(["region", "amount"]),
        )]
        .into_iter()
        .collect(),
    };
    let submit_result = fixture
        .state
        .apply_editor_graph_mutation(
            &session.instance_id,
            &fixture.path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    fixture.path.as_str().into(),
                )),
                GraphRevision::new(3),
                submit_operation_id,
                submit_mutation.clone(),
            ),
        )
        .unwrap();
    assert_eq!(
        fixture.state.get_data().unwrap().graphs[&fixture.path]
            .document
            .nodes[&created_node_id]
            .parameters,
        [(
            ParameterKey::new("columns").unwrap(),
            serde_json::json!(["region", "amount"]),
        )]
        .into_iter()
        .collect()
    );

    let mut create_result = serde_json::to_value(create_result).unwrap();
    let mut connect_result = serde_json::to_value(connect_result).unwrap();
    let mut submit_result = serde_json::to_value(submit_result).unwrap();
    let created_node_id = created_node_id.to_string();
    let connection_id = connect_result["delta"]["payload"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|operation| operation["connection"]["id"].as_str())
        .unwrap()
        .to_owned();
    let normalized_created_node_id = "00000000-0000-0000-0000-000000000009";
    let normalized_connection_id = "00000000-0000-0000-0000-000000000068";
    for result in [&mut create_result, &mut connect_result, &mut submit_result] {
        normalize_string(result, session.instance_id.as_str(), "fixture-project");
        normalize_string(result, &created_node_id, normalized_created_node_id);
        normalize_string(result, &connection_id, normalized_connection_id);
    }

    let mut result = serde_json::json!({
        "catalog": catalog,
        "graphPath": fixture.path.as_str(),
        "projectNodeId": normalized_created_node_id,
        "initialProjection": initial_projection,
        "create": {
            "locale": "en-US",
            "position": { "x": 320.0, "y": 180.0 },
            "operationId": create_operation_id,
            "mutation": create_mutation,
            "result": create_result,
        },
        "connect": {
            "locale": "en-US",
            "operationId": connect_operation_id,
            "mutation": connect_mutation,
            "result": connect_result,
        },
        "submit": {
            "locale": "en-US",
            "operationId": submit_operation_id,
            "selectedColumns": ["region", "amount"],
            "mutation": submit_mutation,
            "result": submit_result,
        },
        "sourceOutput": source_output,
        "projectInput": project_input,
    });
    normalize_string(&mut result, &created_node_id, normalized_created_node_id);
    normalize_string(&mut result, &connection_id, normalized_connection_id);
    fixture.cleanup();
    result
}

#[test]
fn parameterized_static_ui_route_fixture_is_rust_authoritative() {
    let actual = parameterized_static_ui_route_fixture();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/tests/fixtures/parameterized-static-production-route.json");
    if std::env::var_os("YSSBI_UPDATE_PARAMETERIZED_STATIC_ROUTE_FIXTURE").is_some() {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&actual).unwrap()),
        )
        .unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("shared fixture is missing at {}: {error}", path.display()));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&expected).unwrap(),
        actual
    );
}

#[test]
fn production_relational_chain_persisted_parameters_reload_from_disk_authority() {
    use crate::node_system::protocol::ParameterKey;

    let fixture = ProductionRelationalChainFixture::new("persisted-reload", false);
    {
        let mut data = fixture.state.project_data.write().unwrap();
        let graph = &mut data.graphs.get_mut(&fixture.path).unwrap().document;
        graph
            .nodes
            .get_mut(&fixture.nodes[1].id)
            .unwrap()
            .parameters
            .insert(
                ParameterKey::new("predicate").unwrap(),
                serde_json::json!({ "column": "corrupted" }),
            );
        graph
            .nodes
            .get_mut(&fixture.nodes[2].id)
            .unwrap()
            .parameters
            .insert(
                ParameterKey::new("columns").unwrap(),
                serde_json::json!(["corrupted"]),
            );
    }

    fixture.assert_persisted_parameters();
    fixture.cleanup();
}

#[test]
fn production_relational_chain_final_only_demand_publishes_only_exact_final_value() {
    let fixture = ProductionRelationalChainFixture::new("final-only", false);
    let events = DemandRunEvents::default();
    let observer =
        std::sync::Arc::new(crate::node_system::runtime::ProductionRelationalObserver::default());
    let leases = crate::node_system::runtime::ProjectResourceLeaseObserver::default();
    fixture.install(&observer, &leases);

    let result = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &fixture.demand(ProductionChainOutput::Limit),
            &events,
        )
        .expect("final-only production chain executes");

    fixture.assert_final_only_acceptance(&result, &events, &observer, &leases);
    fixture.cleanup();
}

#[test]
fn production_relational_chain_filter_and_project_previews_prune_suffixes() {
    for (output, expected_operators) in [
        (ProductionChainOutput::Filter, 2),
        (ProductionChainOutput::Project, 3),
    ] {
        let fixture = ProductionRelationalChainFixture::new("preview", false);
        let events = DemandRunEvents::default();
        let observer = std::sync::Arc::new(
            crate::node_system::runtime::ProductionRelationalObserver::default(),
        );
        let leases = crate::node_system::runtime::ProjectResourceLeaseObserver::default();
        fixture.install(&observer, &leases);

        let result = fixture
            .state
            .execute_graph_for_current_project_for_test(
                &fixture.path,
                &fixture.demand(output),
                &events,
            )
            .expect("stable preview demand executes");

        fixture.assert_preview(
            output,
            expected_operators,
            &result,
            &events,
            &observer,
            &leases,
        );
        fixture.cleanup();
    }
}

#[test]
fn production_relational_chain_previews_freeze_exact_operator_prefixes() {
    for output in [
        ProductionChainOutput::Filter,
        ProductionChainOutput::Project,
    ] {
        let fixture = ProductionRelationalChainFixture::new("preview-shape", false);
        let events = DemandRunEvents::default();
        let observer = std::sync::Arc::new(
            crate::node_system::runtime::ProductionRelationalObserver::default(),
        );
        let leases = crate::node_system::runtime::ProjectResourceLeaseObserver::default();
        fixture.install(&observer, &leases);
        fixture
            .state
            .execute_graph_for_current_project_for_test(
                &fixture.path,
                &fixture.demand(output),
                &events,
            )
            .unwrap();

        fixture.assert_exact_preview_plan(output, &observer);
        fixture.cleanup();
    }
}

#[test]
fn production_relational_chain_is_uuid_sort_order_independent() {
    let execute = |reverse_uuid_order| {
        let fixture = ProductionRelationalChainFixture::new("determinism", reverse_uuid_order);
        let events = DemandRunEvents::default();
        let observer = std::sync::Arc::new(
            crate::node_system::runtime::ProductionRelationalObserver::default(),
        );
        let leases = crate::node_system::runtime::ProjectResourceLeaseObserver::default();
        fixture.install(&observer, &leases);
        let result = fixture
            .state
            .execute_graph_for_current_project_for_test(
                &fixture.path,
                &fixture.demand(ProductionChainOutput::Limit),
                &events,
            )
            .expect("determinism fixture executes");
        let normalized = fixture.normalized_execution(&result, &events, &observer, &leases);
        fixture.cleanup();
        normalized
    };

    assert_eq!(execute(false), execute(true));
}

#[test]
fn production_relational_chain_cancellation_is_bounded_and_cleans_up_exactly() {
    for checkpoint in [
        crate::node_system::runtime::ProductionRelationalCheckpoint::PredicateEvaluation,
        crate::node_system::runtime::ProductionRelationalCheckpoint::ResultConversion,
    ] {
        let fixture = ProductionRelationalChainFixture::new("cancellation", false);
        fixture.assert_bounded_cancellation(checkpoint);
        fixture.cleanup();
    }
}

#[test]
fn production_relational_chain_defensive_failures_publish_nothing_and_clean_up() {
    for code in [
        crate::node_system::runtime::RelationalErrorCode::HintInvalid,
        crate::node_system::runtime::RelationalErrorCode::TypeMismatch,
    ] {
        let fixture = ProductionRelationalChainFixture::new("defensive-failure", false);
        fixture.assert_defensive_failure(code);
        fixture.cleanup();
    }
}
