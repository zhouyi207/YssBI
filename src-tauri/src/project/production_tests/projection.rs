use super::*;

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
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
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
    updated.data_value = crate::data_contract::DataValue::Int64(2);
    let blocked = state.commit_variable_effects(
        &session_id,
        vec![crate::node_system::runtime::VariableWriteEffect {
            resource: resource_id,
            expected_revision: ResourceRevision::new(1),
            before: updated,
            after: crate::data_contract::DataValue::Int64(3),
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
                name: id.into(),
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
                name: id.into(),
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
            name: "Main".into(),
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
