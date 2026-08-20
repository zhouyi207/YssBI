use super::{
    NormalizedProjectRoot, ProjectFilesystemCoordinator, ProjectFilesystemFaultPoint,
    ProjectFilesystemTransaction, StagedFilesystemMutation,
};
use crate::node_system::document::OperationId;
use crate::project::{
    PROJECT_METADATA_FILE, ProjectInstanceId, ProjectRecoveryMarker, ProjectSession,
    ProjectTransactionContext,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct TestDirectory {
    path: PathBuf,
    coordinator: ProjectFilesystemCoordinator,
}

impl TestDirectory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("yssbi-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self {
            path,
            coordinator: ProjectFilesystemCoordinator::default(),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn coordinator(&self) -> &ProjectFilesystemCoordinator {
        &self.coordinator
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn normalized(path: impl AsRef<Path>) -> NormalizedProjectRoot {
    NormalizedProjectRoot::from_project_path(path).unwrap()
}

fn transaction_context(root: NormalizedProjectRoot) -> ProjectTransactionContext {
    ProjectTransactionContext {
        session: ProjectSession {
            instance_id: ProjectInstanceId::new(),
            root,
        },
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: BTreeMap::new(),
        expected_absent_resources: Default::default(),
        recovery_marker: None,
    }
}

fn prepare_json_transaction_with_coordinator(
    coordinator: &ProjectFilesystemCoordinator,
    temporary: &TestDirectory,
    mutations: Vec<StagedFilesystemMutation>,
) -> Result<super::PreparedProjectFilesystemTransaction, super::ProjectFilesystemError> {
    let root = normalized(temporary.path());
    let lease = coordinator.acquire(root.clone()).unwrap();
    ProjectFilesystemTransaction::prepare_with_validator(
        transaction_context(root),
        lease,
        mutations,
        |_, contents| {
            serde_json::from_slice::<serde_json::Value>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
    )
}

fn prepare_json_transaction(
    temporary: &TestDirectory,
    mutations: Vec<StagedFilesystemMutation>,
) -> Result<super::PreparedProjectFilesystemTransaction, super::ProjectFilesystemError> {
    prepare_json_transaction_with_coordinator(temporary.coordinator(), temporary, mutations)
}

fn prepare_json_transaction_with_recovery_marker(
    temporary: &TestDirectory,
    mutations: Vec<StagedFilesystemMutation>,
    marker: ProjectRecoveryMarker,
) -> Result<super::PreparedProjectFilesystemTransaction, super::ProjectFilesystemError> {
    let root = normalized(temporary.path());
    let lease = temporary.coordinator().acquire(root.clone()).unwrap();
    let mut context = transaction_context(root);
    context.recovery_marker = Some(marker);
    ProjectFilesystemTransaction::prepare_with_validator(
        context,
        lease,
        mutations,
        |_, contents| {
            serde_json::from_slice::<serde_json::Value>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
    )
}

fn move_recovery_copies(temporary: &TestDirectory) -> Vec<PathBuf> {
    std::fs::read_dir(temporary.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().contains("yssbi-move-recovery"))
        })
        .collect()
}

#[test]
fn missing_parent_components_never_escape_the_canonical_existing_ancestor() {
    let temporary = TestDirectory::new("filesystem-existing-anchor");
    let existing = temporary.path().join("existing");
    std::fs::create_dir_all(&existing).unwrap();
    let expected = normalized(&existing);

    for spelling in [
        existing.join("missing").join("..").join(".."),
        existing
            .join("missing")
            .join("nested")
            .join("..")
            .join("..")
            .join(".."),
        existing
            .join("missing")
            .join("..")
            .join("nested")
            .join("..")
            .join(".."),
    ] {
        assert_eq!(normalized(spelling), expected);
    }
}

#[cfg(windows)]
#[test]
fn windows_root_preserves_native_path_and_uses_case_insensitive_identity() {
    use std::collections::hash_map::DefaultHasher;
    use std::ffi::OsString;
    use std::hash::{Hash, Hasher};
    use std::os::windows::ffi::OsStringExt;

    let temporary = TestDirectory::new("filesystem-native-windows-identity");
    let existing = temporary.path().join("Existing");
    std::fs::create_dir_all(&existing).unwrap();
    let canonical_existing = std::fs::canonicalize(&existing).unwrap();
    let upper_suffix = OsString::from_wide(&[0xd800, b'A' as u16, 0x00c4]);
    let lower_suffix = OsString::from_wide(&[0xd800, b'a' as u16, 0x00e4]);

    let upper = normalized(existing.join(&upper_suffix));
    let lower = normalized(existing.join(&lower_suffix));

    assert_eq!(
        upper.as_path(),
        canonical_existing.join(&upper_suffix).as_path()
    );
    assert_eq!(
        lower.as_path(),
        canonical_existing.join(&lower_suffix).as_path()
    );
    assert_eq!(upper, lower);
    assert_eq!(upper.cmp(&lower), std::cmp::Ordering::Equal);

    let hash = |root: &NormalizedProjectRoot| {
        let mut hasher = DefaultHasher::new();
        root.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(&upper), hash(&lower));
}

#[test]
fn metadata_and_directory_paths_normalize_to_the_same_root() {
    let temporary = TestDirectory::new("filesystem-metadata-root");
    let root = temporary.path().join("project");
    std::fs::create_dir_all(&root).unwrap();
    let metadata = root.join(PROJECT_METADATA_FILE);
    std::fs::write(&metadata, "{}").unwrap();

    assert_eq!(normalized(&root), normalized(&metadata));
    assert_eq!(normalized(&root), normalized(root.join("METADATA.YSSBI")));
}

#[test]
fn lifecycle_close_rejects_new_operations_and_reopens_only_after_final_lease() {
    let temporary = TestDirectory::new("filesystem-lifecycle-close");
    let root = normalized(temporary.path().join("project"));
    let coordinator = ProjectFilesystemCoordinator::default();

    let mut lifecycle = coordinator.begin_root_lifecycle(root.clone()).unwrap();
    let error = coordinator.acquire(root.clone()).err().unwrap();
    assert_eq!(error.code(), "project_lifecycle_admission_closed");

    lifecycle.release_initial_and_drain();
    lifecycle.acquire_final().unwrap();
    assert!(lifecycle.holds_lease());
    assert_eq!(
        coordinator.acquire(root.clone()).err().unwrap().code(),
        "project_lifecycle_admission_closed"
    );

    drop(lifecycle);
    drop(coordinator.acquire(root).unwrap());
}

#[test]
fn file_move_commits_source_to_destination_atomically() {
    let temporary = TestDirectory::new("transaction-file-move-commit");
    std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();
    let committed = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
    )
    .unwrap()
    .commit()
    .unwrap();

    assert!(!temporary.path().join("source.json").exists());
    assert_eq!(
        std::fs::read(temporary.path().join("destination.json")).unwrap(),
        br#"{"source":1}"#
    );
    committed.finalize();
}

#[test]
fn file_move_rollback_restores_source_but_retains_target_for_recovery() {
    let temporary = TestDirectory::new("transaction-file-move-rollback");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
}

#[test]
fn nested_move_always_places_recovery_artifact_in_project_root() {
    let temporary = TestDirectory::new("transaction-nested-move-root-recovery");
    let source_directory = temporary.path().join("dir");
    let source = source_directory.join("source.json");
    let target = temporary.path().join("target.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "dir/source.json".into(),
            to: "target.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();

    let error = committed.rollback().unwrap_err();

    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
    assert!(
        !std::fs::read_dir(&source_directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .any(|name| name.to_string_lossy().contains("yssbi-move-recovery"))
    );
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
}

#[test]
fn mixed_move_explicit_rollback_falls_back_to_root_recovery_copy() {
    let temporary = TestDirectory::new("transaction-mixed-move-explicit-rollback-fallback");
    let source_directory = temporary.path().join("dir");
    let source = source_directory.join("source.json");
    let target = temporary.path().join("target.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![
            StagedFilesystemMutation::MoveFile {
                from: "dir/source.json".into(),
                to: "target.json".into(),
            },
            StagedFilesystemMutation::RemoveDirectoryIfEmpty {
                relative_path: "dir".into(),
            },
        ],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    assert!(!source_directory.exists());
    let external_child = source_directory.join("external.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&external_child, br#"{"external":1}"#).unwrap();

    let error = committed.rollback().unwrap_err();

    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
    assert!(!source.exists());
    assert_eq!(
        std::fs::read(&external_child).unwrap(),
        br#"{"external":1}"#
    );
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
}

#[test]
fn mixed_move_commit_failure_retains_root_recovery_copy_for_applied_prefix() {
    let temporary = TestDirectory::new("transaction-mixed-move-commit-failure-fallback");
    let source_directory = temporary.path().join("dir");
    let source = source_directory.join("source.json");
    let target = temporary.path().join("target.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let prepared = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![
            StagedFilesystemMutation::MoveFile {
                from: "dir/source.json".into(),
                to: "target.json".into(),
            },
            StagedFilesystemMutation::RemoveDirectoryIfEmpty {
                relative_path: "dir".into(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "after.json".into(),
                contents: br#"{"after":1}"#.to_vec(),
            },
        ],
        marker.clone(),
    )
    .unwrap();
    std::fs::remove_file(prepared.staging_root().join("prepared/after.json")).unwrap();

    let error = prepared.commit().unwrap_err();

    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
    assert!(!source.exists());
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
}

#[test]
fn mixed_move_drop_unwind_marks_recovery_and_retains_root_copy() {
    let temporary = TestDirectory::new("transaction-mixed-move-drop-fallback");
    let source_directory = temporary.path().join("dir");
    let source = source_directory.join("source.json");
    let target = temporary.path().join("target.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![
            StagedFilesystemMutation::MoveFile {
                from: "dir/source.json".into(),
                to: "target.json".into(),
            },
            StagedFilesystemMutation::RemoveDirectoryIfEmpty {
                relative_path: "dir".into(),
            },
        ],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    assert!(!source_directory.exists());
    let external_child = source_directory.join("external.json");
    std::fs::create_dir(&source_directory).unwrap();
    std::fs::write(&external_child, br#"{"external":1}"#).unwrap();

    drop(committed);

    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
    assert!(!source.exists());
    assert_eq!(
        std::fs::read(&external_child).unwrap(),
        br#"{"external":1}"#
    );
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(marker.error().is_some());
}

#[test]
fn file_move_publication_rollback_preserves_replaced_target_and_marks_recovery() {
    let temporary = TestDirectory::new("transaction-file-move-publication-rollback-race");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    let target_for_hook = target.clone();
    temporary
        .coordinator()
        .set_project_filesystem_rollback_test_hook(Some(Arc::new(move || {
            std::fs::remove_file(&target_for_hook).unwrap();
            std::fs::write(&target_for_hook, br#"{"external":1}"#).unwrap();
        })));

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&target).unwrap(), br#"{"external":1}"#);
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
}

#[test]
fn file_move_rollback_does_not_overwrite_existing_source() {
    let temporary = TestDirectory::new("transaction-file-move-existing-source");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    std::fs::write(&source, br#"{"external":1}"#).unwrap();

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&source).unwrap(), br#"{"external":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
}

#[test]
fn file_move_rollback_fault_retains_original_bytes_in_recovery_copy() {
    let temporary = TestDirectory::new("transaction-file-move-rollback-fault-copy");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    let target_for_hook = target.clone();
    temporary
        .coordinator()
        .set_project_filesystem_rollback_test_hook(Some(Arc::new(move || {
            std::fs::remove_file(&target_for_hook).unwrap();
            std::fs::write(&target_for_hook, br#"{"external":1}"#).unwrap();
        })));
    temporary
        .coordinator()
        .set_project_filesystem_rollback_fault(true);

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&target).unwrap(), br#"{"external":1}"#);
    assert!(!source.exists());
    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
}

#[test]
fn case_only_file_move_rollback_retains_target_and_recovery_copy() {
    let temporary = TestDirectory::new("transaction-case-only-file-move-rollback");
    let source = temporary.path().join("Report.json");
    let target = temporary.path().join("report.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "Report.json".into(),
            to: "report.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();

    let journal = format!("{committed:?}");
    let error = committed.rollback().unwrap_err();

    assert!(journal.contains("Report.json"));
    assert!(journal.contains("report.json"));
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
}

#[test]
fn case_only_publication_rollback_preserves_replaced_target() {
    let temporary = TestDirectory::new("transaction-case-only-publication-rollback-race");
    let source = temporary.path().join("Report.json");
    let target = temporary.path().join("report.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let case_sensitive = !target.exists();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "Report.json".into(),
            to: "report.json".into(),
        }],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    let target_for_hook = target.clone();
    temporary
        .coordinator()
        .set_project_filesystem_rollback_test_hook(Some(Arc::new(move || {
            std::fs::remove_file(&target_for_hook).unwrap();
            std::fs::write(&target_for_hook, br#"{"external":1}"#).unwrap();
        })));

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&target).unwrap(), br#"{"external":1}"#);
    if case_sensitive {
        assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    } else {
        assert_eq!(std::fs::read(&source).unwrap(), br#"{"external":1}"#);
    }
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    let copies = move_recovery_copies(&temporary);
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(&copies[0]).unwrap(), br#"{"source":1}"#);
}

#[test]
fn case_only_file_move_failed_restoration_preserves_external_paths_for_recovery() {
    let temporary = TestDirectory::new("transaction-case-only-file-move-second-leg");
    let source = temporary.path().join("Report.json");
    let target = temporary.path().join("report.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let hook_source = source.clone();
    let hook_target = target.clone();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(Some(Arc::new(move || {
            std::fs::create_dir(&hook_source).unwrap();
            if !hook_target.exists() {
                std::fs::create_dir(&hook_target).unwrap();
            }
        })));

    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "Report.json".into(),
            to: "report.json".into(),
        }],
    )
    .unwrap()
    .commit()
    .unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(source.is_dir());
    assert!(target.is_dir());
    let temporary_move = std::fs::read_dir(temporary.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().contains("yssbi-move"))
        })
        .expect("recovery must retain the internal source file");
    assert_eq!(std::fs::read(temporary_move).unwrap(), br#"{"source":1}"#);
}

#[test]
fn case_only_file_move_uses_internal_temporary_path() {
    let temporary = TestDirectory::new("transaction-case-only-file-move");
    std::fs::write(temporary.path().join("Report.json"), br#"{"source":1}"#).unwrap();
    let mutation = StagedFilesystemMutation::MoveFile {
        from: "Report.json".into(),
        to: "report.json".into(),
    };
    let public_debug = format!("{mutation:?}");
    let committed = prepare_json_transaction(&temporary, vec![mutation])
        .unwrap()
        .commit()
        .unwrap();
    committed.finalize();

    let names = std::fs::read_dir(temporary.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == "report.json"));
    assert!(!names.iter().any(|name| name == "Report.json"));
    assert!(!names.iter().any(|name| name.contains("yssbi-move")));
    assert_eq!(
        public_debug,
        "MoveFile { from: \"Report.json\", to: \"report.json\" }"
    );
}

#[test]
fn file_move_preserves_target_created_after_prepare_and_restores_source() {
    let temporary = TestDirectory::new("transaction-file-move-commit-target-race");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
    )
    .unwrap();
    let target_for_hook = target.clone();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(Some(Arc::new(move || {
            std::fs::write(&target_for_hook, br#"{"external":1}"#).unwrap();
        })));

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"external":1}"#);
}

#[test]
fn file_move_source_removal_failure_retains_target_and_marks_recovery() {
    let temporary = TestDirectory::new("transaction-file-move-source-removal-failure");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let prepared = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
        marker.clone(),
    )
    .unwrap();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::MoveSourceRemoval));

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
}

