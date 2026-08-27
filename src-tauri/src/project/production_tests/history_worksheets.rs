use super::*;

fn history_request(
    resource: ResourceKey,
    revision: ResourceRevision,
) -> MutationRequest<HistoryMutation> {
    MutationRequest::new(resource, revision, OperationId::new(), HistoryMutation {})
}

fn worksheet_history_key(path: &WorksheetResourcePath) -> ResourceKey {
    ResourceKey::Worksheet(crate::node_system::document::WorksheetResourceKey(
        path.as_str().into(),
    ))
}

fn worksheet_revision(state: &ProjectState, path: &WorksheetResourcePath) -> ResourceRevision {
    state.worksheet_revisions.read().unwrap()[path]
}

fn apply_worksheet_history(
    state: &ProjectState,
    project: &ProjectInstanceId,
    path: &WorksheetResourcePath,
    undo: bool,
) -> crate::event::ResourceMutationResultDto {
    let request = history_request(worksheet_history_key(path), worksheet_revision(state, path));
    if undo {
        state
            .undo_last_transaction_observed(project, "en-US", request, |_| {})
            .unwrap()
    } else {
        state
            .redo_last_transaction_observed(project, "en-US", request, |_| {})
            .unwrap()
    }
}

fn assert_strict_resource_mutation_wire_coherent(result: &crate::event::ResourceMutationResultDto) {
    let wire = serde_json::to_value(result).unwrap();
    assert_eq!(
        serde_json::from_value::<crate::event::ResourceMutationResultDto>(wire).unwrap(),
        *result
    );
    for delta in &result.deltas {
        let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &delta.payload
        else {
            continue;
        };
        if let Some(before) = &lifecycle.before {
            assert_eq!(
                before.revision, delta.from_revision,
                "lifecycle before revision must match the delta envelope"
            );
        }
        if let Some(after) = &lifecycle.after {
            assert_eq!(
                after.revision, delta.to_revision,
                "lifecycle after revision must match the delta envelope"
            );
        }
    }
}

fn create_worksheet_history_fixture(
    label: &str,
) -> (
    ProjectState,
    std::path::PathBuf,
    ProjectInstanceId,
    WorksheetResourcePath,
) {
    let (state, root) = state_with_project_path(label);
    let project = state.capture_project_session().unwrap().instance_id;
    let name = crate::project::ResourceName::parse("Durable History").unwrap();
    state
        .create_worksheet_resource_transaction(
            &project,
            &name,
            Some("database".into()),
            OperationId::new(),
        )
        .unwrap();
    let path = WorksheetResourcePath::from_name(&name);
    (state, root, project, path)
}

