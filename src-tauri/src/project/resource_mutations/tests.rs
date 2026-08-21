use super::ResourceMutationTestPoint;
use crate::graph::value::{DataType, DataValue};
use crate::node_system::document::{
    DocumentNode, NodeId, OperationId, ParameterValues, ResourceRevision,
};
use crate::node_system::protocol::NodeTypeId;
use crate::project::{
    GraphDocument, GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData,
    ProjectFilesystemError, ProjectState, ResourceNameError,
};
use crate::variable::{VariableId, VariableInstance, VariableScope};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

struct TestProject {
    root: std::path::PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "yssbi-resource-mutation-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn state(&self, data: ProjectData) -> ProjectState {
        crate::project::fixtures::write_project(&data, self.root.to_string_lossy().as_ref())
            .unwrap();
        for graph_path in data.graphs.keys() {
            crate::project::fixtures::write_graph(
                &data,
                self.root.to_string_lossy().as_ref(),
                graph_path,
            )
            .unwrap();
        }
        let state = ProjectState::new();
        state.activate_project_fixture(self.root.to_string_lossy().into_owned(), data);
        state
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn graph_path(path: &str) -> GraphResourcePath {
    GraphResourcePath::new(path).unwrap()
}

fn function_data(
    path: &GraphResourcePath,
    owner_revision: ResourceRevision,
    embedded_revision: ResourceRevision,
) -> ProjectData {
    let mut resource = GraphResourceDocument::new("Source", GraphDocumentKind::Function);
    resource.document.revision = owner_revision;
    resource.function.as_mut().unwrap().revision = embedded_revision;
    let mut data = ProjectData::new();
    data.graphs.insert(path.clone(), resource);
    data
}

fn function_files(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    let directory = root.join("functions");
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn duplicate_boundary_snapshot(
    state: &ProjectState,
    root: &std::path::Path,
) -> (
    serde_json::Value,
    HashMap<GraphResourcePath, ResourceRevision>,
    (u64, u64),
    BTreeMap<String, Vec<u8>>,
) {
    let data = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let graph_revisions = state.graph_revisions.read().unwrap().clone();
    let publication = state.mutation_publication.lock().unwrap();
    let publication_state = (
        publication.resource_revision,
        publication.authority_generation(),
    );
    drop(publication);
    (
        data,
        graph_revisions,
        publication_state,
        function_files(root),
    )
}

fn assert_duplicate_revision_conflict_without_effects(
    state: &ProjectState,
    root: &std::path::Path,
    source: &GraphResourcePath,
    expected_revision: ResourceRevision,
) {
    let session = state.capture_project_session().unwrap();
    let operation_id = OperationId::new();
    let before = duplicate_boundary_snapshot(state, root);
    for _ in 0..2 {
        let error = state
            .duplicate_graph_resource_transaction(
                &session.instance_id,
                source,
                expected_revision,
                operation_id,
            )
            .unwrap_err();
        assert_eq!(error.code(), "resource_revision_conflict", "{error}");
        assert_eq!(duplicate_boundary_snapshot(state, root), before);
    }
}

fn rewrite_persisted_function_revisions(
    root: &std::path::Path,
    source: &GraphResourcePath,
    owner_revision: ResourceRevision,
    graph_revision: ResourceRevision,
    embedded_revision: ResourceRevision,
) {
    let path = root.join(source.as_str());
    let mut document: GraphDocument =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document.revision = owner_revision;
    document.document.revision = graph_revision;
    document.function.as_mut().unwrap().revision = embedded_revision;
    std::fs::write(path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
}

#[test]
fn graph_rename_preserves_case_only_target_without_suffixing() {
    let source = graph_path("events/Sales.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
    );
    let project = TestProject::new("case-only-rename-allocation");
    let state = project.state(data);

    let renamed = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "sales")
        .unwrap();

    assert_eq!(renamed.path.as_str(), "events/sales.yssbi-event");
}

#[test]
fn graph_rename_rejects_exact_portable_conflict_without_suffixing() {
    let source = graph_path("events/Sales.yssbi-event");
    let existing = graph_path("events/Report.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
    );
    data.graphs.insert(
        existing.clone(),
        GraphResourceDocument::new("Report", GraphDocumentKind::Event),
    );
    let project = TestProject::new("rename-portable-conflict");
    let state = project.state(data);
    let before = serde_json::to_value(state.get_data().unwrap()).unwrap();

    let error = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "report")
        .unwrap_err();

    assert_eq!(error.code(), "resource_name_conflict");
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before
    );
    assert!(project.root.join(source.as_str()).is_file());
    assert!(project.root.join(existing.as_str()).is_file());
    assert!(!project.root.join("events/report 1.yssbi-event").exists());
}

#[test]
fn graph_rename_still_rejects_invalid_resource_name() {
    let source = graph_path("events/Sales.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Sales", GraphDocumentKind::Event),
    );
    let project = TestProject::new("invalid-rename-name");
    let state = project.state(data);

    let error = state
        .rename_graph_resource_fixture(&state.project_instance_id(), &source, "Sales/Report")
        .unwrap_err();

    assert_eq!(
        error,
        ProjectFilesystemError::InvalidResourceName(ResourceNameError::ForbiddenCharacter('/'))
    );
}

