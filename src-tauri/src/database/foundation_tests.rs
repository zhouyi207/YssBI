use crate::database::error::{DatabaseErrorCode, DatabaseOperation};
use crate::database::runtime::{
    DatabaseAdmissionCloseOutcome, DatabaseDrainDeadline, DatabaseDrainOutcome,
    DatabaseOutstandingWork, DatabaseRuntimeRegistry, DatabaseSessionDrainControl,
};
use crate::database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
fn database_declaration_wire_bytes_are_preserved_with_opaque_identity() {
    let declaration = declaration("sales", "Sales");

    let bytes = serde_json::to_vec(&declaration).unwrap();
    assert_eq!(
        bytes,
        br#"{"id":"sales","engine":{"inMemory":{"name":"sales"}},"schemaVersion":1,"required":false,"name":"Sales"}"#
    );
    assert_eq!(
        serde_json::from_slice::<DatabaseDecl>(&bytes).unwrap(),
        declaration
    );
}

#[test]
fn database_declaration_fingerprint_is_deterministic_and_decl_specific() {
    let original = declaration("sales", "Sales");
    let same = declaration("sales", "Sales");
    let renamed = declaration("sales", "Renamed");

    assert_eq!(
        DatabaseDeclarationFingerprint::from_decl(&original),
        DatabaseDeclarationFingerprint::from_decl(&same)
    );
    assert_ne!(
        DatabaseDeclarationFingerprint::from_decl(&original),
        DatabaseDeclarationFingerprint::from_decl(&renamed)
    );
}

#[test]
fn duplicate_database_observations_are_rejected_before_set_creation() {
    let id = DatabaseId::from_existing("sales".into());
    let declaration = declaration("sales", "Sales");
    let observation = DatabaseDeclarationObservation::new(
        DatabaseDeclarationRevision::from_existing(1),
        DatabaseDeclarationFingerprint::from_decl(&declaration),
    );

    let error = DatabaseDeclarationObservationSet::try_from_iter([
        (id.clone(), observation.clone()),
        (id, observation),
    ])
    .unwrap_err();

    assert!(matches!(
        error,
        crate::database_contract::DatabaseDeclarationObservationSetError::DuplicateId(id)
            if id.as_str() == "sales"
    ));
}

#[test]
fn open_request_rejects_declaration_observation_fingerprint_mismatch() {
    let declarations = vec![declaration("sales", "Sales")];
    let other = declaration("sales", "Other");
    let observations = DatabaseDeclarationObservationSet::try_from_iter([(
        DatabaseId::from_existing("sales".into()),
        DatabaseDeclarationObservation::new(
            DatabaseDeclarationRevision::from_existing(4),
            DatabaseDeclarationFingerprint::from_decl(&other),
        ),
    )])
    .unwrap();

    let error = request(declarations, observations).validate().unwrap_err();
    assert!(matches!(
        error,
        crate::database_contract::DatabaseSessionOpenRequestError::FingerprintMismatch(id)
            if id.as_str() == "sales"
    ));
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
