use super::*;

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
            connect_from: None,
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
            name: "Sales".into(),
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
            connect_from: None,
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

    assert_eq!(error.code(), "mutation_conflict");
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
    state.insert_graph(from.clone(), original.clone()).unwrap();
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