#[test]
fn graph_create_rejects_invalid_resource_name_without_effects() {
    let project = TestProject::new("invalid-create-name");
    let state = project.state(ProjectData::new());
    let session = state.capture_project_session().unwrap();
    let before = serde_json::to_value(state.get_data().unwrap()).unwrap();

    let error = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Sales/Report",
            GraphDocumentKind::Event,
            OperationId::new(),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ProjectFilesystemError::InvalidResourceName(ResourceNameError::ForbiddenCharacter('/'))
    );
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        before
    );
    assert!(
        !project
            .root
            .join("events/Sales_Report.yssbi-event")
            .exists()
    );
}

#[test]
fn function_create_after_same_path_removal_continues_the_tombstone_revision() {
    let project = TestProject::new("function-recreate-tombstone");
    let state = project.state(ProjectData::new());
    let session = state.capture_project_session().unwrap();

    let created = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Reusable",
            GraphDocumentKind::Function,
            OperationId::new(),
        )
        .unwrap();
    let path = super::fixture_result_path(&created).unwrap();
    state
        .load_graph_resource(&session.instance_id, &path, 1)
        .unwrap();
    let created_revision = state.graph_revisions.read().unwrap()[&path];
    state
        .remove_graph_resource_transaction(
            &session.instance_id,
            &path,
            created_revision,
            OperationId::new(),
        )
        .unwrap();
    let tombstone_revision = state.graph_revisions.read().unwrap()[&path];

    let recreated = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Reusable",
            GraphDocumentKind::Function,
            OperationId::new(),
        )
        .unwrap();
    let recreated_path = super::fixture_result_path(&recreated).unwrap();
    let recreated_revision = state.graph_revisions.read().unwrap()[&recreated_path];
    let persisted: GraphDocument =
        serde_json::from_slice(&std::fs::read(project.root.join(recreated_path.as_str())).unwrap())
            .unwrap();

    assert_eq!(recreated_path, path);
    assert_eq!(recreated_revision, tombstone_revision.next());
    assert_eq!(persisted.revision, recreated_revision);
    assert_eq!(persisted.function.unwrap().revision, recreated_revision);
}

fn result_graph_path(result: &crate::event::ResourceMutationResultDto) -> GraphResourcePath {
    if let Some(resource_move) = result.moves.first() {
        return graph_path(&resource_move.to);
    }
    let paths = match &result.projection_status {
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths,
        } => expected_graph_paths,
        crate::event::ProjectionStatusDto::Incomplete {
            invalidated_graph_paths,
        } => invalidated_graph_paths,
    };
    let path = paths
        .iter()
        .find(|path| path.starts_with("events/") || path.starts_with("functions/"))
        .expect("resource result must identify its graph path");
    graph_path(path)
}

#[test]
fn duplicate_loaded_graph_uses_resident_authority_without_reading_disk() {
    let project = TestProject::new("duplicate-loaded-authority");
    let source = graph_path("events/Resident.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Resident", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    std::fs::remove_file(project.root.join(source.as_str())).unwrap();
    let session = state.capture_project_session().unwrap();

    let result = state
        .duplicate_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        )
        .unwrap();

    let target = result_graph_path(&result);
    assert!(project.root.join(target.as_str()).is_file());
    assert!(!state.get_data().unwrap().graphs.contains_key(&target));
}

#[test]
fn create_and_duplicate_publish_unloaded_graphs_once_without_residency() {
    enum Case {
        Create,
        Duplicate,
    }

    let mut outcomes = Vec::new();
    for (label, case) in [("create", Case::Create), ("duplicate", Case::Duplicate)] {
        let project = TestProject::new(&format!("{label}-unloaded-publication"));
        let source = graph_path("events/Source.yssbi-event");
        let mut data = ProjectData::new();
        if matches!(case, Case::Duplicate) {
            data.graphs.insert(
                source.clone(),
                GraphResourceDocument::new("Source", GraphDocumentKind::Event),
            );
        }
        let state = project.state(data);
        let session = state.capture_project_session().unwrap();
        let publication_before = state.publication_state_for_test();
        let observations = Arc::new(Mutex::new(Vec::new()));
        let hook_observations = Arc::clone(&observations);
        let hook_state = state.clone();
        let root = project.root.clone();
        state.set_resource_mutation_test_hook(Some(Arc::new(move |point, path| {
            if matches!(
                point,
                ResourceMutationTestPoint::Committed | ResourceMutationTestPoint::BeforePublication
            ) {
                let path = path.expect("publication checkpoint identifies the target");
                hook_observations.lock().unwrap().push((
                    point,
                    root.join(path.as_str()).is_file(),
                    hook_state
                        .project_data
                        .read()
                        .unwrap()
                        .graphs
                        .contains_key(path),
                ));
            }
        })));

        let result = match case {
            Case::Create => state.create_graph_resource_transaction(
                &session.instance_id,
                "Created",
                GraphDocumentKind::Event,
                OperationId::new(),
            ),
            Case::Duplicate => state.duplicate_graph_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                OperationId::new(),
            ),
        }
        .unwrap();
        let target = result_graph_path(&result);
        let publication_after = state.publication_state_for_test();
        let final_resident = state.get_data().unwrap().graphs.contains_key(&target);
        let target_declared = state.graph_revisions.read().unwrap().contains_key(&target);
        outcomes.push((
            label,
            publication_before,
            publication_after,
            observations.lock().unwrap().clone(),
            final_resident,
            target_declared,
        ));
    }

    for (
        label,
        publication_before,
        publication_after,
        observations,
        final_resident,
        target_declared,
    ) in outcomes
    {
        assert_eq!(
            observations,
            vec![
                (ResourceMutationTestPoint::Committed, true, false),
                (ResourceMutationTestPoint::BeforePublication, true, false),
            ],
            "{label} must remain unloaded after disk commit"
        );
        assert!(!final_resident, "{label} target must finish unloaded");
        assert!(target_declared, "{label} target must be declared");
        assert_eq!(publication_after.0, publication_before.0);
        assert_eq!(
            publication_after.1,
            publication_before.1 + 1,
            "{label} must publish once"
        );
        assert_eq!(
            publication_after.2,
            publication_before.2 + 1,
            "{label} must advance authority once"
        );
    }
}

