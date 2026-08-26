use super::*;

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
                    ResourceKey::Graph(path.clone()),
                    ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
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
    let retained_document_path = retained.clone();
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
    let document_path = graph_path.clone();
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        ResourceRevision::from_graph_revision(GraphRevision::new(1))
    );
    drop(prepared);

    let graph_file = root.join(graph_path.as_str());
    std::fs::remove_file(&graph_file).unwrap();
    let missing_error = match state.prepare_history_for_test(
        true,
        MutationRequest::new(
            ResourceKey::Graph(document_path.clone()),
            ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
            ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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

#[test]
fn mixed_residency_graph_history_is_atomic_and_preserves_residency() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-mixed-residency-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let loaded_path = GraphResourcePath::new("events/LoadedHistory.yssbi-event").unwrap();
    let unloaded_path = GraphResourcePath::new("events/UnloadedHistory.yssbi-event").unwrap();
    let loaded_key = loaded_path.clone();
    let unloaded_key = unloaded_path.clone();
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
                    ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        delta.from_revision == ResourceRevision::from_graph_revision(GraphRevision::new(1))
            && delta.to_revision == ResourceRevision::from_graph_revision(GraphRevision::new(2))
    }));

    let redo = state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(loaded_key),
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
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
        delta.from_revision == ResourceRevision::from_graph_revision(GraphRevision::new(2))
            && delta.to_revision == ResourceRevision::from_graph_revision(GraphRevision::new(3))
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
    let graph_key = graph_path.clone();
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

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(graph_key),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        delta.from_revision == ResourceRevision::from_graph_revision(GraphRevision::new(1))
            && delta.to_revision == ResourceRevision::from_graph_revision(GraphRevision::new(2))
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
fn projection_failure_before_commit_has_zero_authoritative_effects() {
    let state = state_with_empty_graph();
    let before_document =
        serde_json::to_value(&state.get_data().unwrap().graphs[&graph_path()].document).unwrap();
    let before_revisions = state.revision_state_for_test();
    let before_history = state.history_status();
    let before_publication = state.publication_state_for_test();
    let before_projection =
        serde_json::to_value(state.graph_projection(&graph_path(), "en-US").unwrap()).unwrap();
    let fail_projection = std::sync::atomic::AtomicBool::new(true);
    state.set_projection_test_hook(std::sync::Arc::new(move || {
        if fail_projection.swap(false, std::sync::atomic::Ordering::AcqRel) {
            return Err("injected projection failure".into());
        }
        Ok(())
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
    assert!(observed.is_empty());
    assert_eq!(
        serde_json::to_value(&state.get_data().unwrap().graphs[&graph_path()].document).unwrap(),
        before_document
    );
    assert_eq!(state.revision_state_for_test(), before_revisions);
    assert_eq!(state.history_status(), before_history);
    assert_eq!(state.publication_state_for_test(), before_publication);
    assert_eq!(
        serde_json::to_value(state.graph_projection(&graph_path(), "en-US").unwrap()).unwrap(),
        before_projection
    );
}
