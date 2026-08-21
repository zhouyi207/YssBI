use super::*;

#[test]
fn graph_mutation_rejects_stale_caller_project() {
    let first_root = std::env::temp_dir().join(format!(
        "yssbi-graph-mutation-stale-first-{}",
        uuid::Uuid::new_v4()
    ));
    let other_root = std::env::temp_dir().join(format!(
        "yssbi-graph-mutation-stale-other-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&other_root).unwrap();
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        first_root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let stale_id = state.capture_project_session().unwrap().instance_id;
    state.activate_project_fixture(
        other_root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before = state.history_status();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();
    let mut events = Vec::new();

    let result = mutate_graph_document_with_emitter(
        &state,
        stale_id,
        graph_path.as_str().to_string(),
        "en-US",
        resource_bound_graph_mutation_request(&graph_path),
        |event| events.push(event),
    );

    assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    assert!(events.is_empty());
    let _ = std::fs::remove_dir_all(first_root);
    let _ = std::fs::remove_dir_all(other_root);
}

#[test]
fn graph_mutation_rejects_project_replacement_before_finalize() {
    let first_root = std::env::temp_dir().join(format!(
        "yssbi-graph-mutation-race-first-{}",
        uuid::Uuid::new_v4()
    ));
    let other_root = std::env::temp_dir().join(format!(
        "yssbi-graph-mutation-race-other-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&other_root).unwrap();
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(
        first_root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
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
    let worker_graph_path = graph_path.clone();
    let worker = std::thread::spawn(move || {
        mutate_graph_document_with_emitter(
            &worker_state,
            project_instance_id,
            worker_graph_path.as_str().to_string(),
            "en-US",
            graph_mutation_request(&worker_graph_path),
            |event| worker_events.lock().unwrap().push(event),
        )
    });

    rendezvous.wait();
    state.activate_project_fixture(
        other_root.to_string_lossy().into_owned(),
        graph_project(&graph_path),
    );
    let data_before_release = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before_release = state.history_status();
    let revisions_before_release = state.revision_state_for_test();
    let publication_before_release = state.publication_state_for_test();
    rendezvous.wait();
    let result = worker.join().unwrap();

    assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
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
    assert!(events.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(first_root);
    let _ = std::fs::remove_dir_all(other_root);
}

#[test]
fn malformed_create_node_body_maps_to_catalog_descriptor_invalid() {
    let raw = serde_json::json!({
        "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
        "baseRevision": 0,
        "operationId": "00000000-0000-0000-0000-000000000777",
        "payload": {
            "type": "createNode",
            "payload": {
                "descriptor": {
                    "kind": "resourceBound",
                    "nodeTypeId": "yssbi.project.function.call",
                    "resourcePath": "functions/Helper.yssbi-function",
                    "resourceRevision": 0,
                    "createArgs": { "kind": "function" }
                },
                "position": { "x": 1.0, "y": 2.0 },
                "userLabel": null,
                "parameters": { "target": "functions/Injected.yssbi-function" }
            }
        }
    });

    let malformed_descriptor = serde_json::json!({
        "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
        "baseRevision": 0,
        "operationId": "00000000-0000-0000-0000-000000000779",
        "payload": {
            "type": "createNode",
            "payload": {
                "descriptor": {
                    "kind": "resourceBound",
                    "nodeTypeId": "yssbi.project.function.call",
                    "resourcePath": "functions/Helper.yssbi-function",
                    "resourceRevision": "stale",
                    "createArgs": { "kind": "function" }
                },
                "position": { "x": 1.0, "y": 2.0 },
                "userLabel": null
            }
        }
    });

    for request in [raw, malformed_descriptor] {
        let error = parse_editor_mutation_request(request).unwrap_err();
        assert_eq!(error.code(), "catalog_descriptor_invalid");
    }
}

#[test]
fn non_descriptor_request_shape_errors_are_not_catalog_errors() {
    let valid_static_descriptor = serde_json::json!({
        "kind": "static",
        "nodeTypeId": "yssbi.constant.int64"
    });
    let cases = [
        serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000801",
            "payload": { "type": "moveNodes", "payload": { "positions": "invalid" } }
        }),
        serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000802",
            "payload": {
                "type": "connect",
                "payload": { "output": { "kind": "declared" }, "input": null, "order": null }
            }
        }),
        serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "not-an-operation-id",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": valid_static_descriptor.clone(),
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": null
                }
            }
        }),
        serde_json::json!({
            "resource": { "kind": "graph", "key": 7 },
            "baseRevision": "zero",
            "operationId": "00000000-0000-0000-0000-000000000803",
            "payload": { "type": "deleteNode", "payload": { "nodeId": "invalid" } }
        }),
        serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000804",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": valid_static_descriptor.clone(),
                    "position": { "x": "left", "y": 2.0 },
                    "userLabel": null
                }
            }
        }),
        serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000805",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": valid_static_descriptor,
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": 42
                }
            }
        }),
    ];

    for raw in cases {
        let error = parse_editor_mutation_request(raw).unwrap_err();
        assert_eq!(error.code(), "invalid_editor_mutation");
    }
}

#[test]
fn injected_create_node_command_has_zero_authoritative_effects() {
    let state = ProjectState::new();
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    state
        .insert_graph(
            graph_path.clone(),
            GraphResourceDocument::new("Main", GraphDocumentKind::Event),
        )
        .unwrap();
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before = state.history_status();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();
    let raw = serde_json::json!({
        "resource": { "kind": "graph", "key": graph_path.as_str() },
        "baseRevision": 0,
        "operationId": "00000000-0000-0000-0000-000000000778",
        "payload": {
            "type": "createNode",
            "payload": {
                "descriptor": {
                    "kind": "resourceBound",
                    "nodeTypeId": "yssbi.project.function.call",
                    "resourcePath": "functions/Helper.yssbi-function",
                    "resourceRevision": 0,
                    "createArgs": { "kind": "function" }
                },
                "position": { "x": 1.0, "y": 2.0 },
                "userLabel": null,
                "parameters": { "target": "functions/Injected.yssbi-function" }
            }
        }
    });
    let mut events = Vec::new();

    let error = mutate_graph_document_with_emitter(
        &state,
        ProjectInstanceId::from_existing(state.project_instance_id()),
        graph_path.as_str().to_string(),
        "en-US",
        raw,
        |event| events.push(event),
    )
    .unwrap_err();

    assert_eq!(error.code(), "catalog_descriptor_invalid");
    assert!(events.is_empty());
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
}

#[test]
fn catalog_mutation_conflicts_preserve_stable_command_error_codes() {
    for (conflict, expected) in [
        (
            crate::node_system::document::MutationConflict::CatalogResourceStale(
                "resource changed".into(),
            ),
            "catalog_resource_stale",
        ),
        (
            crate::node_system::document::MutationConflict::CatalogDescriptorInvalid(
                "descriptor is invalid".into(),
            ),
            "catalog_descriptor_invalid",
        ),
    ] {
        let error = mutation_conflict_to_command_error(conflict, "graph_revision_conflict");
        assert_eq!(error.code(), expected);
    }
}