fn reference_node(path: &GraphResourcePath) -> DocumentNode {
    let mut parameters = ParameterValues::new();
    parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(path.as_str()),
    );
    DocumentNode {
        id: NodeId::new(),
        node_type: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
        parameters,
        user_label: None,
    }
}

#[cfg(windows)]
fn try_link_test_file(link: &std::path::Path, target: &std::path::Path) -> bool {
    match std::os::windows::fs::symlink_file(target, link) {
        Ok(()) => true,
        Err(error)
            if error.raw_os_error() == Some(1314)
                || error.kind() == std::io::ErrorKind::Unsupported =>
        {
            eprintln!("skipping test: Windows file symlinks are unavailable: {error}");
            false
        }
        Err(error) => panic!("failed to create test file symlink: {error}"),
    }
}

#[cfg(unix)]
fn try_link_test_file(link: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(target, link).unwrap();
    true
}

#[cfg(windows)]
fn link_test_directory(link: &std::path::Path, target: &std::path::Path) {
    let status = std::process::Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            link.to_string_lossy().as_ref(),
            target.to_string_lossy().as_ref(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(unix)]
fn link_test_directory(link: &std::path::Path, target: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

fn scoped_variable(name: &str, path: &GraphResourcePath) -> VariableInstance {
    VariableInstance {
        id: VariableId::new(),
        name: name.into(),
        data_type: DataType::Int64,
        data_value: DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: VariableScope::Function {
            function_path: path.as_str().into(),
        },
        tags: Vec::new(),
    }
}

#[test]
fn duplicate_operation_is_rejected_while_in_flight_and_after_success() {
    let project = TestProject::new("duplicate-operation-admission");
    let state = Arc::new(project.state(ProjectData::new()));
    let session = state.capture_project_session().unwrap();
    let operation_id = OperationId::new();
    let (committed_tx, committed_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    let first_commit = Arc::new(std::sync::atomic::AtomicBool::new(true));
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
        if point == ResourceMutationTestPoint::Committed
            && first_commit.swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            committed_tx.send(()).unwrap();
            resume_rx.lock().unwrap().recv().unwrap();
        }
    })));

    let first_state = Arc::clone(&state);
    let first_session = session.clone();
    let first = std::thread::spawn(move || {
        first_state.create_graph_resource_transaction(
            &first_session.instance_id,
            "Once",
            GraphDocumentKind::Event,
            operation_id,
        )
    });
    committed_rx.recv().unwrap();

    let second_state = Arc::clone(&state);
    let second_session = session.clone();
    let (second_tx, second_rx) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        let result = second_state.create_graph_resource_transaction(
            &second_session.instance_id,
            "Once",
            GraphDocumentKind::Event,
            operation_id,
        );
        second_tx.send(result).unwrap();
    });
    let concurrent = second_rx.recv_timeout(std::time::Duration::from_millis(100));
    resume_tx.send(()).unwrap();
    let first_result = first.join().unwrap().unwrap();
    let concurrent = concurrent.unwrap_or_else(|_| second_rx.recv().unwrap());
    second.join().unwrap();

    assert_eq!(concurrent.unwrap_err().code(), "duplicate_operation");
    let replay = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Once",
            GraphDocumentKind::Event,
            operation_id,
        )
        .unwrap_err();
    assert_eq!(replay.code(), "duplicate_operation");
    assert_eq!(
        first_result.project_instance_id,
        session.instance_id.as_str()
    );
    assert_eq!(
        std::fs::read_dir(project.root.join("events"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .count(),
        1
    );
}

#[test]
fn old_session_reservations_cannot_clear_same_uuid_new_session_reservations() {
    let project = TestProject::new("operation-ledger-owner-old");
    let state = project.state(ProjectData::new());
    let old_session = state.capture_project_session().unwrap();
    let completed_id = OperationId::new();
    let dropped_id = OperationId::new();
    let old_complete = state
        .reserve_resource_operation(&old_session.instance_id, completed_id)
        .unwrap();
    let old_drop = state
        .reserve_resource_operation(&old_session.instance_id, dropped_id)
        .unwrap();

    let replacement = TestProject::new("operation-ledger-owner-new");
    state.activate_project_fixture(
        replacement.root.to_string_lossy().into_owned(),
        ProjectData::new(),
    );
    let new_session = state.capture_project_session().unwrap();
    let new_complete = state
        .reserve_resource_operation(&new_session.instance_id, completed_id)
        .unwrap();
    let new_drop = state
        .reserve_resource_operation(&new_session.instance_id, dropped_id)
        .unwrap();

    old_complete.complete();
    drop(old_drop);

    assert_eq!(
        state
            .reserve_resource_operation(&new_session.instance_id, completed_id)
            .err()
            .unwrap()
            .code(),
        "duplicate_operation"
    );
    assert_eq!(
        state
            .reserve_resource_operation(&new_session.instance_id, dropped_id)
            .err()
            .unwrap()
            .code(),
        "duplicate_operation"
    );
    drop(new_complete);
    drop(new_drop);
}

#[test]
fn activation_swaps_the_operation_ledger_inside_the_publication_boundary() {
    let project = TestProject::new("operation-ledger-atomic-old");
    let state = project.state(ProjectData::new());
    let hook_state = state.clone();
    state.set_activation_store_replaced_test_hook(std::sync::Arc::new(move || {
        assert!(hook_state.resource_operations.try_lock().is_err());
    }));

    let replacement = TestProject::new("operation-ledger-atomic-new");
    state.activate_project_fixture(
        replacement.root.to_string_lossy().into_owned(),
        ProjectData::new(),
    );

    let new_session = state.capture_project_session().unwrap();
    let operation_id = OperationId::new();
    let reservation = state
        .reserve_resource_operation(&new_session.instance_id, operation_id)
        .unwrap();
    assert_eq!(
        state
            .reserve_resource_operation(&new_session.instance_id, operation_id)
            .err()
            .unwrap()
            .code(),
        "duplicate_operation"
    );
    drop(reservation);
}

#[test]
fn failed_operation_releases_its_reservation_for_retry() {
    let project = TestProject::new("failed-operation-release");
    let state = project.state(ProjectData::new());
    let session = state.capture_project_session().unwrap();
    let operation_id = OperationId::new();
    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::FirstLiveReplacement,
    ));

    let first = state.create_graph_resource_transaction(
        &session.instance_id,
        "Retry",
        GraphDocumentKind::Event,
        operation_id,
    );
    state.set_project_filesystem_fault(None);
    assert_eq!(first.unwrap_err().code(), "transaction_commit_failed");

    let retry = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Retry",
            GraphDocumentKind::Event,
            operation_id,
        )
        .unwrap();
    assert_eq!(retry.project_instance_id, session.instance_id.as_str());
}

