use super::*;

#[test]
fn destination_appearance_rejects_graph_move_without_authoritative_effects() {
    let (state, root) = state_with_project_path("destination-conflict");
    let from = GraphResourcePath::new("events/Source.yssbi-event").unwrap();
    let to = GraphResourcePath::new("events/Destination.yssbi-event").unwrap();
    let source = GraphResourceDocument::new("Source", GraphDocumentKind::Event);
    state.insert_graph(from.clone(), source.clone()).unwrap();
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
    state
        .insert_graph(
            to.clone(),
            GraphResourceDocument::new("Concurrent", GraphDocumentKind::Event),
        )
        .unwrap();

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
    let (worksheet_path, worksheet) = fixtures::worksheet("Recovery", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet_path.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet_path);
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
            .load_worksheet_document(&context.session.instance_id, &worksheet_path)
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
                    path: worksheet_path.clone(),
                    revision: ResourceRevision::INITIAL,
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
fn unwind_rollback_failure_blocks_mutations_until_activation() {
    let (state, root) = state_with_project_path("recovery-boundary");
    let (worksheet_path, worksheet) = fixtures::worksheet("Blocked Read", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet_path.clone(), worksheet.clone());
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
        .load_worksheet_document(&context.session.instance_id, &worksheet_path)
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
    state
        .insert_graph(
            event.clone(),
            GraphResourceDocument::new("Recovery", GraphDocumentKind::Event),
        )
        .unwrap();
    state
        .insert_graph(
            function.clone(),
            GraphResourceDocument::new("Recovery", GraphDocumentKind::Function),
        )
        .unwrap();
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
    assert_eq!(
        state
            .execute_graph_for_current_project_for_test(
                &event,
                &crate::node_system::plan::ExecutionDemand::Default,
                &NOOP_RUN_EVENT_SINK
            )
            .unwrap_err()
            .kind(),
        crate::project::ProjectExecutionErrorKind::RecoveryRequired
    );
    assert_eq!(
        state
            .with_database_writer(
                &crate::project::ProjectInstanceId::from_existing("blocked".into()),
                "missing",
                GraphRevision::INITIAL,
                OperationId::new(),
                |_, _| Ok(()),
            )
            .unwrap_err()
            .command_code(),
        Some("project_recovery_required")
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
    assert_eq!(state.resource_lifecycle_entry_count(), 0);
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
fn canonical_graph_insert_lifecycle_publication_preserves_fresh_revision() {
    let (state, root) = state_with_project_path("graph-insert-lifecycle-wire-revision");
    let path = GraphResourcePath::new("events/Inserted.yssbi-event").unwrap();
    let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        path.as_str().into(),
    ));
    let operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(0x803));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id,
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: [key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };

    let result = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::InsertGraph {
                path: path.clone(),
                resource: GraphResourceDocument::new("Inserted", GraphDocumentKind::Event),
            },
        )
        .unwrap();

    let wire = serde_json::to_value(&result).unwrap();
    let delta = &wire["deltas"][0];
    assert_eq!(delta["payload"]["kind"], "resource_lifecycle");
    assert_eq!(delta["fromRevision"], 0);
    assert_eq!(delta["toRevision"], 0);
    assert_eq!(delta["payload"]["patch"]["after"]["revision"], 0);
    assert_eq!(
        state.get_data().unwrap().graphs[&path].document.revision,
        GraphRevision::INITIAL
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonical_graph_unload_lifecycle_preserves_authoritative_revision() {
    let (state, root) = state_with_project_path("graph-unload-lifecycle-wire-revision");
    let path = GraphResourcePath::new("events/Unloaded.yssbi-event").unwrap();
    let mut resource = GraphResourceDocument::new("Unloaded", GraphDocumentKind::Event);
    resource.document.revision = GraphRevision::new(4);
    state.insert_graph(path.clone(), resource).unwrap();
    let key = ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        path.as_str().into(),
    ));
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![key.clone()],
        expected_revisions: [(key, GraphRevision::new(4))].into_iter().collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };

    let result = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::UnloadGraph { path: path.clone() },
        )
        .unwrap();
    let delta = &result.deltas[0];

    assert_eq!(delta.from_revision, GraphRevision::new(4));
    assert_eq!(delta.to_revision, GraphRevision::new(4));
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
        &delta.payload
    else {
        panic!("expected resource lifecycle delta");
    };
    assert_eq!(
        lifecycle.after.as_ref().unwrap().revision,
        GraphRevision::new(4)
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(4)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn normalized_resource_lifecycle_routes_every_insert_through_project_state() {
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
    state.unload_graph_resource(&created).unwrap();

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
            crate::data_contract::DataType::Int64,
            crate::data_contract::DataValue::Int64(9),
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
            crate::data_contract::DataType::Int64,
            crate::data_contract::DataValue::Int64(1),
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
                crate::data_contract::DataType::Int64,
                crate::data_contract::DataValue::Int64(1),
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
