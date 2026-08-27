use super::*;

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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        super::project_state::VariableRevisionEntry::deleted(ResourceRevision::new(7)),
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
                    ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        specialized_head.history_id = crate::project::HistoryEntryId::new();
        specialized_head.persistence = policy;
        match policy {
            crate::node_system::document::HistoryPersistencePolicy::DurableVariableEffects => {
                specialized_head.variable_effect_snapshots = Some(Default::default());
                specialized_head.resource_lifecycle = None;
                specialized_head.resource_move = None;
            }
            crate::node_system::document::HistoryPersistencePolicy::DurableResourceMove => {
                let ResourceKey::Graph(path) = resource.clone() else {
                    unreachable!();
                };
                specialized_head.variable_effect_snapshots = None;
                specialized_head.resource_lifecycle = None;
                specialized_head.resource_move =
                    Some(crate::node_system::document::ResourceMoveHistoryPatch {
                        from: path.as_str().into(),
                        to: path.as_str().into(),
                        kind: crate::node_system::document::ResourceLifecycleKind::Event,
                        payload: crate::node_system::document::ResourceMoveHistoryPayload::Graph {
                            persisted_move_payload: serde_json::Value::Null,
                        },
                    });
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
    specialized_head.history_id = crate::project::HistoryEntryId::new();
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
    let event_key = event_path.clone();
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
                ResourceRevision::INITIAL,
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        .revision = ResourceRevision::new(7);
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
                id: crate::graph_document::FunctionParameterId::new("request_id"),
                name: "Request ID".into(),
                type_name: "string".into(),
            },
            crate::node_system::document::FunctionParameter {
                id: crate::graph_document::FunctionParameterId::new("payload"),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
        ResourceRevision::from_graph_revision(GraphRevision::new(2))
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
    assert_eq!(
        undo.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(1))
    );
    assert_eq!(
        undo.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(2))
    );
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
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
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
    assert_eq!(
        redo.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(2))
    );
    assert_eq!(
        redo.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(3))
    );
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
        redo.deltas[0].to_revision.to_graph_revision()
    );
    assert_eq!(redo_function.signature, signature);
    assert_eq!(
        redo_function
            .signature
            .parameters
            .iter()
            .map(|parameter| parameter.id.as_str())
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
    let loaded_key = loaded_path.clone();
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
        super::project_state::VariableRevisionEntry::deleted(ResourceRevision::INITIAL),
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
                ResourceRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    None,
                    Some(serde_json::to_value(&created).unwrap()),
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                updated_key.clone(),
                ResourceRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(&updated_before).unwrap()),
                    Some(serde_json::to_value(&updated_after).unwrap()),
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                removed_key.clone(),
                ResourceRevision::INITIAL,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(&removed).unwrap()),
                    None,
                ),
            ),
            crate::node_system::document::ResourcePatch::variable(
                global_key.clone(),
                ResourceRevision::INITIAL,
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
                caused_by: Some(undo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Graph(
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::RemoveNode {
                        node: loaded_node.clone(),
                    }]),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(created_key.clone()),
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
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
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
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
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
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
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
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
    assert_eq!(
        created_revision.revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(2))
    );
    assert!(!created_revision.is_present());
    for revision in [updated_revision, removed_revision, global_revision] {
        assert_eq!(
            revision.revision,
            ResourceRevision::from_graph_revision(GraphRevision::new(2))
        );
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
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
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
                from_revision: ResourceRevision::new(2),
                to_revision: ResourceRevision::new(3),
                caused_by: Some(redo_operation),
                payload: crate::node_system::document::ResourceDocumentPatch::Graph(
                    GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                        node: loaded_node.clone(),
                    }]),
                ),
            },
            crate::node_system::document::ResourceDeltaEvent {
                resource: ResourceKey::Variable(created_key.clone()),
                from_revision: ResourceRevision::new(2),
                to_revision: ResourceRevision::new(3),
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
                from_revision: ResourceRevision::new(2),
                to_revision: ResourceRevision::new(3),
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
                from_revision: ResourceRevision::new(2),
                to_revision: ResourceRevision::new(3),
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
                from_revision: ResourceRevision::new(2),
                to_revision: ResourceRevision::new(3),
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
        assert_eq!(
            revision.revision,
            ResourceRevision::from_graph_revision(GraphRevision::new(3))
        );
        assert!(revision.is_present());
    }
    assert_eq!(
        removed_revision.revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(3))
    );
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
    let resource = ResourceKey::Graph(graph_path.clone());
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

    let undo = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
    assert_eq!(
        undo.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(1))
    );
    assert_eq!(
        undo.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(2))
    );
    assert_eq!(undo_disk.revision, undo.deltas[0].to_revision);
    assert_eq!(
        state.revision_state_for_test().0.get(&graph_path).copied(),
        Some(undo_disk.revision.to_graph_revision())
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
        Some(redo_disk.revision.to_graph_revision())
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
                    ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
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
                ResourceRevision::from_graph_revision(GraphRevision::new(7)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        ),
        Err(MutationConflict::RecoveryRequired(_))
    ));
    std::fs::remove_dir_all(root).unwrap();
}