#[test]
fn create_rechecks_destination_under_lease_and_routes_insert_through_project_state() {
    let project = TestProject::new("create-destination-race");
    let state = Arc::new(project.state(ProjectData::new()));
    let session = state.capture_project_session().unwrap();
    let root = project.root.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, candidate| {
        if point == ResourceMutationTestPoint::Planned {
            let candidate = candidate.expect("create planning exposes candidate");
            let target = root.join(candidate.as_str());
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let competing = GraphResourceDocument::new("Race", GraphDocumentKind::Event);
            let contents = crate::project::project_io::serialize_graph_resource_document(
                &competing,
                HashMap::new(),
            )
            .unwrap();
            std::fs::write(target, contents).unwrap();
        }
    })));

    let result = state
        .create_graph_resource_transaction(
            &session.instance_id,
            "Race",
            GraphDocumentKind::Event,
            OperationId::new(),
        )
        .unwrap();
    let created = result_graph_path(&result);

    assert_ne!(created, graph_path("events/Race.yssbi-event"));
    let competing: GraphDocument = serde_json::from_slice(
        &std::fs::read(project.root.join("events/Race.yssbi-event")).unwrap(),
    )
    .unwrap();
    assert_eq!(competing.name, "Race");
    assert!(project.root.join(created.as_str()).is_file());
    assert!(!state.get_data().unwrap().graphs.contains_key(&created));
    assert!(state.graph_revisions.read().unwrap().contains_key(&created));
}

