use crate::database::error::{DatabaseErrorCode, DatabaseOperation};
use crate::database::runtime::{
    DatabaseAdmissionCloseOutcome, DatabaseDrainDeadline, DatabaseDrainOutcome,
    DatabaseOutstandingWork, DatabaseRuntimeRegistry, DatabaseSessionDrainControl,
};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};

fn declaration(id: &str, name: &str) -> DatabaseDecl {
    DatabaseDecl {
        id: DatabaseId::from_existing(id.into()),
        engine: DatabaseEngine::InMemory { name: id.into() },
        schema_version: 1,
        required: false,
        name: name.into(),
    }
}

fn observations_for(declarations: &[DatabaseDecl]) -> DatabaseDeclarationObservationSet {
    DatabaseDeclarationObservationSet::try_from_iter(declarations.iter().map(|declaration| {
        (
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(4),
                DatabaseDeclarationFingerprint::from_decl(declaration),
            ),
        )
    }))
    .unwrap()
}

fn request(
    declarations: Vec<DatabaseDecl>,
    observations: DatabaseDeclarationObservationSet,
) -> DatabaseSessionOpenRequest {
    DatabaseSessionOpenRequest::new(
        DatabaseSessionIdentity::from_existing("session-1".into()),
        NonZeroU64::new(7).unwrap(),
        Some(PathBuf::from("project")),
        declarations.into(),
        observations,
    )
}

#[test]
fn database_session_close_and_drain_are_idempotent_and_non_blocking() {
    let registry = DatabaseRuntimeRegistry::new();
    let declarations = vec![declaration("sales", "Sales")];
    let session = registry
        .open_session(request(
            declarations.clone(),
            observations_for(&declarations),
        ))
        .unwrap();

    assert_eq!(session.identity().as_str(), "session-1");
    assert_eq!(session.generation().get(), 7);
    assert_eq!(
        session.close_admission(),
        DatabaseAdmissionCloseOutcome::Closed
    );
    assert_eq!(
        session.close_admission(),
        DatabaseAdmissionCloseOutcome::AlreadyClosed
    );

    let error = session
        .admit_operation(DatabaseOperation::Query)
        .unwrap_err();
    assert_eq!(error.code(), DatabaseErrorCode::AdmissionClosed);

    assert_eq!(
        session.drain(&DatabaseSessionDrainControl::new(
            DatabaseDrainDeadline::at(Instant::now(),)
        )),
        DatabaseDrainOutcome::Drained {
            outstanding: DatabaseOutstandingWork::default(),
        }
    );
}

#[test]
fn database_session_admission_closure_is_session_local() {
    let registry = DatabaseRuntimeRegistry::new();
    let declarations = vec![declaration("sales", "Sales")];
    let observations = observations_for(&declarations);
    let first = registry
        .open_session(request(declarations.clone(), observations.clone()))
        .unwrap();
    let second = registry
        .open_session(request(declarations, observations))
        .unwrap();

    assert_eq!(
        first.close_admission(),
        DatabaseAdmissionCloseOutcome::Closed
    );
    assert_eq!(
        first
            .admit_operation(DatabaseOperation::Query)
            .unwrap_err()
            .code(),
        DatabaseErrorCode::AdmissionClosed
    );
    assert!(second.admit_operation(DatabaseOperation::Query).is_ok());
}

#[test]
fn database_drain_timeout_is_typed_and_does_not_detach_outstanding_work() {
    let registry = DatabaseRuntimeRegistry::new();
    let declarations = vec![declaration("sales", "Sales")];
    let session = registry
        .open_session(request(
            declarations.clone(),
            observations_for(&declarations),
        ))
        .unwrap();
    let lease = session.admit_operation(DatabaseOperation::Query).unwrap();

    let outcome = session.drain(&DatabaseSessionDrainControl::new(
        DatabaseDrainDeadline::at(Instant::now()),
    ));
    assert!(matches!(
        outcome,
        DatabaseDrainOutcome::TimedOut { outstanding }
            if outstanding.operation_leases() == 1
    ));

    drop(lease);
    assert_eq!(
        session.drain(&DatabaseSessionDrainControl::new(
            DatabaseDrainDeadline::at(Instant::now() + Duration::from_secs(1),)
        )),
        DatabaseDrainOutcome::Drained {
            outstanding: DatabaseOutstandingWork::default(),
        }
    );
}
