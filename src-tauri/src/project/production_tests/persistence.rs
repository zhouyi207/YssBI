use super::*;

fn computation_settings_request(
    state: &ProjectState,
    expected_revision: u64,
    absolute: f64,
) -> crate::project::ComputationSettingsMutationRequest {
    let mut settings = crate::project::ProjectComputationSettings::default();
    settings.numeric.tolerance.absolute = absolute;
    crate::project::ComputationSettingsMutationRequest {
        project_instance_id: current_project_instance_id(state),
        operation_id: OperationId::new(),
        expected_revision,
        settings,
    }
}

fn computation_settings_snapshot(
    project: &crate::project::fixtures::TempProject,
) -> (
    Vec<u8>,
    crate::project::ProjectComputationSettings,
    u64,
    (String, u64, u64),
) {
    let state = project.state();
    let metadata = std::fs::read(
        state
            .capture_project_session()
            .unwrap()
            .root
            .as_path()
            .join(crate::project::PROJECT_METADATA_FILE),
    )
    .unwrap();
    let current = state.get_computation_settings().unwrap();
    (
        metadata,
        current.settings,
        current.settings_revision,
        state.publication_state_for_test(),
    )
}

#[test]
fn computation_settings_mutation_commits_disk_and_authority_atomically() {
    let project = temp_project_with_empty_graph("computation-settings");
    let state = project.state();
    let before = state.publication_state_for_test();
    let request = computation_settings_request(state, 0, 1e-8);

    let receipt = state
        .update_computation_settings_transaction(request.clone())
        .unwrap();
    let root = state.capture_project_session().unwrap().root;
    let reloaded =
        crate::project::load_project_from_file(root.as_path().to_string_lossy().as_ref()).unwrap();

    assert_eq!(receipt.operation_id, request.operation_id);
    assert_eq!(receipt.project_instance_id, request.project_instance_id);
    assert_eq!(receipt.settings_revision, 1);
    assert_eq!(receipt.publication_revision, before.1 + 1);
    assert_eq!(receipt.settings, request.settings);
    assert_eq!(reloaded.computation_settings, receipt.settings);
    assert_eq!(
        state.get_data().unwrap().computation_settings,
        receipt.settings
    );
    assert_eq!(state.authority_generation_for_test(), before.2 + 1);
}

#[test]
fn stale_computation_settings_revision_changes_nothing() {
    let project = temp_project_with_empty_graph("stale-computation-settings");
    let state = project.state();
    state
        .update_computation_settings_transaction(computation_settings_request(state, 0, 1e-8))
        .unwrap();
    let before = computation_settings_snapshot(&project);

    let error = state
        .update_computation_settings_transaction(computation_settings_request(state, 0, 1e-7))
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(computation_settings_snapshot(&project), before);
}

#[test]
fn computation_settings_disk_failure_preserves_memory_and_manifest() {
    let project = temp_project_with_empty_graph("computation-settings-disk-failure");
    let state = project.state();
    let before = computation_settings_snapshot(&project);
    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::FirstLiveReplacement,
    ));

    let error = state
        .update_computation_settings_transaction(computation_settings_request(state, 0, 1e-8))
        .unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(computation_settings_snapshot(&project), before);
}

#[test]
fn computation_settings_commit_emits_exactly_one_event() {
    let project = temp_project_with_empty_graph("computation-settings-event");
    let request = computation_settings_request(project.state(), 0, 1e-8);
    let mut events = Vec::new();

    let receipt = crate::commands::command_project::settings::update_project_computation_settings_with_emitter(
        project.state(),
        request,
        |event| events.push(event),
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [crate::event::Event::Project(crate::event::EventProject::ComputationSettingsChanged {
            result,
        })] if result == &receipt
    ));
}

#[test]
fn computation_settings_publication_failure_rolls_back_manifest_and_memory() {
    let project = temp_project_with_empty_graph("computation-settings-publication-failure");
    let state = project.state();
    let before = computation_settings_snapshot(&project);
    state.set_computation_settings_publication_test_hook(std::sync::Arc::new(|| {
        panic!("injected computation settings publication failure")
    }));

    let error = state
        .update_computation_settings_transaction(computation_settings_request(state, 0, 1e-8))
        .unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(computation_settings_snapshot(&project), before);
}