#[test]
fn duplicate_rejects_redirected_source_under_root_lease() {
    let project = TestProject::new("duplicate-redirected-source");
    let source = graph_path("events/Source.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    let source_file = project.root.join(source.as_str());
    let outside = std::env::temp_dir().join(format!(
        "yssbi-duplicate-external-{}.yssbi-event",
        uuid::Uuid::new_v4()
    ));
    std::fs::copy(&source_file, &outside).unwrap();
    std::fs::remove_file(&source_file).unwrap();
    if !try_link_test_file(&source_file, &outside) {
        let _ = std::fs::remove_file(&outside);
        return;
    }
    let session = state.capture_project_session().unwrap();

    let result = state.duplicate_graph_resource_transaction(
        &session.instance_id,
        &source,
        ResourceRevision::INITIAL,
        OperationId::new(),
    );

    let _ = std::fs::remove_file(&outside);
    assert_eq!(result.unwrap_err().code(), "transaction_prepare_failed");
    assert_eq!(
        std::fs::read_dir(project.root.join("events"))
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        1
    );
}

fn assert_remove_rejects_redirected_file(label: &str) {
    let project = TestProject::new(label);
    let source = graph_path("events/Source.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    let source_file = project.root.join(source.as_str());
    let outside = std::env::temp_dir().join(format!(
        "yssbi-remove-external-{}.yssbi-event",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&outside, b"external contents must not be parsed").unwrap();
    std::fs::remove_file(&source_file).unwrap();
    if !try_link_test_file(&source_file, &outside) {
        let _ = std::fs::remove_file(&outside);
        return;
    }
    let session = state.capture_project_session().unwrap();

    let error = state
        .remove_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        )
        .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(
        error.to_string().contains("redirect"),
        "redirect must be rejected before parsing external bytes: {error}"
    );
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        b"external contents must not be parsed"
    );
    let _ = std::fs::remove_file(&source_file);
    let _ = std::fs::remove_file(outside);
}

fn assert_remove_rejects_redirected_directory(label: &str) {
    let project = TestProject::new(label);
    let source = graph_path("events/Source.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    let events = project.root.join("events");
    std::fs::remove_dir_all(&events).unwrap();
    let outside = std::env::temp_dir().join(format!(
        "yssbi-remove-external-directory-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("Source.yssbi-event"),
        b"external contents must not be parsed",
    )
    .unwrap();
    link_test_directory(&events, &outside);
    let session = state.capture_project_session().unwrap();

    let error = state
        .remove_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        )
        .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(
        error.to_string().contains("redirect"),
        "redirect ancestor must be rejected before parsing external bytes: {error}"
    );
    assert_eq!(
        std::fs::read(outside.join("Source.yssbi-event")).unwrap(),
        b"external contents must not be parsed"
    );
    let _ = std::fs::remove_dir(&events);
    let _ = std::fs::remove_dir_all(outside);
}

#[cfg(windows)]
#[test]
fn remove_rejects_real_windows_file_reparse_point_before_read() {
    assert_remove_rejects_redirected_file("remove-windows-file-reparse");
}

#[cfg(windows)]
#[test]
fn remove_rejects_real_windows_directory_junction_before_read() {
    assert_remove_rejects_redirected_directory("remove-windows-directory-junction");
}

#[cfg(unix)]
#[test]
fn remove_rejects_unix_file_symlink_before_read() {
    assert_remove_rejects_redirected_file("remove-unix-file-symlink");
}

#[cfg(unix)]
#[test]
fn remove_rejects_unix_directory_symlink_before_read() {
    assert_remove_rejects_redirected_directory("remove-unix-directory-symlink");
}

#[test]
fn duplicate_rechecks_destination_and_allocates_persistent_identities_in_rust() {
    let project = TestProject::new("duplicate-identities");
    let source = graph_path("functions/Source.yssbi-function");
    let mut resource = GraphResourceDocument::new("Source", GraphDocumentKind::Function);
    let call = reference_node(&source);
    resource.document.nodes.insert(call.id, call);
    let variable = scoped_variable("Local", &source);
    let source_variable_id = variable.id;
    let mut data = ProjectData::new();
    data.graphs.insert(source.clone(), resource);
    data.variables.insert(variable.id, variable);
    let state = project.state(data);
    let session = state.capture_project_session().unwrap();
    let root = project.root.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, candidate| {
        if point == ResourceMutationTestPoint::Planned {
            let candidate = candidate.expect("duplicate planning exposes candidate");
            let target = root.join(candidate.as_str());
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let competing = GraphResourceDocument::new("Source 1", GraphDocumentKind::Function);
            let contents = crate::project::project_io::serialize_graph_resource_document(
                &competing,
                HashMap::new(),
            )
            .unwrap();
            std::fs::write(target, contents).unwrap();
        }
    })));

    let result = state
        .duplicate_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            OperationId::new(),
        )
        .unwrap();
    let duplicated = result_graph_path(&result);
    let source_disk: GraphDocument =
        serde_json::from_slice(&std::fs::read(project.root.join(source.as_str())).unwrap())
            .unwrap();
    let duplicate_disk: GraphDocument =
        serde_json::from_slice(&std::fs::read(project.root.join(duplicated.as_str())).unwrap())
            .unwrap();

    assert_ne!(duplicated, graph_path("functions/Source 1.yssbi-function"));
    assert!(
        source_disk
            .document
            .nodes
            .keys()
            .all(|id| !duplicate_disk.document.nodes.contains_key(id))
    );
    assert!(
        source_disk
            .document
            .connections
            .keys()
            .all(|id| !duplicate_disk.document.connections.contains_key(id))
    );
    assert!(
        !duplicate_disk
            .local_variables
            .contains_key(&source_variable_id)
    );
    assert!(duplicate_disk.local_variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == duplicated.as_str())
        }));
    assert!(duplicate_disk.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(duplicated.as_str()))
    }));
    assert!(!state.get_data().unwrap().graphs.contains_key(&duplicated));
}

#[test]
fn duplicate_loaded_function_requires_owner_embedded_and_ledger_exact() {
    let source = graph_path("functions/LoadedAuthority.yssbi-function");

    let owner_mismatch = TestProject::new("duplicate-loaded-owner-ledger-mismatch");
    let owner_state = owner_mismatch.state(function_data(
        &source,
        ResourceRevision::INITIAL,
        ResourceRevision::INITIAL,
    ));
    owner_state
        .graph_revisions
        .write()
        .unwrap()
        .insert(source.clone(), ResourceRevision::new(1));
    assert_duplicate_revision_conflict_without_effects(
        &owner_state,
        &owner_mismatch.root,
        &source,
        ResourceRevision::new(1),
    );

    let embedded_mismatch = TestProject::new("duplicate-loaded-embedded-mismatch");
    let embedded_state = embedded_mismatch.state(function_data(
        &source,
        ResourceRevision::INITIAL,
        ResourceRevision::INITIAL,
    ));
    embedded_state
        .project_data
        .write()
        .unwrap()
        .graphs
        .get_mut(&source)
        .unwrap()
        .function
        .as_mut()
        .unwrap()
        .revision = ResourceRevision::new(1);
    assert_duplicate_revision_conflict_without_effects(
        &embedded_state,
        &embedded_mismatch.root,
        &source,
        ResourceRevision::INITIAL,
    );
}

