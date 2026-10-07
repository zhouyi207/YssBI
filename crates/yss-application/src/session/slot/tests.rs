use super::recovery::RecoveryWorkState;
use super::replacement::ReplacementAdvanceOutcome;
use super::*;
use std::num::NonZeroU64;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::error::DatabaseOperation;
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_graph_execution::identity::ExecutionSessionId;
use yss_graph_execution::identity::RuntimeGeneration;
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::GraphRuntimeState;
use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch};
use yss_node_catalog::build_builtin_node_system;
use yss_project::ProjectState;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

fn session(epoch: u64) -> Arc<ApplicationSession> {
    let project_session_id = ProjectSessionId::new(format!("session-{epoch}"));
    let execution_session_id = ExecutionSessionId::new(uuid::Uuid::from_u128(epoch as u128));
    let project = Arc::new(ProjectState::new());
    let builtin = build_builtin_node_system().expect("test built-ins are valid");
    let graph = Arc::new(
        GraphRuntimeState::from_components(
            GraphRuntimeEpoch::from_existing(epoch),
            GraphRuntimeComponents {
                registry: builtin.registry,
                catalog: builtin.catalog,
            },
        )
        .unwrap(),
    );
    let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
        DatabaseId,
        DatabaseDeclarationObservation,
    )>())
    .expect("empty observation set is valid");
    let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
    let database = Arc::new(
        DatabaseRuntimeRegistry::new()
            .open_session(DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                NonZeroU64::new(1).expect("non-zero test generation"),
                declarations,
                observations,
            ))
            .expect("empty database session is valid"),
    );
    let execution = Arc::new(ExecutionRuntimeState::new(
        execution_session_id,
        RuntimeGeneration::from_existing(epoch),
        yss_node_kernel::KernelRegistry::default().into(),
        yss_database_runtime::dataset_query_engine().unwrap(),
    ));
    let resource_provider_factory = Arc::new(ResourceProviderFactory::new(
        project_session_id.as_str().into(),
    ));
    Arc::new(ApplicationSession::new_for_test(
        ApplicationSessionEpoch::from_existing(epoch),
        ProjectInstanceId::from_existing(format!("project-{epoch}")),
        project_session_id,
        execution_session_id,
        RuntimeGeneration::from_existing(epoch),
        project,
        graph,
        execution,
        database,
        resource_provider_factory,
    ))
}

#[test]
fn capture_and_revalidate_use_one_session_envelope() {
    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Inactive)
    ));

    let first = session(1);
    slot.publish_for_test(Arc::clone(&first));
    let captured = slot
        .capture_session()
        .expect("active session is capturable");
    assert!(Arc::ptr_eq(&captured, &first));
    assert_eq!(captured.project_session_id().as_str(), "session-1");
    assert_eq!(captured.graph().epoch().get(), 1);
    assert_eq!(
        captured.execution_session_id(),
        captured.execution().session_id()
    );
    assert_eq!(
        captured.runtime_generation(),
        captured.execution().generation()
    );
    assert!(slot.revalidate_captured_session(&captured).is_ok());

    let second = session(2);
    slot.publish_for_test(Arc::clone(&second));
    assert!(matches!(
        slot.revalidate_captured_session(&captured),
        Err(SessionRevalidationError::Changed)
    ));
}

#[test]
fn stale_refresh_cannot_revoke_the_replacement_session() {
    let application = ApplicationState::initialize().expect("application initializes");
    let captured = application.capture_session().unwrap();
    application.rebuild_application_session(&captured).unwrap();
    let current = application.capture_session().unwrap();
    assert!(!Arc::ptr_eq(&captured, &current));

    let result = application.rebuild_application_session(&captured);
    assert!(matches!(
        result,
        Err(ApplicationSessionRefreshError::Replacement)
    ));
    assert!(Arc::ptr_eq(
        &application.capture_session().unwrap(),
        &current
    ));
    assert!(current.execution().admit().is_ok());
    assert!(
        current
            .database()
            .admit_operation(DatabaseOperation::Query)
            .is_ok()
    );
}

#[test]
fn replacement_observer_can_capture_the_published_session() {
    let application = ApplicationState::initialize().expect("application initializes");
    let old = application
        .capture_session()
        .expect("initial session is active");
    let expected_epoch = old.epoch().next().expect("replacement epoch exists");
    let slot = Arc::downgrade(&application.session_slot);
    let (sent, received) = std::sync::mpsc::channel();
    let _subscription = old
        .presentation
        .subscribe(Arc::new(move |event| {
            assert!(matches!(event, yss_ui_contract::UiEvent::SessionChanged));
            let slot = slot.upgrade().expect("application still owns its slot");
            // A blocking capture would deadlock if publication kept the write lock.
            let unlocked = slot.inner.state.try_read().is_ok();
            let epoch = unlocked.then(|| {
                slot.capture_session()
                    .expect("replacement is active before notification")
                    .epoch()
            });
            sent.send(epoch).expect("test receiver remains alive");
        }))
        .expect("old session accepts an observer");

    application
        .rebuild_application_session(&old)
        .expect("replacement publishes successfully");

    assert_eq!(
        received.try_recv().expect("observer was notified"),
        Some(expected_epoch)
    );
    assert_eq!(
        application.revalidate_captured_session(&old),
        Err(SessionRevalidationError::Changed)
    );
}