#[test]
fn file_move_cleanup_never_deletes_target_replaced_after_identity_check() {
    let temporary = TestDirectory::new("transaction-file-move-cleanup-target-race");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
    )
    .unwrap();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::MoveSourceRemoval));
    let target_for_hook = target.clone();
    temporary
        .coordinator()
        .set_before_move_target_delete_hook(Some(Arc::new(move || {
            std::fs::remove_file(&target_for_hook).unwrap();
            std::fs::write(&target_for_hook, br#"{"external":1}"#).unwrap();
        })));

    let error = prepared.commit().unwrap_err();

    assert_eq!(std::fs::read(&target).unwrap(), br#"{"external":1}"#);
    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
}

#[test]
fn file_move_target_cleanup_failure_requires_recovery() {
    let temporary = TestDirectory::new("transaction-file-move-target-cleanup-failure");
    let source = temporary.path().join("source.json");
    let target = temporary.path().join("destination.json");
    std::fs::write(&source, br#"{"source":1}"#).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "destination.json".into(),
        }],
    )
    .unwrap();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::MoveTargetCleanup));

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert_eq!(std::fs::read(&source).unwrap(), br#"{"source":1}"#);
    assert_eq!(std::fs::read(&target).unwrap(), br#"{"source":1}"#);
}

#[test]
fn file_move_rejects_existing_portable_conflict() {
    let temporary = TestDirectory::new("transaction-file-move-portable-conflict");
    std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();
    std::fs::write(temporary.path().join("report.json"), br#"{"conflict":1}"#).unwrap();

    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::MoveFile {
            from: "source.json".into(),
            to: "Report.json".into(),
        }],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(error.to_string().contains("portable conflict"));
    assert!(temporary.path().join("source.json").is_file());
    assert!(temporary.path().join("report.json").is_file());
}

#[test]
fn prepare_rejects_generic_portable_alias_of_move_target() {
    let temporary = TestDirectory::new("transaction-move-target-generic-portable-alias");
    std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();

    let error = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::MoveFile {
                from: "source.json".into(),
                to: "Report.json".into(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "report.json".into(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(error.to_string().contains("portable path"));
    assert_eq!(
        std::fs::read(temporary.path().join("source.json")).unwrap(),
        br#"{"source":1}"#
    );
}

#[test]
fn prepare_rejects_generic_portable_alias_of_move_source() {
    let temporary = TestDirectory::new("transaction-move-source-generic-portable-alias");
    std::fs::write(temporary.path().join("Report.json"), br#"{"source":1}"#).unwrap();

    let error = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::MoveFile {
                from: "Report.json".into(),
                to: "destination.json".into(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "report.json".into(),
                contents: br#"{"generic":1}"#.to_vec(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(error.to_string().contains("portable path"));
    assert_eq!(
        std::fs::read(temporary.path().join("Report.json")).unwrap(),
        br#"{"source":1}"#
    );
}

#[test]
fn prepare_allows_one_case_only_portable_rewrite_pair() {
    let temporary = TestDirectory::new("transaction-case-only-portable-rewrite-pair");
    std::fs::write(temporary.path().join("report.json"), br#"{"source":1}"#).unwrap();

    let prepared = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "Report.json".into(),
                contents: br#"{"rewritten":1}"#.to_vec(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "unrelated.json".into(),
                contents: br#"{"unrelated":1}"#.to_vec(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "report.json".into(),
            },
        ],
    )
    .unwrap();

    assert!(
        prepared
            .staging_root()
            .join("prepared/Report.json")
            .is_file()
    );
    assert!(
        prepared
            .staging_root()
            .join("prepared/unrelated.json")
            .is_file()
    );
}

#[test]
fn prepare_rejects_other_portable_rewrite_pair_shapes() {
    let cases = [
        (
            "same-path",
            vec![
                StagedFilesystemMutation::Write {
                    relative_path: "report.json".into(),
                    contents: br#"{"rewritten":1}"#.to_vec(),
                },
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "report.json".into(),
                },
            ],
        ),
        (
            "reverse-order",
            vec![
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "report.json".into(),
                },
                StagedFilesystemMutation::Write {
                    relative_path: "Report.json".into(),
                    contents: br#"{"rewritten":1}"#.to_vec(),
                },
            ],
        ),
        (
            "two-writes",
            vec![
                StagedFilesystemMutation::Write {
                    relative_path: "Report.json".into(),
                    contents: br#"{"rewritten":1}"#.to_vec(),
                },
                StagedFilesystemMutation::Write {
                    relative_path: "report.json".into(),
                    contents: br#"{"rewritten":2}"#.to_vec(),
                },
            ],
        ),
        (
            "two-removes",
            vec![
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "Report.json".into(),
                },
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "report.json".into(),
                },
            ],
        ),
        (
            "third-owner",
            vec![
                StagedFilesystemMutation::Write {
                    relative_path: "Report.json".into(),
                    contents: br#"{"rewritten":1}"#.to_vec(),
                },
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "report.json".into(),
                },
                StagedFilesystemMutation::CreateDirectory {
                    relative_path: "REPORT.JSON".into(),
                },
            ],
        ),
    ];

    for (name, mutations) in cases {
        let temporary =
            TestDirectory::new(&format!("transaction-portable-rewrite-pair-reject-{name}"));
        std::fs::write(temporary.path().join("report.json"), br#"{"source":1}"#).unwrap();

        let error = prepare_json_transaction(&temporary, mutations).unwrap_err();

        assert_eq!(error.code(), "transaction_prepare_failed", "{name}");
        assert!(error.to_string().contains("portable path"), "{name}");
    }
}

#[test]
fn portable_owner_matrix_rejects_generic_alias_before_move() {
    let temporary = TestDirectory::new("transaction-portable-owner-generic-before-move");
    std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();

    let error = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "Report.json".into(),
                contents: br#"{"generic":1}"#.to_vec(),
            },
            StagedFilesystemMutation::MoveFile {
                from: "source.json".into(),
                to: "report.json".into(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(error.to_string().contains("portable path"));
}

#[test]
fn portable_owner_matrix_rejects_move_after_completed_rewrite_pair() {
    let temporary = TestDirectory::new("transaction-portable-owner-pair-before-move");
    std::fs::write(temporary.path().join("report.json"), br#"{"old":1}"#).unwrap();
    std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();

    let error = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "Report.json".into(),
                contents: br#"{"rewritten":1}"#.to_vec(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "report.json".into(),
            },
            StagedFilesystemMutation::MoveFile {
                from: "source.json".into(),
                to: "REPORT.JSON".into(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(error.to_string().contains("portable path"));
}

#[test]
fn portable_owner_matrix_allows_two_independent_rewrite_pairs() {
    let temporary = TestDirectory::new("transaction-portable-owner-two-pairs");
    std::fs::write(temporary.path().join("report.json"), br#"{"old":1}"#).unwrap();
    std::fs::write(temporary.path().join("budget.json"), br#"{"old":2}"#).unwrap();

    let prepared = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "Report.json".into(),
                contents: br#"{"rewritten":1}"#.to_vec(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "Budget.json".into(),
                contents: br#"{"rewritten":2}"#.to_vec(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "report.json".into(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "budget.json".into(),
            },
        ],
    )
    .unwrap();

    assert!(
        prepared
            .staging_root()
            .join("prepared/Report.json")
            .is_file()
    );
    assert!(
        prepared
            .staging_root()
            .join("prepared/Budget.json")
            .is_file()
    );
}

#[test]
fn portable_owner_matrix_allows_unicode_full_casefold_rewrite_pair() {
    let temporary = TestDirectory::new("transaction-portable-owner-unicode-pair");
    std::fs::write(temporary.path().join("STRASSE.json"), br#"{"source":1}"#).unwrap();

    let prepared = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "Straße.json".into(),
                contents: br#"{"rewritten":1}"#.to_vec(),
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: "STRASSE.json".into(),
            },
        ],
    )
    .unwrap();

    assert!(
        prepared
            .staging_root()
            .join("prepared/Straße.json")
            .is_file()
    );
}

#[test]
fn portable_owner_matrix_rejects_unicode_move_alias_in_both_orders() {
    let cases = [
        (
            "generic-before-move",
            vec![
                StagedFilesystemMutation::Write {
                    relative_path: "Straße.json".into(),
                    contents: br#"{"generic":1}"#.to_vec(),
                },
                StagedFilesystemMutation::MoveFile {
                    from: "source.json".into(),
                    to: "STRASSE.json".into(),
                },
            ],
        ),
        (
            "move-before-generic",
            vec![
                StagedFilesystemMutation::MoveFile {
                    from: "source.json".into(),
                    to: "Straße.json".into(),
                },
                StagedFilesystemMutation::RemoveFile {
                    relative_path: "STRASSE.json".into(),
                },
            ],
        ),
    ];

    for (name, mutations) in cases {
        let temporary =
            TestDirectory::new(&format!("transaction-portable-owner-unicode-move-{name}"));
        std::fs::write(temporary.path().join("source.json"), br#"{"source":1}"#).unwrap();

        let error = prepare_json_transaction(&temporary, mutations).unwrap_err();

        assert_eq!(error.code(), "transaction_prepare_failed", "{name}");
        assert!(error.to_string().contains("portable path"), "{name}");
    }
}

#[test]
fn prepare_serializes_every_document_before_touching_live_files() {
    let temporary = TestDirectory::new("transaction-prepare-atomic");
    std::fs::write(temporary.path().join("first.json"), br#"{"live":1}"#).unwrap();
    std::fs::write(temporary.path().join("second.json"), br#"{"live":2}"#).unwrap();

    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::StagedSerialization));
    let error = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "first.json".into(),
                contents: br#"{"prepared":1}"#.to_vec(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "second.json".into(),
                contents: br#"{"prepared":2}"#.to_vec(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.code(), "transaction_prepare_failed");
    assert_eq!(
        std::fs::read(temporary.path().join("first.json")).unwrap(),
        br#"{"live":1}"#
    );
    assert_eq!(
        std::fs::read(temporary.path().join("second.json")).unwrap(),
        br#"{"live":2}"#
    );
    assert!(!temporary.path().join(".yssbi-transaction").exists());
}

#[test]
fn commit_failure_restores_only_touched_files_and_directory_topology() {
    let temporary = TestDirectory::new("transaction-precise-rollback");
    std::fs::write(temporary.path().join("first.json"), br#"{"live":1}"#).unwrap();
    std::fs::write(temporary.path().join("unrelated.txt"), b"preserve").unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "first.json".into(),
                contents: br#"{"prepared":1}"#.to_vec(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "new/nested.json".into(),
                contents: br#"{"prepared":2}"#.to_vec(),
            },
        ],
    )
    .unwrap();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(
        std::fs::read(temporary.path().join("first.json")).unwrap(),
        br#"{"live":1}"#
    );
    assert!(!temporary.path().join("new").exists());
    assert_eq!(
        std::fs::read(temporary.path().join("unrelated.txt")).unwrap(),
        b"preserve"
    );
}