#[test]
fn duplicate_unloaded_function_rejects_ahead_or_incoherent_persisted_revisions() {
    let cases = [
        (
            "owner-ahead",
            ResourceRevision::new(1),
            ResourceRevision::new(2),
            ResourceRevision::new(2),
            ResourceRevision::new(2),
        ),
        (
            "embedded-ahead",
            ResourceRevision::new(1),
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
            ResourceRevision::new(2),
        ),
        (
            "embedded-incoherent",
            ResourceRevision::new(2),
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
            ResourceRevision::new(1),
        ),
    ];

    for (label, authority, owner, graph, embedded) in cases {
        let project = TestProject::new(&format!("duplicate-unloaded-{label}"));
        let source = graph_path("functions/UnloadedAuthority.yssbi-function");
        let state = project.state(function_data(
            &source,
            ResourceRevision::INITIAL,
            ResourceRevision::INITIAL,
        ));
        state.project_data.write().unwrap().graphs.remove(&source);
        state
            .graph_revisions
            .write()
            .unwrap()
            .insert(source.clone(), authority);
        rewrite_persisted_function_revisions(&project.root, &source, owner, graph, embedded);

        assert_duplicate_revision_conflict_without_effects(
            &state,
            &project.root,
            &source,
            authority,
        );
    }
}

#[test]
fn duplicate_unloaded_function_uses_exact_retained_token_and_initial_target() {
    let project = TestProject::new("duplicate-unloaded-retained-happy");
    let source = graph_path("functions/Retained.yssbi-function");
    let variable = scoped_variable("Retained local", &source);
    let source_variable_id = variable.id;
    let mut data = function_data(
        &source,
        ResourceRevision::INITIAL,
        ResourceRevision::INITIAL,
    );
    data.variables.insert(variable.id, variable);
    let state = project.state(data);
    state.project_data.write().unwrap().graphs.remove(&source);
    let retained = ResourceRevision::new(5);
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(source.clone(), retained);
    rewrite_persisted_function_revisions(
        &project.root,
        &source,
        ResourceRevision::new(1),
        ResourceRevision::new(1),
        ResourceRevision::new(1),
    );

    assert_duplicate_revision_conflict_without_effects(
        &state,
        &project.root,
        &source,
        ResourceRevision::new(4),
    );

    let session = state.capture_project_session().unwrap();
    let result = state
        .duplicate_graph_resource_transaction(
            &session.instance_id,
            &source,
            retained,
            OperationId::new(),
        )
        .unwrap();
    let target = result_graph_path(&result);
    let target_document: GraphDocument =
        serde_json::from_slice(&std::fs::read(project.root.join(target.as_str())).unwrap())
            .unwrap();

    assert_eq!(target_document.revision, ResourceRevision::INITIAL);
    assert_eq!(target_document.document.revision, ResourceRevision::INITIAL);
    assert_eq!(
        target_document.function.as_ref().unwrap().revision,
        ResourceRevision::INITIAL
    );
    assert!(
        !target_document
            .local_variables
            .contains_key(&source_variable_id)
    );
    assert!(target_document.local_variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == target.as_str())
        }));
    assert_eq!(state.graph_revisions.read().unwrap()[&source], retained);
    assert_eq!(
        state.graph_revisions.read().unwrap()[&target],
        ResourceRevision::INITIAL
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&target));
}

#[test]
fn remove_rolls_back_file_when_authoritative_revision_changed() {
    let project = TestProject::new("remove-stale-publication");
    let path = graph_path("events/Remove.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        path.clone(),
        GraphResourceDocument::new("Remove", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    let before = std::fs::read(project.root.join(path.as_str())).unwrap();
    let concurrent = state.clone();
    let concurrent_path = path.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
        if point == ResourceMutationTestPoint::BeforePublication {
            let mut data = concurrent.project_data.write().unwrap();
            data.graphs
                .get_mut(&concurrent_path)
                .unwrap()
                .document
                .revision = ResourceRevision::new(1);
        }
    })));
    let session = state.capture_project_session().unwrap();

    let error = state
        .remove_graph_resource_transaction(
            &session.instance_id,
            &path,
            ResourceRevision::INITIAL,
            OperationId::new(),
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        std::fs::read(project.root.join(path.as_str())).unwrap(),
        before
    );
    assert!(state.get_data().unwrap().graphs.contains_key(&path));
}

#[test]
fn unloaded_source_rename_preserves_persisted_local_variables_on_reload() {
    let project = TestProject::new("rename-unloaded-source-locals");
    let source = graph_path("functions/Source.yssbi-function");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Function),
    );
    let mut first = scoped_variable("First", &source);
    first.data_value = DataValue::Int64(41);
    first.description = "first persisted local".into();
    first.tags = vec!["alpha".into()];
    let first_id = first.id;
    let mut second = scoped_variable("Second", &source);
    second.data_value = DataValue::Int64(42);
    second.description = "second persisted local".into();
    second.tags = vec!["beta".into()];
    let second_id = second.id;
    data.variables.insert(first_id, first.clone());
    data.variables.insert(second_id, second.clone());
    let state = project.state(data);
    state.unload_graph_resource(&source).unwrap();
    assert!(state.get_data().unwrap().variables.is_empty());
    let session = state.capture_project_session().unwrap();

    let result = state
        .rename_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Renamed",
            1,
            OperationId::new(),
        )
        .unwrap();
    let target = result_graph_path(&result);
    assert!(!project.root.join(source.as_str()).exists());

    let reloaded = ProjectState::new();
    let reloaded_session = reloaded.activate_project_from_path(&project.root).unwrap();
    reloaded
        .load_graph_projection(&reloaded_session.instance_id, &target, 1, "en-US")
        .unwrap();
    let variables = reloaded.get_data().unwrap().variables;
    assert_eq!(variables.len(), 2);
    assert_eq!(variables[&first_id].data_value, first.data_value);
    assert_eq!(variables[&first_id].description, first.description);
    assert_eq!(variables[&first_id].tags, first.tags);
    assert_eq!(variables[&second_id].data_value, second.data_value);
    assert_eq!(variables[&second_id].description, second.description);
    assert_eq!(variables[&second_id].tags, second.tags);
    assert!(variables.values().all(|variable| {
            matches!(&variable.scope, VariableScope::Function { function_path } if function_path == target.as_str())
        }));
}

