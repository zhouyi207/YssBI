use super::*;

#[test]
fn history_commands_reject_stale_project_identity_with_zero_effects() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-history-command-stale-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let stale_id = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let status_before = state.history_status();
    let lengths_before = state.history_lengths_for_test();
    let undo_head_before = state.history_head_id_for_test(true);
    let redo_head_before = state.history_head_id_for_test(false);
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();
    let mut events = Vec::new();

    let status_error = get_project_history_status_from_state(&state, stale_id.clone()).unwrap_err();
    let undo_error = undo_graph_document_with_emitter(
        &state,
        stale_id.clone(),
        "en-US",
        history_request(&graph_path),
        |event| events.push(event),
    )
    .unwrap_err();
    let redo_error = redo_graph_document_with_emitter(
        &state,
        stale_id,
        "en-US",
        history_request(&graph_path),
        |event| events.push(event),
    )
    .unwrap_err();

    for error in [status_error, undo_error, redo_error] {
        assert_eq!(error.code(), "stale_project_lifecycle");
    }
    assert!(events.is_empty());
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.history_status(), status_before);
    assert_eq!(state.history_lengths_for_test(), lengths_before);
    assert_eq!(state.history_head_id_for_test(true), undo_head_before);
    assert_eq!(state.history_head_id_for_test(false), redo_head_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn function_signature_command_rejects_stale_project_identity() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-function-signature-command-stale-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("functions/Compute.yssbi-function").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        function_project(&graph_path),
    );
    let stale_id = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        function_project(&graph_path),
    );
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let mut events = Vec::new();

    let error = update_function_signature_with_emitter(
        &state,
        stale_id,
        graph_path.as_str().to_string(),
        "en-US",
        function_signature_request(&graph_path),
        |event| events.push(event),
    )
    .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert!(events.is_empty());
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn function_signature_command_rejects_project_replacement_before_publication() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-function-signature-command-race-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("functions/Compute.yssbi-function").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        function_project(&graph_path),
    );
    let project_instance_id = state.capture_project_session().unwrap().instance_id;
    let rendezvous = Arc::new(std::sync::Barrier::new(2));
    let hook_rendezvous = Arc::clone(&rendezvous);
    state.set_mutation_publication_test_hook(Arc::new(move || {
        hook_rendezvous.wait();
        hook_rendezvous.wait();
    }));
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let worker_events = Arc::clone(&events);
    let worker_state = state.clone();
    let worker_path = graph_path.clone();
    let worker = std::thread::spawn(move || {
        update_function_signature_with_emitter(
            &worker_state,
            project_instance_id,
            worker_path.as_str().to_string(),
            "en-US",
            function_signature_request(&worker_path),
            |event| worker_events.lock().unwrap().push(event),
        )
    });

    rendezvous.wait();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        function_project(&graph_path),
    );
    let before_data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    rendezvous.wait();
    let error = worker.join().unwrap().unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert!(events.lock().unwrap().is_empty());
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before_data
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hydrate_editor_graph_rejects_stale_project_identity() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-hydrate-editor-graph-stale-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let stale_id = state.capture_project_session().unwrap().instance_id;
    let mut replacement = graph_project(&graph_path);
    replacement.graphs.get_mut(&graph_path).unwrap().name = "Replacement".into();
    let replacement_state = state.clone();
    let replacement_root = root.to_string_lossy().into_owned();
    state.set_projection_test_hook(Arc::new(move || {
        replacement_state.activate_project_fixture(replacement_root.clone(), replacement.clone());
        Ok(())
    }));

    let error =
        hydrate_editor_graph_from_state(&state, stale_id, graph_path.as_str().to_string(), "en-US")
            .unwrap_err();

    assert_eq!(error.code(), "stale_project_lifecycle");
    assert_eq!(
        state.get_data().unwrap().graphs[&graph_path].name,
        "Replacement"
    );
    let _ = std::fs::remove_dir_all(root);
}