#[test]
fn stale_replacement_worker_stops_after_recovery_supersedes_its_phase() {
    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    let old = session(1);
    slot.publish_for_test(Arc::clone(&old));
    let mut worker = slot
        .begin_replacement_for_test(&old)
        .expect("active slot replaces");
    let required = slot.install_recovery_for_test(
        Arc::clone(&old),
        worker.epoch,
        SessionRecoveryPhase::DrainOldExecution,
    );

    assert_eq!(
        worker
            .complete_phase_for_test(ReplacementPhase::CloseAdmissions)
            .expect("superseded worker reports retained recovery"),
        ReplacementAdvanceOutcome::Superseded(required)
    );
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Recovering)
    ));
    let state = slot
        .inner
        .state
        .read()
        .unwrap_or_else(|error| error.into_inner());
    assert!(matches!(
        &*state,
        SessionSlotState::Recovering {
            epoch,
            recovery,
            retained: RetainedSessionRecovery {
                old: retained,
                work: RecoveryWorkState::Available(SessionRecoveryPhase::DrainOldExecution),
            },
        } if *epoch == worker.epoch
            && *recovery == required.recovery
            && Arc::ptr_eq(retained, &old)
    ));
}

#[test]
fn recovery_claim_is_single_owner_and_drop_reinstalls_exact_work() {
    let slot = Arc::new(ApplicationSessionSlot::new(
        crate::session::NodeComponents::builtins().unwrap(),
    ));
    let old = session(1);
    let required = slot.install_recovery_for_test(
        Arc::clone(&old),
        ApplicationSessionEpoch::from_existing(2),
        SessionRecoveryPhase::ResolveOldDatabase,
    );
    let application = ApplicationState::new(Arc::clone(&slot));
    let control = SessionRecoveryControl::new(SessionRecoveryDeadline::at(
        Instant::now() + Duration::from_secs(1),
    ));
    assert!(matches!(
        application.retry_session_recovery(required.recovery, &control),
        Err(SessionRecoveryError::WrongPhase)
    ));
    let first = slot
        .claim_recovery(required.recovery)
        .expect("first recovery claimant owns the work");
    let barrier = Arc::new(Barrier::new(2));
    let concurrent_slot = Arc::clone(&slot);
    let concurrent_barrier = Arc::clone(&barrier);
    let recovery = required.recovery;
    let second = thread::spawn(move || {
        concurrent_barrier.wait();
        concurrent_slot.claim_recovery(recovery)
    });
    barrier.wait();
    assert!(matches!(
        second.join().expect("claim thread completes"),
        Err(SessionRecoveryError::AlreadyInProgress)
    ));
    drop(first);

    let state = slot
        .inner
        .state
        .read()
        .unwrap_or_else(|error| error.into_inner());
    assert!(matches!(
        &*state,
        SessionSlotState::Recovering {
            recovery: current,
            retained: RetainedSessionRecovery {
                old: retained,
                work: RecoveryWorkState::Available(SessionRecoveryPhase::ResolveOldDatabase),
            },
            ..
        } if *current == recovery && Arc::ptr_eq(retained, &old)
    ));
    drop(state);

    let second_claim = slot
        .claim_recovery(recovery)
        .expect("reinstalled recovery can be claimed again");
    assert!(Arc::ptr_eq(&second_claim.old, &old));
    assert_eq!(
        second_claim.phase(),
        SessionRecoveryPhase::ResolveOldDatabase
    );
    drop(second_claim);

    assert_eq!(
        slot.complete_recovery_phase_for_test(
            recovery,
            Some(SessionRecoveryPhase::DrainOldDatabase),
        )
        .expect("recovery advances with the same retained owner"),
        SessionRecoveryOutcome::RetryRequired(RecoveryRequired {
            recovery,
            failed_epoch: ApplicationSessionEpoch::from_existing(2),
            phase: SessionRecoveryPhase::DrainOldDatabase,
        })
    );
    let terminal = slot
        .complete_recovery_phase_for_test(recovery, None)
        .expect("terminal recovery consumes the retained owner");
    assert_eq!(
        terminal,
        SessionRecoveryOutcome::ReplacementMayRestart {
            next_epoch: ApplicationSessionEpoch::from_existing(3),
        }
    );
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Inactive)
    ));
}

#[test]
fn non_active_capture_errors_remain_fieldless() {
    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    slot.set_replacing_for_test(
        ApplicationSessionEpoch::from_existing(1),
        ReplacementPhase::DrainDatabase,
    );
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Replacing)
    ));
    slot.set_recovering_for_test(
        ApplicationSessionEpoch::from_existing(1),
        SessionRecoveryId::from_existing(1),
        session(1),
        SessionRecoveryPhase::ClearOldProject,
    );
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Recovering)
    ));
}
