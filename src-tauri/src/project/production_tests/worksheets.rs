use super::*;

#[test]
fn worksheet_removal_environment_failure_never_changes_filesystem_target() {
    let (remove_state, remove_root) =
        state_with_project_path("worksheet-remove-environment-failure");
    let invalid_database = remove_root.join("database/invalid.duckdb");
    std::fs::create_dir_all(invalid_database.parent().unwrap()).unwrap();
    std::fs::write(&invalid_database, b"not a DuckDB database").unwrap();
    let (worksheet_path, worksheet) = fixtures::worksheet("Preserved", "database");
    remove_state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet_path.clone(), worksheet.clone());
    remove_state.initialize_worksheet_revision_for_test(&worksheet_path);
    fixtures::write_worksheet(&remove_root, &worksheet_path, &worksheet).unwrap();
    insert_uncached_duckdb_declaration(&remove_state, "database/invalid.duckdb");
    let worksheet_file = remove_root.join(worksheet_path.relative_path());
    let worksheet_before = std::fs::read(&worksheet_file).unwrap();

    let session = remove_state.capture_project_session().unwrap();
    remove_state.set_project_filesystem_rollback_fault(true);
    let remove_result = remove_state.remove_worksheet_resource_transaction(
        &session.instance_id,
        &worksheet_path,
        ResourceRevision::INITIAL,
        OperationId::new(),
    );
    remove_state.set_project_filesystem_rollback_fault(false);

    assert!(remove_result.is_err());
    assert_eq!(std::fs::read(&worksheet_file).unwrap(), worksheet_before);
    assert!(
        remove_state
            .project_data
            .read()
            .unwrap()
            .worksheets
            .contains_key(&worksheet_path)
    );
    std::fs::remove_dir_all(remove_root).unwrap();
}

#[test]
fn worksheet_revision_conflict_has_zero_authoritative_effects() {
    let (state, root) = state_with_project_path("worksheet-revision-conflict");
    let (worksheet_path, worksheet) = fixtures::worksheet("Original", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet_path.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet_path);
    let key = ResourceKey::Worksheet(crate::node_system::document::WorksheetResourceKey(
        worksheet_path.as_str().into(),
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
    concurrent.chart_type = "line".into();
    state
        .apply_resource_document_patch(
            &current,
            ResourceDocumentPatch::UpsertWorksheet {
                path: worksheet_path.clone(),
                document: concurrent,
            },
        )
        .unwrap();
    let mut stale_document = worksheet.clone();
    stale_document.chart_type = "area".into();

    let error = state
        .apply_resource_document_patch(
            &stale,
            ResourceDocumentPatch::UpsertWorksheet {
                path: worksheet_path.clone(),
                document: stale_document,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        state.get_data().unwrap().worksheets[&worksheet_path].chart_type,
        "line"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_upsert_rejects_portable_path_collision_without_effects() {
    let (state, root) = state_with_project_path("worksheet-portable-collision");
    let (existing_path, existing) = fixtures::worksheet("Straße", "database");
    let (colliding_path, colliding) = fixtures::worksheet("STRASSE", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(existing_path.clone(), existing.clone());
    state.initialize_worksheet_revision_for_test(&existing_path);
    fixtures::write_worksheet(&root, &existing_path, &existing).unwrap();
    let existing_file = root.join(existing_path.relative_path());
    let colliding_file = root.join(colliding_path.relative_path());
    let existing_bytes = std::fs::read(&existing_file).unwrap();
    let authority_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let revisions_before = state.revision_state_for_test();
    let generation_before = state.authority_generation_for_test();

    let colliding_key = ResourceKey::Worksheet(crate::node_system::document::WorksheetResourceKey(
        colliding_path.as_str().into(),
    ));
    let direct_context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: [colliding_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let error = state
        .apply_resource_document_patch(
            &direct_context,
            ResourceDocumentPatch::UpsertWorksheet {
                path: colliding_path.clone(),
                document: colliding,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "resource_revision_conflict");

    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        authority_before
    );
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.authority_generation_for_test(), generation_before);
    assert_eq!(std::fs::read(existing_file).unwrap(), existing_bytes);
    assert!(!colliding_file.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_worksheet_save_is_rejected_without_disk_or_authoritative_effects() {
    let (state, root) = state_with_project_path("worksheet-stale-save");
    let (worksheet_path, worksheet) = fixtures::worksheet("Original", "database");
    state
        .project_data
        .write()
        .unwrap()
        .worksheets
        .insert(worksheet_path.clone(), worksheet.clone());
    state.initialize_worksheet_revision_for_test(&worksheet_path);
    fixtures::write_worksheet(&root, &worksheet_path, &worksheet).unwrap();
    let mut current = worksheet.clone();
    current.chart_type = "line".into();
    let session = state.capture_project_session().unwrap();
    state
        .save_worksheet_document(
            &session.instance_id,
            &worksheet_path,
            ResourceRevision::INITIAL,
            OperationId::new(),
            current,
        )
        .unwrap();
    let mut stale = worksheet.clone();
    stale.chart_type = "area".into();

    let error = state
        .save_worksheet_document(
            &session.instance_id,
            &worksheet_path,
            ResourceRevision::INITIAL,
            OperationId::new(),
            stale,
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        state.get_data().unwrap().worksheets[&worksheet_path].chart_type,
        "line"
    );
    assert_eq!(
        crate::project::load_worksheet_from_file(&root, &worksheet_path)
            .unwrap()
            .chart_type,
        "line"
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
    let (worksheet_path, worksheet) = fixtures::worksheet("Authoritative", "database");

    state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::UpsertWorksheet {
                path: worksheet_path.clone(),
                document: worksheet.clone(),
            },
        )
        .unwrap();

    let data = state.get_data().unwrap();
    assert_eq!(data.worksheets[&worksheet_path], worksheet);
    assert_eq!(
        data.variables[&concurrent_id].name,
        "Concurrent Worksheet Variable"
    );
    std::fs::remove_dir_all(root).unwrap();
}