#[test]
fn generic_directory_topology_check_precedes_destructive_child_restore() {
    let temporary = TestDirectory::new("transaction-generic-directory-topology-order");
    let directory = temporary.path().join("dir");
    let child = directory.join("child.json");
    let external = directory.join("external.json");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(&child, br#"{"original":1}"#).unwrap();
    let marker = ProjectRecoveryMarker::default();
    let committed = prepare_json_transaction_with_recovery_marker(
        &temporary,
        vec![
            StagedFilesystemMutation::RemoveFile {
                relative_path: "dir/child.json".into(),
            },
            StagedFilesystemMutation::RemoveDirectoryIfEmpty {
                relative_path: "dir".into(),
            },
        ],
        marker.clone(),
    )
    .unwrap()
    .commit()
    .unwrap();
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(&child, br#"{"external_child":1}"#).unwrap();
    std::fs::write(&external, br#"{"external_extra":1}"#).unwrap();

    let error = committed.rollback().unwrap_err();

    assert_eq!(std::fs::read(&child).unwrap(), br#"{"external_child":1}"#);
    assert_eq!(
        std::fs::read(&external).unwrap(),
        br#"{"external_extra":1}"#
    );
    assert!(error.recovery_required());
    assert!(marker.error().is_some());
}

#[test]
fn rollback_failure_reports_transaction_rollback_failed_with_recovery_requirement() {
    let temporary = TestDirectory::new("transaction-rollback-failure");
    std::fs::write(temporary.path().join("first.json"), br#"{"live":1}"#).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![
            StagedFilesystemMutation::Write {
                relative_path: "first.json".into(),
                contents: br#"{"prepared":1}"#.to_vec(),
            },
            StagedFilesystemMutation::Write {
                relative_path: "second.json".into(),
                contents: br#"{"prepared":2}"#.to_vec(),
            },
        ],
    )
    .unwrap();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));
    temporary
        .coordinator()
        .set_project_filesystem_rollback_fault(true);

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    temporary
        .coordinator()
        .set_project_filesystem_rollback_fault(false);
}

#[test]
fn staging_directory_is_removed_after_commit_and_rollback() {
    let temporary = TestDirectory::new("transaction-staging-cleanup");
    std::fs::write(temporary.path().join("document.json"), br#"{"live":1}"#).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "document.json".into(),
            contents: br#"{"prepared":1}"#.to_vec(),
        }],
    )
    .unwrap();
    let staging_root = prepared.staging_root().to_path_buf();

    let committed = prepared.commit().unwrap();
    assert!(!staging_root.exists());
    committed.rollback().unwrap();

    assert!(!staging_root.exists());
    assert_eq!(
        std::fs::read(temporary.path().join("document.json")).unwrap(),
        br#"{"live":1}"#
    );

    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "document.json".into(),
            contents: br#"{"prepared":2}"#.to_vec(),
        }],
    )
    .unwrap();
    let staging_root = prepared.staging_root().to_path_buf();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));
    assert_eq!(
        prepared.commit().unwrap_err().code(),
        "transaction_commit_failed"
    );
    assert!(!staging_root.exists());

    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "document.json".into(),
            contents: br#"{"prepared":3}"#.to_vec(),
        }],
    )
    .unwrap();
    let staging_root = prepared.staging_root().to_path_buf();
    temporary
        .coordinator()
        .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::StagingCleanup));
    assert_eq!(
        prepared.commit().unwrap_err().code(),
        "transaction_commit_failed"
    );
    assert!(!staging_root.exists());
    assert_eq!(
        std::fs::read(temporary.path().join("document.json")).unwrap(),
        br#"{"live":1}"#
    );
}