#[test]
fn loaded_caller_rename_cascade_survives_fresh_reload() {
    let project = TestProject::new("rename-loaded-caller-persistence");
    let source = graph_path("functions/Source.yssbi-function");
    let caller = graph_path("events/Caller.yssbi-event");
    let mut caller_resource = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
    let call = reference_node(&source);
    caller_resource.document.nodes.insert(call.id, call);
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Function),
    );
    data.graphs.insert(caller.clone(), caller_resource);
    let state = project.state(data);
    let session = state.capture_project_session().unwrap();

    let result = state
        .rename_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Renamed",
            1,
            OperationId::new(),
        )
        .unwrap();
    let target = graph_path(&result.moves.first().unwrap().to);
    let authority = state.get_data().unwrap();
    let target_revision = authority.graphs[&target].document.revision.get();
    let caller_revision = authority.graphs[&caller].document.revision.get();
    drop(authority);
    let target_replacement = result
        .projection_replacements
        .iter()
        .find(|replacement| replacement.graph_path == target.as_str())
        .expect("rename result must replace the loaded destination");
    assert_eq!(
        target_replacement.projection.source_revision,
        target_revision
    );
    let caller_replacement = result
        .projection_replacements
        .iter()
        .find(|replacement| replacement.graph_path == caller.as_str())
        .expect("rename result must replace every loaded affected caller");
    assert_eq!(
        caller_replacement.projection.source_revision,
        caller_revision
    );
    assert!(caller_replacement.projection.nodes.iter().any(|node| {
        node.parameter_editors.iter().any(|editor| {
            editor.value.as_ref().and_then(serde_json::Value::as_str) == Some(target.as_str())
        })
    }));
    assert!(caller_replacement.projection.nodes.iter().all(|node| {
        node.parameter_editors.iter().all(|editor| {
            editor.value.as_ref().and_then(serde_json::Value::as_str) != Some(source.as_str())
        })
    }));

    let persisted: GraphDocument =
        serde_json::from_slice(&std::fs::read(project.root.join(caller.as_str())).unwrap())
            .unwrap();
    assert!(persisted.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(target.as_str()))
    }));
    assert!(persisted.document.nodes.values().all(|node| {
        node.parameters
            .values()
            .all(|value| value.as_str() != Some(source.as_str()))
    }));

    let reloaded = ProjectState::new();
    let reloaded_session = reloaded.activate_project_from_path(&project.root).unwrap();
    reloaded
        .load_graph_projection(&reloaded_session.instance_id, &caller, 1, "en-US")
        .unwrap();
    let authority = reloaded.get_data().unwrap();
    assert!(
        authority.graphs[&caller]
            .document
            .nodes
            .values()
            .any(|node| {
                node.parameters
                    .values()
                    .any(|value| value.as_str() == Some(target.as_str()))
            })
    );
    assert!(
        authority.graphs[&caller]
            .document
            .nodes
            .values()
            .all(|node| {
                node.parameters
                    .values()
                    .all(|value| value.as_str() != Some(source.as_str()))
            })
    );
}

#[test]
fn rename_stages_complete_reference_cascade_before_live_mutation() {
    let project = TestProject::new("rename-prepared-cascade");
    let source = graph_path("functions/Source.yssbi-function");
    let caller = graph_path("events/Caller.yssbi-event");
    let mut caller_resource = GraphResourceDocument::new("Caller", GraphDocumentKind::Event);
    let call = reference_node(&source);
    caller_resource.document.nodes.insert(call.id, call);
    let global = scoped_variable("Scoped", &source);
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Function),
    );
    data.graphs.insert(caller.clone(), caller_resource);
    data.variables.insert(global.id, global);
    let state = project.state(data);
    state.unload_graph_resource(&caller).unwrap();
    let source_before = std::fs::read(project.root.join(source.as_str())).unwrap();
    let caller_before = std::fs::read(project.root.join(caller.as_str())).unwrap();
    let globals_before =
        std::fs::read(project.root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let hook_state = state.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
        if point == ResourceMutationTestPoint::Prepared {
            hook_state.set_project_filesystem_fault(Some(
                crate::project::ProjectFilesystemFaultPoint::SecondLiveReplacement,
            ));
        }
    })));
    let session = state.capture_project_session().unwrap();

    let error = state
        .rename_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Renamed",
            1,
            OperationId::new(),
        )
        .unwrap_err();
    state.set_project_filesystem_fault(None);

    assert_eq!(
        error.code(),
        "transaction_commit_failed",
        "unexpected rename failure: {error}"
    );
    assert_eq!(
        std::fs::read(project.root.join(source.as_str())).unwrap(),
        source_before
    );
    assert_eq!(
        std::fs::read(project.root.join(caller.as_str())).unwrap(),
        caller_before
    );
    assert_eq!(
        std::fs::read(project.root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        globals_before
    );
    assert!(
        !project
            .root
            .join("functions/Renamed.yssbi-function")
            .exists()
    );
}

