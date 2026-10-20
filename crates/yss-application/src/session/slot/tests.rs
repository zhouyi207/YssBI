use super::*;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Instant;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::error::DatabaseOperation;
use yss_database_runtime::runtime::{DatabaseRuntimeRegistry, DatabaseRuntimeSession};
use yss_database_runtime::session_api::{
    DatabaseDeclarationTransition, DatabaseMutationOperation, DatabaseMutationRequest,
    commit_database_runtime_change, prepare_database_runtime_change,
};
use yss_database_runtime::test_support::{DatasetFixture, SALES_ID};
use yss_database_schema::DatabaseRuntimeRevision;
use yss_graph_execution::identity::ExecutionSessionId;
use yss_graph_execution::identity::RuntimeGeneration;
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::GraphRuntimeState;
use yss_graph_runtime::{GraphRuntimeComponents, GraphRuntimeEpoch};
use yss_node_catalog::build_builtin_node_system;
use yss_project::ProjectState;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

fn empty_database(identity: &str) -> DatabaseRuntimeSession {
    let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
        DatabaseId,
        DatabaseDeclarationObservation,
    )>())
    .expect("empty observation set is valid");
    let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
    DatabaseRuntimeRegistry::new()
        .open_session(DatabaseSessionOpenRequest::new(
            DatabaseSessionIdentity::from_existing(identity.into()),
            NonZeroU64::new(1).expect("non-zero test generation"),
            declarations,
            observations,
        ))
        .expect("empty database session is valid")
}

fn session(epoch: u64) -> Arc<ApplicationSession> {
    session_with_database(epoch, empty_database(&format!("session-{epoch}")))
}

fn session_with_database(epoch: u64, database: DatabaseRuntimeSession) -> Arc<ApplicationSession> {
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
        Arc::new(database),
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
            let unlocked = slot.state.try_read().is_ok();
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
fn undrained_replacement_is_recovered_from_its_pending_phase() {
    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    let old = session(1);
    slot.publish_for_test(Arc::clone(&old));
    let running = old.execution().admit().expect("old session admits work");

    assert!(matches!(
        slot.begin_project_replacement(&old, Instant::now()),
        Err(SessionReplacementError::RecoveryRequired)
    ));
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Recovering)
    ));
    assert!(matches!(
        slot.recover(Instant::now()),
        Err(SessionRecoveryError::DeadlineElapsed {
            phase: SessionRecoveryPhase::DrainOldExecution,
        })
    ));
    assert!(matches!(
        &*slot.state.read().unwrap_or_else(|error| error.into_inner()),
        SessionSlotState::Recovering {
            epoch,
            old: retained,
            pending: SessionRecoveryPhase::DrainOldExecution,
        } if epoch.get() == 3 && Arc::ptr_eq(retained, &old)
    ));

    let (sent, received) = std::sync::mpsc::channel();
    let _subscription = old
        .presentation
        .subscribe(Arc::new(move |event| {
            sent.send(matches!(event, yss_ui_contract::UiEvent::SessionChanged))
                .expect("test receiver remains alive");
        }))
        .expect("old session accepts an observer");
    drop(running);
    assert!(
        slot.recover(Instant::now())
            .expect("drained recovery publishes a session")
    );
    let recovered = slot.capture_session().expect("recovered session is active");
    assert_eq!(recovered.epoch().get(), 4);
    assert!(recovered.project().get_path().is_none());
    assert_eq!(received.try_recv(), Ok(true));
    assert!(
        !slot
            .recover(Instant::now())
            .expect("an active slot has nothing to recover")
    );
}

#[test]
fn abandoned_storage_handoff_does_not_block_replacement() {
    let fixture = DatasetFixture::single_i64(SALES_ID, "value", vec![1]);
    let declaration = fixture.instance.decl.clone();
    let observation = DatabaseDeclarationObservation::new(
        DatabaseDeclarationRevision::from_existing(1),
        DatabaseDeclarationFingerprint::from_decl(&declaration),
    );
    let database = DatabaseRuntimeRegistry::new()
        .open_session_with_instances(
            DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing("session-1".into()),
                NonZeroU64::new(1).expect("non-zero test generation"),
                vec![declaration.clone()].into(),
                DatabaseDeclarationObservationSet::try_from_iter([(
                    declaration.id.clone(),
                    observation.clone(),
                )])
                .expect("observation set is valid"),
            ),
            [fixture.instance.clone()],
        )
        .expect("database session is valid");
    let physical = database
        .prepare_physical_mutation(
            &declaration.id,
            &DatabaseMutationOperation::EditCell {
                row: 0,
                column: "value".into(),
                value: yss_data_contract::TabularScalar::Null,
                row_id: None,
            },
            "abandoned-handoff",
        )
        .expect("mutation prepares");
    let prepared = prepare_database_runtime_change(
        &database,
        DatabaseMutationRequest::new(
            declaration.id.clone(),
            DatabaseRuntimeRevision::from_existing(0),
            DatabaseDeclarationTransition {
                expected: observation.clone(),
                next: observation,
            },
        ),
        &physical,
    )
    .expect("runtime change prepares");
    drop(commit_database_runtime_change(&database, prepared).expect("runtime change commits"));
    drop(physical);

    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    let old = session_with_database(1, database);
    slot.publish_for_test(Arc::clone(&old));
    let replacement = slot
        .begin_project_replacement(&old, Instant::now())
        .expect("resolving the abandoned handoff lets the database drain");
    assert!(
        !slot
            .recover(Instant::now())
            .expect("an in-flight replacement is not a recovery")
    );
    slot.finish_project_replacement(replacement)
        .expect("drained replacement publishes");
    assert_eq!(slot.capture_session().unwrap().epoch().get(), 2);
}

#[test]
fn non_active_capture_errors_remain_fieldless() {
    let slot = ApplicationSessionSlot::new(crate::session::NodeComponents::builtins().unwrap());
    *slot
        .state
        .write()
        .unwrap_or_else(|error| error.into_inner()) = SessionSlotState::Replacing {
        epoch: ApplicationSessionEpoch::from_existing(1),
    };
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Replacing)
    ));
    *slot
        .state
        .write()
        .unwrap_or_else(|error| error.into_inner()) = SessionSlotState::Recovering {
        epoch: ApplicationSessionEpoch::from_existing(1),
        old: session(1),
        pending: SessionRecoveryPhase::ClearOldProject,
    };
    assert!(matches!(
        slot.capture_session(),
        Err(SessionCaptureError::Recovering)
    ));
}