#[cfg(unix)]
#[test]
fn prepare_rejects_symlinked_live_and_staging_ancestors() {
    use std::os::unix::fs::symlink;

    let temporary = TestDirectory::new("transaction-symlink-defense");
    let outside = TestDirectory::new("transaction-symlink-outside");
    symlink(outside.path(), temporary.path().join("linked")).unwrap();
    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "linked/escaped.json".into(),
            contents: br#"{"escaped":true}"#.to_vec(),
        }],
    )
    .unwrap_err();
    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(!outside.path().join("escaped.json").exists());

    symlink(outside.path(), temporary.path().join(".yssbi-transaction")).unwrap();
    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "safe.json".into(),
            contents: br#"{"safe":true}"#.to_vec(),
        }],
    )
    .unwrap_err();
    assert_eq!(error.code(), "transaction_prepare_failed");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn prepare_rejects_reparse_point_live_and_staging_ancestors() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-reparse-defense");
    let outside = TestDirectory::new("transaction-reparse-outside");
    junction(&temporary.path().join("linked"), outside.path());
    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "linked/escaped.json".into(),
            contents: br#"{"escaped":true}"#.to_vec(),
        }],
    )
    .unwrap_err();
    assert_eq!(error.code(), "transaction_prepare_failed");
    assert!(!outside.path().join("escaped.json").exists());

    junction(&temporary.path().join(".yssbi-transaction"), outside.path());
    let error = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "safe.json".into(),
            contents: br#"{"safe":true}"#.to_vec(),
        }],
    )
    .unwrap_err();
    assert_eq!(error.code(), "transaction_prepare_failed");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn commit_rejects_reparse_point_staging_traversal() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-staging-reparse-commit");
    let outside = TestDirectory::new("transaction-staging-reparse-outside");
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "nested/document.json".into(),
            contents: br#"{"prepared":1}"#.to_vec(),
        }],
    )
    .unwrap();
    let staged_parent = prepared.staging_root().join("prepared/nested");
    std::fs::remove_dir_all(&staged_parent).unwrap();
    std::fs::write(outside.path().join("document.json"), br#"{"attacker":1}"#).unwrap();
    junction(&staged_parent, outside.path());

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(
        error.to_string().contains("traverses a redirect"),
        "staged redirect must be rejected before reading it: {error}"
    );
    assert!(!temporary.path().join("nested/document.json").exists());
    assert_eq!(
        std::fs::read(outside.path().join("document.json")).unwrap(),
        br#"{"attacker":1}"#
    );
}