#[test]
fn rename_rollback_restores_only_target_graph_global_and_worksheet_paths() {
    let project = TestProject::new("rename-precise-rollback");
    let source = graph_path("events/Source.yssbi-event");
    let unrelated = project.root.join("events/unrelated.bin");
    let worksheet = project.root.join("worksheets/unrelated.yssbi-worksheet");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    std::fs::write(&unrelated, b"unrelated graph sentinel").unwrap();
    std::fs::create_dir_all(worksheet.parent().unwrap()).unwrap();
    std::fs::write(&worksheet, b"worksheet sentinel").unwrap();
    let before = std::fs::read(project.root.join(source.as_str())).unwrap();
    let concurrent = state.clone();
    let source_for_hook = source.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
        if point == ResourceMutationTestPoint::BeforePublication {
            concurrent
                .project_data
                .write()
                .unwrap()
                .graphs
                .get_mut(&source_for_hook)
                .unwrap()
                .document
                .revision = ResourceRevision::new(1);
        }
    })));
    let session = state.capture_project_session().unwrap();

    let error = state
        .rename_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Renamed",
            1,
            OperationId::new(),
        )
        .unwrap_err();

    assert_eq!(error.code(), "resource_revision_conflict");
    assert_eq!(
        std::fs::read(project.root.join(source.as_str())).unwrap(),
        before
    );
    assert!(!project.root.join("events/Renamed.yssbi-event").exists());
    assert_eq!(
        std::fs::read(unrelated).unwrap(),
        b"unrelated graph sentinel"
    );
    assert_eq!(std::fs::read(worksheet).unwrap(), b"worksheet sentinel");
}

#[test]
fn rename_narrow_patch_preserves_unrelated_graph_variable_and_history_mutations() {
    let project = TestProject::new("rename-narrow-publication");
    let source = graph_path("events/Source.yssbi-event");
    let unrelated_path = graph_path("events/Unrelated.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    data.graphs.insert(
        unrelated_path.clone(),
        GraphResourceDocument::new("Unrelated", GraphDocumentKind::Event),
    );
    let state = project.state(data);
    let mut variable = scoped_variable("Concurrent", &source);
    variable.scope = VariableScope::Global;
    let variable_id = variable.id;
    let concurrent = state.clone();
    let unrelated_for_hook = unrelated_path.clone();
    state.set_resource_mutation_test_hook(Some(Arc::new(move |point, _| {
        if point == ResourceMutationTestPoint::BeforePublication {
            concurrent
                .project_data
                .write()
                .unwrap()
                .graphs
                .get_mut(&unrelated_for_hook)
                .unwrap()
                .name = "Concurrent graph".into();
            concurrent
                .project_data
                .write()
                .unwrap()
                .variables
                .insert(variable_id, variable.clone());
            concurrent
                .graph_revisions
                .write()
                .unwrap()
                .insert(unrelated_for_hook.clone(), ResourceRevision::new(9));
            concurrent.variable_revisions.write().unwrap().insert(
                variable_id,
                crate::project::project_state::VariableRevisionEntry::present(
                    ResourceRevision::new(7),
                ),
            );
            concurrent.append_history_head_for_test();
        }
    })));
    let session = state.capture_project_session().unwrap();

    let result = state
        .rename_graph_resource_transaction(
            &session.instance_id,
            &source,
            ResourceRevision::INITIAL,
            "Renamed",
            1,
            OperationId::new(),
        )
        .unwrap();

    let authority = state.get_data().unwrap();
    assert_eq!(authority.graphs[&unrelated_path].name, "Concurrent graph");
    assert_eq!(authority.variables[&variable_id].name, "Concurrent");
    assert_eq!(
        state.graph_revisions.read().unwrap()[&unrelated_path],
        ResourceRevision::new(9)
    );
    assert_eq!(
        state.variable_revisions.read().unwrap()[&variable_id].revision,
        ResourceRevision::new(7)
    );
    assert_eq!(state.history.read().unwrap().undo_len(), 2);
    assert!(result.history.can_undo);
}

#[test]
fn old_project_create_duplicate_remove_and_rename_have_zero_effects() {
    let old = TestProject::new("old-project");
    let source = graph_path("events/Source.yssbi-event");
    let mut data = ProjectData::new();
    data.graphs.insert(
        source.clone(),
        GraphResourceDocument::new("Source", GraphDocumentKind::Event),
    );
    let state = old.state(data);
    let old_session = state.capture_project_session().unwrap();
    let old_files = std::fs::read(old.root.join(source.as_str())).unwrap();
    let replacement = TestProject::new("replacement-project");
    state.activate_project_fixture(
        replacement.root.to_string_lossy().into_owned(),
        ProjectData::new(),
    );

    let create = state.create_graph_resource_transaction(
        &old_session.instance_id,
        "Stale",
        GraphDocumentKind::Event,
        OperationId::new(),
    );
    let duplicate = state.duplicate_graph_resource_transaction(
        &old_session.instance_id,
        &source,
        ResourceRevision::INITIAL,
        OperationId::new(),
    );
    let remove = state.remove_graph_resource_transaction(
        &old_session.instance_id,
        &source,
        ResourceRevision::INITIAL,
        OperationId::new(),
    );
    let rename = state.rename_graph_resource_transaction(
        &old_session.instance_id,
        &source,
        ResourceRevision::INITIAL,
        "Stale rename",
        1,
        OperationId::new(),
    );

    for result in [create, duplicate, remove, rename] {
        assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
    }
    assert_eq!(
        std::fs::read(old.root.join(source.as_str())).unwrap(),
        old_files
    );
    assert_eq!(state.get_data().unwrap().graphs.len(), 0);
    assert_eq!(state.authority_generation_for_test(), 0);
    assert!(!state.history_status().can_undo);
    assert_eq!(
        state
            .graph_revisions
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::new()
    );
}
