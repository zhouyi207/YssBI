use super::{
    NormalizedProjectRoot, ProjectFilesystemCoordinator, ProjectFilesystemFaultPoint,
    ProjectFilesystemTransaction, StagedFilesystemMutation, set_before_remove_mutation_hook,
    set_project_filesystem_fault, set_project_filesystem_rollback_fault,
};
use crate::node_system::document::OperationId;
use crate::project::{
    PROJECT_METADATA_FILE, ProjectInstanceId, ProjectSession, ProjectTransactionContext,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("yssbi-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

fn prepare_json_transaction(
    temporary: &TestDirectory,
    mutations: Vec<StagedFilesystemMutation>,
) -> Result<super::PreparedProjectFilesystemTransaction, super::ProjectFilesystemError> {
    let root = normalized(temporary.path());
    let lease = ProjectFilesystemCoordinator::default()
        .acquire(root.clone())
        .unwrap();
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

#[test]
fn equivalent_existing_and_missing_root_spellings_share_one_lease() {
    let temporary = TestDirectory::new("filesystem-normalization");
    let existing = temporary.path().join("existing");
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(existing.join(PROJECT_METADATA_FILE), "{}").unwrap();

    let existing_direct = normalized(&existing);
    let existing_dotted = normalized(existing.join(".").join("child").join(".."));
    assert_eq!(existing_direct, existing_dotted);

    let missing_direct = normalized(temporary.path().join("destination"));
    let missing_dotted = normalized(
        temporary
            .path()
            .join("missing-parent")
            .join("..")
            .join("destination"),
    );
    assert_eq!(missing_direct, missing_dotted);

    #[cfg(windows)]
    {
        let slash_spelling = existing.to_string_lossy().replace('\\', "/").to_uppercase();
        assert_eq!(existing_direct, normalized(slash_spelling));
    }

    let coordinator = ProjectFilesystemCoordinator::default();
    let lease = coordinator.acquire(missing_direct).unwrap();
    let contender = coordinator.clone();
    let contender_root = missing_dotted;
    let (finished_tx, finished_rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _lease = contender.acquire(contender_root).unwrap();
        finished_tx.send(()).unwrap();
    });

    assert!(
        finished_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    drop(lease);
    finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    handle.join().unwrap();
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
fn reverse_order_multi_root_acquisition_is_sorted_deduplicated_and_deadlock_free() {
    let temporary = TestDirectory::new("filesystem-multi-root");
    let root_a = normalized(temporary.path().join("a"));
    let root_b = normalized(temporary.path().join("b"));
    let expected = vec![root_a.clone(), root_b.clone()];
    let coordinator = ProjectFilesystemCoordinator::default();
    let barrier = Arc::new(Barrier::new(3));
    let (finished_tx, finished_rx) = mpsc::channel();

    let spawn_acquire = |roots: Vec<NormalizedProjectRoot>| {
        let coordinator = coordinator.clone();
        let barrier = Arc::clone(&barrier);
        let finished_tx = finished_tx.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let lease = coordinator.acquire_many(roots).unwrap();
            finished_tx.send(lease.roots().to_vec()).unwrap();
            std::thread::sleep(Duration::from_millis(25));
        })
    };

    let first = spawn_acquire(vec![root_a.clone(), root_b.clone(), root_a]);
    let second = spawn_acquire(vec![root_b.clone(), root_b, expected[0].clone()]);
    barrier.wait();

    assert_eq!(
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        expected
    );
    assert_eq!(
        finished_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        expected
    );
    first.join().unwrap();
    second.join().unwrap();
}

#[test]
fn blocked_multi_root_acquisition_never_partially_reserves_a_free_root() {
    let temporary = TestDirectory::new("filesystem-atomic-admission");
    let root_a = normalized(temporary.path().join("a"));
    let root_b = normalized(temporary.path().join("b"));
    let coordinator = ProjectFilesystemCoordinator::default();
    let held_b = coordinator.acquire(root_b.clone()).unwrap();
    let waiter_blocked = coordinator.observe_next_wait();
    let (multi_finished_tx, multi_finished_rx) = mpsc::channel();

    let multi_coordinator = coordinator.clone();
    let multi_a = root_a.clone();
    let multi_b = root_b;
    let multi = std::thread::spawn(move || {
        let _lease = multi_coordinator.acquire_many([multi_a, multi_b]).unwrap();
        multi_finished_tx.send(()).unwrap();
    });
    waiter_blocked.recv_timeout(Duration::from_secs(2)).unwrap();

    let independent_coordinator = coordinator.clone();
    let (independent_tx, independent_rx) = mpsc::channel();
    let independent = std::thread::spawn(move || {
        let _lease = independent_coordinator.acquire(root_a).unwrap();
        independent_tx.send(()).unwrap();
    });
    independent_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        multi_finished_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );

    drop(held_b);
    multi_finished_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    independent.join().unwrap();
    multi.join().unwrap();
}

#[test]
fn lease_set_releases_roots_in_reverse_order() {
    let temporary = TestDirectory::new("filesystem-release-order");
    let roots = vec![
        normalized(temporary.path().join("a")),
        normalized(temporary.path().join("b")),
        normalized(temporary.path().join("c")),
    ];
    let coordinator = ProjectFilesystemCoordinator::default();

    let lease = coordinator.acquire_many(roots.clone()).unwrap();
    assert!(roots.iter().all(|root| lease.contains(root)));
    drop(lease);

    let mut expected = roots;
    expected.reverse();
    assert_eq!(coordinator.release_trace(), expected);
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
fn lifecycle_drain_waits_for_previously_admitted_multi_root_operation() {
    let temporary = TestDirectory::new("filesystem-lifecycle-drain");
    let root_a = normalized(temporary.path().join("a"));
    let root_b = normalized(temporary.path().join("b"));
    let coordinator = ProjectFilesystemCoordinator::default();
    let held_b = coordinator.acquire(root_b.clone()).unwrap();
    let admitted_waiting = coordinator.observe_next_wait();
    let (ordinary_done_tx, ordinary_done_rx) = mpsc::channel();

    let ordinary_coordinator = coordinator.clone();
    let ordinary_a = root_a.clone();
    let ordinary = std::thread::spawn(move || {
        let lease = ordinary_coordinator
            .acquire_many([ordinary_a, root_b])
            .unwrap();
        ordinary_done_tx.send(()).unwrap();
        drop(lease);
    });
    admitted_waiting
        .recv_timeout(Duration::from_secs(2))
        .unwrap();

    let mut lifecycle = coordinator.begin_root_lifecycle(root_a.clone()).unwrap();
    assert_eq!(
        coordinator.acquire(root_a.clone()).err().unwrap().code(),
        "project_lifecycle_admission_closed"
    );
    let (drained_tx, drained_rx) = mpsc::channel();
    let drain = std::thread::spawn(move || {
        lifecycle.release_initial_and_drain();
        drained_tx.send(()).unwrap();
        lifecycle
    });

    assert!(drained_rx.recv_timeout(Duration::from_millis(100)).is_err());
    drop(held_b);
    ordinary_done_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    ordinary.join().unwrap();
    drained_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    let mut lifecycle = drain.join().unwrap();
    lifecycle.acquire_final().unwrap();
    assert!(lifecycle.holds_lease());
    drop(lifecycle);
}

#[test]
fn prepare_serializes_every_document_before_touching_live_files() {
    let temporary = TestDirectory::new("transaction-prepare-atomic");
    std::fs::write(temporary.path().join("first.json"), br#"{"live":1}"#).unwrap();
    std::fs::write(temporary.path().join("second.json"), br#"{"live":2}"#).unwrap();

    set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::StagedSerialization));
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
    set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));

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
    set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));
    set_project_filesystem_rollback_fault(true);

    let error = prepared.commit().unwrap_err();

    assert_eq!(error.code(), "transaction_rollback_failed");
    assert!(error.recovery_required());
    set_project_filesystem_rollback_fault(false);
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
    set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));
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
    set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::StagingCleanup));
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
    set_before_remove_mutation_hook(Some(std::sync::Arc::new(move || {
        std::fs::remove_file(parent_for_hook.join("victim.json")).unwrap();
        std::fs::remove_dir(&parent_for_hook).unwrap();
        junction(&parent_for_hook, &outside_for_hook);
    })));

    let error = prepared.commit().unwrap_err();
    set_before_remove_mutation_hook(None);

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
    set_before_remove_mutation_hook(Some(std::sync::Arc::new(move || {
        std::fs::remove_dir(parent_for_hook.join("empty")).unwrap();
        std::fs::remove_dir(&parent_for_hook).unwrap();
        junction(&parent_for_hook, &outside_for_hook);
    })));

    let error = prepared.commit().unwrap_err();
    set_before_remove_mutation_hook(None);

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