#[cfg(windows)]
#[test]
fn rollback_rejects_reparse_point_live_traversal() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-live-reparse-rollback");
    let outside = TestDirectory::new("transaction-live-reparse-outside");
    let live_parent = temporary.path().join("nested");
    std::fs::create_dir_all(&live_parent).unwrap();
    std::fs::write(live_parent.join("document.json"), br#"{"live":1}"#).unwrap();
    let committed = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "nested/document.json".into(),
            contents: br#"{"prepared":1}"#.to_vec(),
        }],
    )
    .unwrap()
    .commit()
    .unwrap();
    std::fs::remove_file(live_parent.join("document.json")).unwrap();
    std::fs::remove_dir(&live_parent).unwrap();
    std::fs::write(outside.path().join("document.json"), br#"{"attacker":1}"#).unwrap();
    junction(&live_parent, outside.path());

    let error = committed.rollback().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert_eq!(
        std::fs::read(outside.path().join("document.json")).unwrap(),
        br#"{"attacker":1}"#
    );
}

#[cfg(windows)]
#[test]
fn remove_file_revalidates_ancestor_before_commit() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-remove-file-reparse");
    let outside = TestDirectory::new("transaction-remove-file-outside");
    let parent = temporary.path().join("nested");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::write(parent.join("victim.json"), b"inside").unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::RemoveFile {
            relative_path: "nested/victim.json".into(),
        }],
    )
    .unwrap();
    std::fs::remove_file(parent.join("victim.json")).unwrap();
    std::fs::remove_dir(&parent).unwrap();
    std::fs::write(outside.path().join("victim.json"), b"outside").unwrap();
    junction(&parent, outside.path());

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(
        std::fs::read(outside.path().join("victim.json")).unwrap(),
        b"outside"
    );
}