#[test]
fn worksheet_create_delete_save_and_rename_undo_redo_are_durable() {
    let (state, root, project, source) =
        create_worksheet_history_fixture("worksheet-history-durable");
    assert_eq!(state.project_instance_id(), project.as_str());
    assert!(state.history_status().can_undo);
    assert!(root.join(source.relative_path()).is_file());

    let undo_create = apply_worksheet_history(&state, &project, &source, true);
    assert_eq!(undo_create.project_instance_id, project.as_str());
    assert!(!root.join(source.relative_path()).exists());
    assert!(!state.get_data().unwrap().worksheets.contains_key(&source));
    assert!(undo_create.history.can_redo);
    let redo_create = apply_worksheet_history(&state, &project, &source, false);
    assert_eq!(redo_create.project_instance_id, project.as_str());
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
        redo_create_lifecycle,
    ) = &redo_create.deltas[0].payload
    else {
        panic!("create redo must publish a lifecycle delta");
    };
    assert_eq!(
        redo_create_lifecycle.after.as_ref().unwrap().revision,
        redo_create.deltas[0].to_revision
    );
    assert_strict_resource_mutation_wire_coherent(&redo_create);
    assert!(root.join(source.relative_path()).is_file());
    assert!(state.get_data().unwrap().worksheets.contains_key(&source));

    let revision = worksheet_revision(&state, &source);
    let mut saved = state.get_data().unwrap().worksheets[&source].clone();
    saved.chart_type = "line".into();
    state
        .save_worksheet_document(&project, &source, revision, OperationId::new(), saved)
        .unwrap();
    assert_eq!(
        crate::project::load_worksheet_from_file(&root, &source)
            .unwrap()
            .chart_type,
        "line"
    );
    apply_worksheet_history(&state, &project, &source, true);
    assert_eq!(
        crate::project::load_worksheet_from_file(&root, &source)
            .unwrap()
            .chart_type,
        "histogram"
    );
    apply_worksheet_history(&state, &project, &source, false);
    assert_eq!(
        state.get_data().unwrap().worksheets[&source].chart_type,
        "line"
    );

    let target_name = crate::project::ResourceName::parse("Renamed History").unwrap();
    let target = WorksheetResourcePath::from_name(&target_name);
    state
        .rename_worksheet_resource_transaction(
            &project,
            &source,
            worksheet_revision(&state, &source),
            &target_name,
            1,
            OperationId::new(),
        )
        .unwrap();
    assert!(!root.join(source.relative_path()).exists());
    assert!(root.join(target.relative_path()).is_file());
    let undo_move = apply_worksheet_history(&state, &project, &target, true);
    assert_eq!(undo_move.moves[0].from, target.as_str());
    assert_eq!(undo_move.moves[0].to, source.as_str());
    assert!(root.join(source.relative_path()).is_file());
    let redo_move = apply_worksheet_history(&state, &project, &source, false);
    assert_eq!(redo_move.moves[0].from, source.as_str());
    assert_eq!(redo_move.moves[0].to, target.as_str());
    assert!(root.join(target.relative_path()).is_file());

    state
        .remove_worksheet_resource_transaction(
            &project,
            &target,
            worksheet_revision(&state, &target),
            OperationId::new(),
        )
        .unwrap();
    assert!(!root.join(target.relative_path()).exists());
    let undo_delete = apply_worksheet_history(&state, &project, &target, true);
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
        undo_delete_lifecycle,
    ) = &undo_delete.deltas[0].payload
    else {
        panic!("delete undo must publish a lifecycle delta");
    };
    assert_eq!(
        undo_delete_lifecycle.after.as_ref().unwrap().revision,
        undo_delete.deltas[0].to_revision
    );
    assert_strict_resource_mutation_wire_coherent(&undo_delete);
    assert!(root.join(target.relative_path()).is_file());
    assert!(state.get_data().unwrap().worksheets.contains_key(&target));
    let redo_delete = apply_worksheet_history(&state, &project, &target, false);
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
        redo_delete_lifecycle,
    ) = &redo_delete.deltas[0].payload
    else {
        panic!("delete redo must publish a lifecycle delta");
    };
    assert_eq!(
        redo_delete_lifecycle.before.as_ref().unwrap().revision,
        redo_delete.deltas[0].from_revision
    );
    assert_strict_resource_mutation_wire_coherent(&redo_delete);
    assert!(!root.join(target.relative_path()).exists());
    assert!(!state.get_data().unwrap().worksheets.contains_key(&target));
    assert_eq!(state.project_instance_id(), project.as_str());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_history_rejects_stale_project_before_filesystem_commit() {
    let (state, root, project, path) =
        create_worksheet_history_fixture("worksheet-history-stale-project");
    let file_before = std::fs::read(root.join(path.relative_path())).unwrap();
    state.set_history_after_preparation_test_hook(publish_empty_replacement_hook(&state, &root));

    let error = state
        .undo_last_transaction_observed(
            &project,
            "en-US",
            history_request(
                worksheet_history_key(&path),
                worksheet_revision(&state, &path),
            ),
            |_| {},
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::StaleProjectLifecycle(_)));
    assert_eq!(
        std::fs::read(root.join(path.relative_path())).unwrap(),
        file_before
    );
    assert_empty_replacement_authority(&state, &project);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_history_filesystem_failure_has_zero_authoritative_effects() {
    let (state, root, project, path) =
        create_worksheet_history_fixture("worksheet-history-filesystem-failure");
    let revision = worksheet_revision(&state, &path);
    let mut saved = state.get_data().unwrap().worksheets[&path].clone();
    saved.chart_type = "line".into();
    state
        .save_worksheet_document(&project, &path, revision, OperationId::new(), saved)
        .unwrap();
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();
    let history_before = state.history_status();
    let file_before = std::fs::read(root.join(path.relative_path())).unwrap();
    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
    ));

    let error = state
        .undo_last_transaction_observed(
            &project,
            "en-US",
            history_request(
                worksheet_history_key(&path),
                worksheet_revision(&state, &path),
            ),
            |_| {},
        )
        .unwrap_err();
    state.set_project_filesystem_fault(None);

    assert!(matches!(error, MutationConflict::History(_)));
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    assert_eq!(state.history_status(), history_before);
    assert_eq!(
        std::fs::read(root.join(path.relative_path())).unwrap(),
        file_before
    );
    assert_eq!(state.project_instance_id(), project.as_str());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worksheet_history_publication_failure_enters_authoritative_recovery() {
    let (state, root, project, path) =
        create_worksheet_history_fixture("worksheet-history-publication-recovery");
    let hook_state = state.clone();
    let hook_path = path.clone();
    state.set_history_after_disk_commit_test_hook(std::sync::Arc::new(move || {
        hook_state
            .worksheet_revisions
            .write()
            .unwrap()
            .insert(hook_path.clone(), ResourceRevision::new(99));
    }));
    state.set_project_filesystem_rollback_fault(true);

    let error = state
        .undo_last_transaction_observed(
            &project,
            "en-US",
            history_request(
                worksheet_history_key(&path),
                worksheet_revision(&state, &path),
            ),
            |_| {},
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::RecoveryRequired(_)));
    assert_eq!(
        state.get_data().unwrap_err().code(),
        "project_recovery_required"
    );
    assert_eq!(state.project_instance_id(), project.as_str());
    state.set_project_filesystem_rollback_fault(false);
    std::fs::remove_dir_all(root).unwrap();
}
