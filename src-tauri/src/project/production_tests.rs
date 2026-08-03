use super::*;
use crate::node_system::document::{
    DocumentNode, EditorGraphMutationDto, GraphDocumentOperation, GraphDocumentPatch,
    GraphMutation, GraphRevision, HistoryMutation, MutationConflict, MutationRequest, OperationId,
    ParameterValues, ResourceKey,
};
use crate::node_system::protocol::NodeTypeId;
use crate::node_system::runtime::NOOP_RUN_EVENT_SINK;

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Production.yssbi-event").unwrap()
}

fn document_path() -> crate::node_system::document::GraphResourcePath {
    crate::node_system::document::GraphResourcePath(graph_path().as_str().into())
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

fn state_with_empty_graph() -> ProjectState {
    let state = ProjectState::new();
    state.insert_graph(
        graph_path(),
        GraphResourceDocument::new("Production", GraphDocumentKind::Event),
    );
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
            .apply_editor_graph_mutation_observed(&graph_path(), "en-US", request, |_| {
                event_count.set(event_count.get() + 1);
            })
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
fn resource_descriptor_publication_materializes_exact_variable_binding() {
    let (state, root, variable_id) = resource_descriptor_fixture("resource-descriptor-valid");

    let result = state
        .apply_editor_graph_mutation(
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
            &graph_path(),
            "en-US",
            resource_descriptor_request(variable_id),
            move |_| {
                observed_by_callback.store(true, std::sync::atomic::Ordering::Release);
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "catalog_resource_stale");
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

    crate::project::set_project_filesystem_rollback_fault(true);
    let result =
        state.rename_graph_resource_fixture(&state.project_instance_id(), &source, "After");
    crate::project::set_project_filesystem_rollback_fault(false);

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

    crate::project::set_project_filesystem_rollback_fault(true);
    let upsert_result = upsert_state.upsert_worksheet_document(worksheet.clone());
    crate::project::set_project_filesystem_rollback_fault(false);

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

    crate::project::set_project_filesystem_rollback_fault(true);
    let remove_result = remove_state.remove_worksheet_document(&worksheet.id);
    crate::project::set_project_filesystem_rollback_fault(false);

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
            &graph,
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    assert!(matches!(
        state.update_function_signature_observed(
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
    crate::project::set_project_filesystem_rollback_fault(true);
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
            .execute_graph(
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
        .apply_editor_graph_mutation(&graph_path(), "en-US", request)
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
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();
    state
        .undo_last_transaction_observed(
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
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
        .unwrap();

    let undo = state
        .undo_last_transaction_observed(
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
fn project_reload_clears_history_status() {
    let state = state_with_empty_graph();
    state
        .apply_editor_graph_mutation(
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
    ProjectState,
    GraphResourcePath,
    GraphResourcePath,
    ResourceKey,
) {
    let state = ProjectState::new();
    let function_path =
        GraphResourcePath::new(format!("functions/{label}.yssbi-function")).unwrap();
    let caller_path = GraphResourcePath::new(format!("events/{label}Caller.yssbi-event")).unwrap();
    state.insert_graph(
        function_path.clone(),
        GraphResourceDocument::new(label, GraphDocumentKind::Function),
    );
    let mut caller =
        GraphResourceDocument::new(format!("{label} Caller"), GraphDocumentKind::Event);
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    caller.document.nodes.insert(call.id, call);
    state.insert_graph(caller_path.clone(), caller);
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
    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        mutation_state.apply_editor_graph_mutation(
            &graph_path(),
            "en-US",
            editor_mutation_request(GraphRevision::INITIAL, OperationId::new()),
        )
    });
    projection_started_rx.recv().unwrap();

    state
        .undo_last_transaction_observed(
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
    state.set_mutation_publication_test_hook(std::sync::Arc::new(move || {
        history_changed_tx.send(()).unwrap();
        release_publication_rx.lock().unwrap().recv().unwrap();
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
    history_changed_rx.recv().unwrap();

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
    mutation.join().unwrap().unwrap();
    status.join().unwrap();
    assert_eq!(
        status_rx.recv().unwrap(),
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
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
        executing_state.execute_graph(
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
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Tax.yssbi-function").unwrap();
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Tax", GraphDocumentKind::Function),
    );
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
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Revisioned.yssbi-function").unwrap();
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Revisioned", GraphDocumentKind::Function),
    );
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
        state.undo_last_transaction_observed("en-US", stale_undo, |_| {}),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let undo_operation = OperationId::new();
    let undo_result = state
        .undo_last_transaction_observed(
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

    let stale_redo = MutationRequest::new(
        undo_deltas[0].resource.clone(),
        GraphRevision::new(1),
        OperationId::new(),
        HistoryMutation {},
    );
    assert!(matches!(
        state.redo_last_transaction_observed("en-US", stale_redo, |_| {}),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let redo_operation = OperationId::new();
    let redo_result = state
        .redo_last_transaction_observed(
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
    let state = ProjectState::new();
    let function_path = GraphResourcePath::new("functions/Declared.yssbi-function").unwrap();
    let caller_path = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    state.insert_graph(
        function_path.clone(),
        GraphResourceDocument::new("Declared", GraphDocumentKind::Function),
    );
    let mut caller = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function_path.as_str()),
    );
    caller.document.nodes.insert(call.id, call);
    state.insert_graph(caller_path.clone(), caller);

    let operation_id = OperationId::new();
    let result = state
        .update_function_signature_observed(
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
    let state = ProjectState::new();
    let first_path = GraphResourcePath::new("functions/First.yssbi-function").unwrap();
    let second_path = GraphResourcePath::new("functions/Second.yssbi-function").unwrap();
    let caller_path = GraphResourcePath::new("events/SharedCaller.yssbi-event").unwrap();
    for (path, name) in [(&first_path, "First"), (&second_path, "Second")] {
        state.insert_graph(
            path.clone(),
            GraphResourceDocument::new(name, GraphDocumentKind::Function),
        );
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
    state.insert_graph(caller_path, caller);

    let (first_projection_tx, first_projection_rx) = std::sync::mpsc::channel();
    let (release_first_tx, release_first_rx) = std::sync::mpsc::channel();
    let release_first_rx = std::sync::Mutex::new(release_first_rx);
    let projection_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = std::sync::Arc::clone(&projection_calls);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            first_projection_tx.send(()).unwrap();
            release_first_rx.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }));

    let (published_tx, published_rx) = std::sync::mpsc::channel();
    let spawn_signature = |path: GraphResourcePath, return_type: &'static str| {
        let mutation_state = state.clone();
        let published_tx = published_tx.clone();
        std::thread::spawn(move || {
            mutation_state
                .update_function_signature_observed(
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
                            .unwrap();
                    },
                )
                .unwrap()
        })
    };

    let first = spawn_signature(first_path, "Int64");
    first_projection_rx.recv().unwrap();
    let second = spawn_signature(second_path, "Float64");
    assert_eq!(
        published_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap(),
        2,
        "the second commit must publish while the first projection is blocked",
    );
    release_first_tx.send(()).unwrap();
    assert_eq!(published_rx.recv().unwrap(), 1);
    assert_eq!(second.join().unwrap().publication_revision, 2);
    assert_eq!(first.join().unwrap().publication_revision, 1);
}

#[test]
fn resource_publication_revision_restarts_for_a_replacement_project() {
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Revisioned.yssbi-function").unwrap();
    let mutate = |state: &ProjectState, return_type: &str| {
        state.insert_graph(
            path.clone(),
            GraphResourceDocument::new("Revisioned", GraphDocumentKind::Function),
        );
        state
            .update_function_signature_observed(
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

    let previous = mutate(&state, "Int64");
    assert_eq!(previous.publication_revision, 1);
    state.activate_project_fixture("replacement-project".into(), ProjectData::new());
    let replacement = mutate(&state, "Float64");
    assert_eq!(replacement.publication_revision, 1);
    assert_ne!(
        previous.project_instance_id,
        replacement.project_instance_id
    );
}

#[test]
fn delayed_old_project_result_keeps_its_original_instance_identity() {
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Delayed.yssbi-function").unwrap();
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Delayed", GraphDocumentKind::Function),
    );
    let (projection_started_tx, projection_started_rx) = std::sync::mpsc::channel();
    let (release_projection_tx, release_projection_rx) = std::sync::mpsc::channel();
    let release_projection_rx = std::sync::Mutex::new(release_projection_rx);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls = std::sync::Arc::clone(&calls);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if hook_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            projection_started_tx.send(()).unwrap();
            release_projection_rx.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }));

    let old_state = state.clone();
    let old_path = path.clone();
    let old = std::thread::spawn(move || {
        old_state
            .update_function_signature_observed(
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
            )
            .unwrap()
    });
    projection_started_rx.recv().unwrap();

    state.activate_project_fixture("replacement-project".into(), ProjectData::new());
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Delayed", GraphDocumentKind::Function),
    );
    let replacement = state
        .update_function_signature_observed(
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
    release_projection_tx.send(()).unwrap();
    let delayed = old.join().unwrap();

    assert_eq!(delayed.publication_revision, 1);
    assert_eq!(replacement.publication_revision, 1);
    assert_ne!(delayed.project_instance_id, replacement.project_instance_id);
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
        .execute_graph(
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
    let registry = crate::node_system::catalog::build_builtin_registry();
    let compiler = crate::node_system::compiler::GraphCompiler::with_interface_resolvers(
        &registry,
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
        .execute_graph(
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
        .execute_graph(
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
            .execute_graph(
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
        .execute_graph(
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
    let context = crate::node_system::runtime::RelationalContext {
        run_id: crate::node_system::analysis::RunId::new(1),
        resources: &resources,
        cancellation: &cancellation,
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
        bridge_inputs: Box::new([]),
        requested_fragment_outputs: Box::new([]),
        roots: Box::new([crate::node_system::plan::RelationalOperatorIndex::new(1)]),
        pushdown_hints: Box::new([crate::node_system::plan::RelationalPushdownHint::Limit {
            source: crate::node_system::plan::RelationalOperatorIndex::new(0),
            rows: 2,
        }]),
    };

    let result = crate::node_system::runtime::ProductionRelationalBackend::default()
        .execute(&context, &plan, &[], &[])
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
        .execute_graph(
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
        .execute_graph(
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
        .execute_graph(&graph_path(), &invalid_demand, &NOOP_RUN_EVENT_SINK)
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
        .execute_graph(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
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
        .execute_graph(&graph_path(), &unavailable_demand, &events)
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

    crate::project::set_project_filesystem_fault(Some(
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
    let project_instance_id = state.capture_project_session().unwrap().instance_id;

    crate::project::set_project_filesystem_fault(Some(
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
    let failed_index = state.read_project_index(&project_instance_id).unwrap();
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
    let success_index = state.read_project_index(&project_instance_id).unwrap();
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

fn temp_project_with_valid_constant_graph(label: &str) -> crate::project::fixtures::TempProject {
    let project = temp_project_with_empty_graph(label);
    let mut constant = node("yssbi.constant.int64");
    constant.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(7),
    );
    project
        .state()
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
fn projection_and_execution_reuse_one_compile_product() {
    let (state, root) = active_state_with_valid_constant_graph("projection-execution-reuse");
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    state
        .execute_graph(
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
fn default_and_two_demands_reuse_one_basis_compile_with_distinct_selection_digests() {
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
        .execute_graph(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let first_output = output(first_node);
    let first_events = DemandRunEvents::default();
    let first_run = state
        .execute_graph(&graph_path(), &demand(first_output.clone()), &first_events)
        .unwrap();
    let second_run = state
        .execute_graph(
            &graph_path(),
            &demand(output(second_node)),
            &NOOP_RUN_EVENT_SINK,
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
        .execute_graph(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
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
        .execute_graph(
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
fn authority_mismatch_rejects_populated_variant_without_overwriting_current_product() {
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

    let error = state
        .execute_graph(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap_err();

    assert!(error.contains("stale_project_lifecycle"), "{error}");
    let (compile_id, variants) = observed.lock().unwrap().unwrap();
    assert_eq!(variants, 1);
    assert!(events.0.lock().unwrap().iter().all(|event| !matches!(
        event.kind,
        crate::node_system::runtime::RunEventKind::OperationStarted { .. }
    )));
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
        .execute_graph(
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
fn stale_compile_cannot_restore_an_older_plan() {
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
    let coordinator = state.compile_coordinator.read().unwrap().clone();

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

    assert!(!coordinator.contains_slot_for_test(&document_path()));
    release_gate_tx.send(()).unwrap();
    let stale_error = stale.join().unwrap().unwrap_err();
    assert!(stale_error.contains("stale_project_lifecycle"));

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
fn function_body_mutations_invalidate_other_graph_compile_slots() {
    for entry in ["mutation", "patch"] {
        let (state, function_path, caller_path, _) =
            function_state_with_caller(&format!("FunctionBody{entry}"));
        state.graph_projection(&caller_path, "en-US").unwrap();
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
            !coordinator.contains_slot_for_test(&caller_document_path),
            "{entry} left a dependent caller compile slot published"
        );
    }
}

#[test]
fn project_replacement_detaches_old_compile_generation_and_populated_variants() {
    let (state, root) = active_state_with_valid_constant_graph("replace-compile-generation");
    state
        .execute_graph(
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
fn committed_source_cannot_rebind_after_authority_generation_aba() {
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
        crate::event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
    assert!(result.projection_replacements.is_empty());
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
        execution_state.execute_graph(
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
fn publish_gate_rejects_authority_generation_change() {
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

    let error = projection.join().unwrap().unwrap_err();
    assert!(error.contains("stale_project_lifecycle"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fast_path_gate_rejects_authority_generation_change() {
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

    let error = projection.join().unwrap().unwrap_err();
    assert!(error.contains("stale_project_lifecycle"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn coalesced_waiter_terminates_when_authority_generation_changes() {
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
        let error = result.unwrap_err();
        assert!(error.contains("stale_project_lifecycle"));
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

    release_gate_tx.send(()).unwrap();
    let active_error = active.join().unwrap().unwrap_err();
    assert!(active_error.contains("stale_project_lifecycle"));
    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn execution_rejects_function_body_change_after_main_plan_before_run() {
    let project = temp_project_with_valid_constant_graph("execution-authority-gate");
    let state = project.state();
    let function_path = state
        .create_graph_resource_fixture("Authority", GraphDocumentKind::Function)
        .unwrap();

    let (plan_ready_tx, plan_ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    state.set_execution_before_final_gate_test_hook(std::sync::Arc::new(move || {
        let _ = plan_ready_tx.send(());
        let _ = release_rx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recv_timeout(std::time::Duration::from_secs(5));
    }));
    let run_entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let run_entered_for_hook = std::sync::Arc::clone(&run_entered);
    state.set_execution_before_run_test_hook(std::sync::Arc::new(move || {
        run_entered_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));

    let error = std::thread::scope(|scope| {
        let executing_state = state.clone();
        let (execution_done_tx, execution_done_rx) = std::sync::mpsc::channel();
        let execution = scope.spawn(move || {
            let result = executing_state.execute_graph(
                &graph_path(),
                &crate::node_system::plan::ExecutionDemand::Default,
                &NOOP_RUN_EVENT_SINK,
            );
            let _ = execution_done_tx.send(());
            result
        });
        plan_ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let (selected_compile_id, variants) = state
            .published_variant_cache_state_for_test(&graph_path())
            .unwrap();
        assert_eq!(variants, 1);
        state
            .apply_graph_patch(
                &function_path,
                MutationRequest::new(
                    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        function_path.as_str().into(),
                    )),
                    GraphRevision::INITIAL,
                    OperationId::new(),
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                        node: node("yssbi.constant.int64"),
                    }]),
                ),
            )
            .unwrap();
        assert!(
            state
                .published_variant_cache_state_for_test(&graph_path())
                .is_none()
        );
        assert!(selected_compile_id.get() > 0);
        release_tx.send(()).unwrap();
        execution_done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        execution.join().unwrap().unwrap_err()
    });
    assert!(error.contains("stale"), "unexpected error: {error}");
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
        .execute_graph(
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
        .execute_graph(
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
        .execute_graph(
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
        .execute_graph(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative Source -> Rename -> Limit graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.materialization_bridges, Some(0));
    assert_eq!(observation.bridge_inputs, vec![0]);
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
        .execute_graph(
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
        .execute_graph(
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
        .execute_graph(
            &path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative relational graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.materialization_bridges, Some(0));
    assert_eq!(observation.bridge_inputs, vec![0]);
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