#[cfg(windows)]
#[test]
fn remove_file_revalidates_immediately_before_mutation() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-remove-file-last-check");
    let outside = TestDirectory::new("transaction-remove-file-last-check-outside");
    let parent = temporary.path().join("nested");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::write(parent.join("victim.json"), b"inside").unwrap();
    std::fs::write(outside.path().join("victim.json"), b"outside").unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::RemoveFile {
            relative_path: "nested/victim.json".into(),
        }],
    )
    .unwrap();
    let parent_for_hook = parent.clone();
    let outside_for_hook = outside.path().to_path_buf();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(Some(std::sync::Arc::new(move || {
            std::fs::remove_file(parent_for_hook.join("victim.json")).unwrap();
            std::fs::remove_dir(&parent_for_hook).unwrap();
            junction(&parent_for_hook, &outside_for_hook);
        })));

    let error = prepared.commit().unwrap_err();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(None);

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert_eq!(
        std::fs::read(outside.path().join("victim.json")).unwrap(),
        b"outside"
    );
}

#[cfg(windows)]
#[test]
fn remove_directory_revalidates_immediately_before_mutation() {
    fn junction(link: &Path, target: &Path) {
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

    let temporary = TestDirectory::new("transaction-remove-directory-reparse");
    let outside = TestDirectory::new("transaction-remove-directory-outside");
    let parent = temporary.path().join("container");
    std::fs::create_dir_all(parent.join("empty")).unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::RemoveDirectoryIfEmpty {
            relative_path: "container/empty".into(),
        }],
    )
    .unwrap();
    std::fs::create_dir(outside.path().join("empty")).unwrap();
    let parent_for_hook = parent.clone();
    let outside_for_hook = outside.path().to_path_buf();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(Some(std::sync::Arc::new(move || {
            std::fs::remove_dir(parent_for_hook.join("empty")).unwrap();
            std::fs::remove_dir(&parent_for_hook).unwrap();
            junction(&parent_for_hook, &outside_for_hook);
        })));

    let error = prepared.commit().unwrap_err();
    temporary
        .coordinator()
        .set_before_remove_mutation_hook(None);

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    assert!(outside.path().join("empty").is_dir());
}

#[test]
fn create_directory_rejects_an_existing_file() {
    let temporary = TestDirectory::new("transaction-create-directory-file");
    std::fs::write(temporary.path().join("not-a-directory"), b"preserve").unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::CreateDirectory {
            relative_path: "not-a-directory".into(),
        }],
    )
    .unwrap();

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_commit_failed");
    assert_eq!(
        std::fs::read(temporary.path().join("not-a-directory")).unwrap(),
        b"preserve"
    );
}

#[test]
fn replacement_uses_collision_safe_create_new_file() {
    let temporary = TestDirectory::new("transaction-replacement-create-new");
    let live = temporary.path().join("document.json");
    let predictable = temporary.path().join(".document.json.yssbi-replacement-0");
    std::fs::write(&live, br#"{"live":1}"#).unwrap();
    std::fs::write(&predictable, b"attacker-owned").unwrap();
    let prepared = prepare_json_transaction(
        &temporary,
        vec![StagedFilesystemMutation::Write {
            relative_path: "document.json".into(),
            contents: br#"{"prepared":1}"#.to_vec(),
        }],
    )
    .unwrap();

    prepared.commit().unwrap().finalize();

    assert_eq!(std::fs::read(live).unwrap(), br#"{"prepared":1}"#);
    assert_eq!(std::fs::read(predictable).unwrap(), b"attacker-owned");
    assert_eq!(
        std::fs::read_dir(temporary.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains("yssbi-replacement"))
            .count(),
        1
    );
}
