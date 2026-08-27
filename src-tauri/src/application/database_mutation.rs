#![allow(
    dead_code,
    reason = "staged until Project database authority is installed"
)]

use std::fmt;

use thiserror::Error;

use super::execution::session_slot::{
    ApplicationSessionEpoch, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::database::error::{DatabaseError, DatabaseOperation};
use crate::database::runtime::DatabaseRuntimeSession;
use crate::database::schema_snapshot::DatabaseRuntimeRevision;
use crate::database::session_api::{
    CommittedDatabaseRuntimeChange, DatabaseCompensationAttempt, DatabaseCompensationFailureCode,
    DatabaseDeclarationTransition, DatabaseMutationOperation as RuntimeDatabaseMutationOperation,
    DatabaseMutationRequest as RuntimeDatabaseMutationRequest, DatabaseRuntimeChangeOutcome,
    commit_database_runtime_change, prepare_database_runtime_change,
};
use crate::database_contract::{DatabaseDeclarationObservation, DatabaseId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DatabaseMutationRequest {
    database: DatabaseId,
    expected_runtime_revision: u64,
    expected_observation: DatabaseDeclarationObservation,
    next_observation: DatabaseDeclarationObservation,
    operation: RuntimeDatabaseMutationOperation,
}

impl DatabaseMutationRequest {
    pub(crate) fn new(
        database: DatabaseId,
        expected_runtime_revision: u64,
        expected_observation: DatabaseDeclarationObservation,
        next_observation: DatabaseDeclarationObservation,
        operation: RuntimeDatabaseMutationOperation,
    ) -> Self {
        Self {
            database,
            expected_runtime_revision,
            expected_observation,
            next_observation,
            operation,
        }
    }

    fn into_runtime(self) -> RuntimeDatabaseMutationRequest {
        RuntimeDatabaseMutationRequest {
            database: self.database,
            expected_runtime_revision: DatabaseRuntimeRevision::from_existing(
                self.expected_runtime_revision,
            ),
            declaration_transition: DatabaseDeclarationTransition {
                expected: self.expected_observation,
                next: self.next_observation,
            },
            operation: self.operation,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedProjectDatabaseMutation {
    session_epoch: ApplicationSessionEpoch,
    database: DatabaseId,
}

impl PreparedProjectDatabaseMutation {
    pub(crate) fn new(session_epoch: ApplicationSessionEpoch, database: DatabaseId) -> Self {
        Self {
            session_epoch,
            database,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ProjectDatabaseMutationError {
    #[error("Project database authority is not installed")]
    AuthorityUnavailable,
    #[error("Project database mutation session is stale")]
    StaleSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ProjectDatabaseFinalizeError {
    #[error("Project database finalization rejected the runtime change")]
    Rejected,
    #[error("Project database finalization observed a stale session")]
    StaleSession,
}

#[derive(Debug)]
pub(crate) struct ProjectDatabaseMutationReceipt {
    session_epoch: ApplicationSessionEpoch,
    database: DatabaseId,
}

impl ProjectDatabaseMutationReceipt {
    pub(crate) fn session_epoch(&self) -> ApplicationSessionEpoch {
        self.session_epoch
    }

    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }
}

pub(crate) trait ProjectDatabaseMutationPort: Send + Sync {
    fn prepare(
        &self,
        session_epoch: ApplicationSessionEpoch,
        request: &DatabaseMutationRequest,
    ) -> Result<PreparedProjectDatabaseMutation, ProjectDatabaseMutationError>;

    /// This is the Project finalization seam. It must not perform Database I/O
    /// and must consume its prepared Project owner exactly once.
    fn finalize(
        &self,
        prepared: PreparedProjectDatabaseMutation,
        database: &DatabaseRuntimeChangeOutcome,
    ) -> Result<ProjectDatabaseMutationReceipt, ProjectDatabaseFinalizeError>;
}

pub(crate) struct UnresolvedDatabaseCompensation {
    database: DatabaseId,
    failure: DatabaseCompensationFailureCode,
    owner: CommittedDatabaseRuntimeChange,
}

impl fmt::Debug for UnresolvedDatabaseCompensation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnresolvedDatabaseCompensation")
            .field("database", &self.database)
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl UnresolvedDatabaseCompensation {
    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub(crate) fn failure(&self) -> DatabaseCompensationFailureCode {
        self.failure
    }

    pub(crate) fn retry(self) -> Result<DatabaseMutationRecovery, UnresolvedDatabaseCompensation> {
        match self.owner.compensate() {
            DatabaseCompensationAttempt::Restored(_) => Ok(DatabaseMutationRecovery::Restored {
                database: self.database,
            }),
            DatabaseCompensationAttempt::Retryable { owner, failure } => Err(Self {
                database: self.database,
                failure: failure.code(),
                owner,
            }),
        }
    }
}

#[derive(Debug)]
pub(crate) enum DatabaseMutationRecovery {
    Restored {
        database: DatabaseId,
    },
    Retryable {
        owner: UnresolvedDatabaseCompensation,
    },
}

#[derive(Debug, Error)]
pub(crate) enum DatabaseMutationApplicationError {
    #[error("application session capture failed")]
    SessionCapture(#[source] SessionCaptureError),
    #[error("database admission failed")]
    Admission(#[source] DatabaseError),
    #[error("Project database preparation failed")]
    ProjectPrepare(#[source] ProjectDatabaseMutationError),
    #[error("Database runtime preparation failed")]
    DatabasePrepare(#[source] DatabaseError),
    #[error("Database runtime commit failed")]
    DatabaseCommit(#[source] DatabaseError),
    #[error("captured application session changed after Database commit")]
    StaleSession {
        source: SessionRevalidationError,
        recovery: DatabaseMutationRecovery,
    },
    #[error("Project finalization failed after Database commit")]
    ProjectFinalize {
        source: ProjectDatabaseFinalizeError,
        recovery: DatabaseMutationRecovery,
    },
    #[error("Project database mutation coordinator is staged")]
    Staged,
}

pub(crate) fn mutate_database(
    state: &ApplicationState,
    _request: DatabaseMutationRequest,
) -> Result<DatabaseMutationApplicationReceipt, DatabaseMutationApplicationError> {
    let captured = state
        .capture_session()
        .map_err(DatabaseMutationApplicationError::SessionCapture)?;
    let _admission = captured
        .database()
        .admit_operation(DatabaseOperation::PrepareMutation)
        .map_err(DatabaseMutationApplicationError::Admission)?;
    Err(DatabaseMutationApplicationError::Staged)
}

pub(crate) fn mutate_database_with_project_authority(
    state: &ApplicationState,
    request: DatabaseMutationRequest,
    project: &dyn ProjectDatabaseMutationPort,
) -> Result<DatabaseMutationApplicationReceipt, DatabaseMutationApplicationError> {
    let captured = state
        .capture_session()
        .map_err(DatabaseMutationApplicationError::SessionCapture)?;
    let _admission = captured
        .database()
        .admit_operation(DatabaseOperation::PrepareMutation)
        .map_err(DatabaseMutationApplicationError::Admission)?;
    coordinate_database_handoff(
        captured.epoch(),
        captured.database(),
        request,
        project,
        || state.revalidate_captured_session(&captured),
    )
    .map_err(|error| match error {
        HandoffError::ProjectPrepare(source) => {
            DatabaseMutationApplicationError::ProjectPrepare(source)
        }
        HandoffError::DatabasePrepare(source) => {
            DatabaseMutationApplicationError::DatabasePrepare(source)
        }
        HandoffError::DatabaseCommit(source) => {
            DatabaseMutationApplicationError::DatabaseCommit(source)
        }
        HandoffError::StaleSession { source, recovery } => {
            DatabaseMutationApplicationError::StaleSession { source, recovery }
        }
        HandoffError::ProjectFinalize { source, recovery } => {
            DatabaseMutationApplicationError::ProjectFinalize { source, recovery }
        }
    })
}

#[derive(Debug)]
enum HandoffError {
    ProjectPrepare(ProjectDatabaseMutationError),
    DatabasePrepare(DatabaseError),
    DatabaseCommit(DatabaseError),
    StaleSession {
        source: SessionRevalidationError,
        recovery: DatabaseMutationRecovery,
    },
    ProjectFinalize {
        source: ProjectDatabaseFinalizeError,
        recovery: DatabaseMutationRecovery,
    },
}

fn coordinate_database_handoff(
    session_epoch: ApplicationSessionEpoch,
    database: &DatabaseRuntimeSession,
    request: DatabaseMutationRequest,
    project: &dyn ProjectDatabaseMutationPort,
    final_session_gate: impl FnOnce() -> Result<(), SessionRevalidationError>,
) -> Result<DatabaseMutationApplicationReceipt, HandoffError> {
    let prepared_project = project
        .prepare(session_epoch, &request)
        .map_err(HandoffError::ProjectPrepare)?;
    let prepared_database = prepare_database_runtime_change(database, request.into_runtime())
        .map_err(HandoffError::DatabasePrepare)?;
    let committed_database = commit_database_runtime_change(database, prepared_database)
        .map_err(HandoffError::DatabaseCommit)?;
    let database_outcome = committed_database.outcome();
    if let Err(source) = final_session_gate() {
        return Err(HandoffError::StaleSession {
            source,
            recovery: compensate_committed_change(committed_database),
        });
    }

    let project_receipt = match project.finalize(prepared_project, database_outcome) {
        Ok(receipt) => receipt,
        Err(source) => {
            return Err(HandoffError::ProjectFinalize {
                source,
                recovery: compensate_committed_change(committed_database),
            });
        }
    };
    let database_outcome = committed_database.confirm();
    Ok(DatabaseMutationApplicationReceipt {
        session_epoch: project_receipt.session_epoch(),
        database: database_outcome.database().clone(),
    })
}

fn compensate_committed_change(
    committed: CommittedDatabaseRuntimeChange,
) -> DatabaseMutationRecovery {
    let database = committed.outcome().database().clone();
    match committed.compensate() {
        DatabaseCompensationAttempt::Restored(_) => DatabaseMutationRecovery::Restored { database },
        DatabaseCompensationAttempt::Retryable { owner, failure } => {
            DatabaseMutationRecovery::Retryable {
                owner: UnresolvedDatabaseCompensation {
                    database,
                    failure: failure.code(),
                    owner,
                },
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct DatabaseMutationApplicationReceipt {
    session_epoch: ApplicationSessionEpoch,
    database: DatabaseId,
}

impl DatabaseMutationApplicationReceipt {
    pub(crate) fn session_epoch(&self) -> ApplicationSessionEpoch {
        self.session_epoch
    }

    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use std::num::NonZeroU64;

    struct RejectingProject;

    impl ProjectDatabaseMutationPort for RejectingProject {
        fn prepare(
            &self,
            session_epoch: ApplicationSessionEpoch,
            request: &DatabaseMutationRequest,
        ) -> Result<PreparedProjectDatabaseMutation, ProjectDatabaseMutationError> {
            Ok(PreparedProjectDatabaseMutation::new(
                session_epoch,
                request.database.clone(),
            ))
        }

        fn finalize(
            &self,
            _prepared: PreparedProjectDatabaseMutation,
            _database: &DatabaseRuntimeChangeOutcome,
        ) -> Result<ProjectDatabaseMutationReceipt, ProjectDatabaseFinalizeError> {
            Err(ProjectDatabaseFinalizeError::Rejected)
        }
    }

    fn database_session() -> DatabaseRuntimeSession {
        let declaration = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };
        let observations =
            crate::database_contract::DatabaseDeclarationObservationSet::try_from_iter([(
                declaration.id.clone(),
                DatabaseDeclarationObservation::new(
                    DatabaseDeclarationRevision::from_existing(1),
                    DatabaseDeclarationFingerprint::from_decl(&declaration),
                ),
            )])
            .expect("observation set is valid");
        DatabaseRuntimeRegistry::new()
            .open_session(DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing("session".into()),
                NonZeroU64::new(1).expect("generation is non-zero"),
                None,
                vec![declaration].into(),
                observations,
            ))
            .expect("database session is valid")
    }

    fn request(session: &DatabaseRuntimeSession) -> DatabaseMutationRequest {
        let observation = session
            .observations()
            .iter()
            .next()
            .expect("test declaration has an observation")
            .1
            .clone();
        DatabaseMutationRequest::new(
            DatabaseId::from_existing("sales".into()),
            0,
            observation.clone(),
            observation,
            RuntimeDatabaseMutationOperation::EditCell {
                row: 0,
                column: "value".into(),
                value: crate::tabular::contract::TabularScalar::Null,
            },
        )
    }

    #[test]
    fn project_handoff_failure_consumes_exact_database_owner_into_typed_compensation() {
        let database = database_session();
        let result = coordinate_database_handoff(
            ApplicationSessionEpoch::from_existing(1),
            &database,
            request(&database),
            &RejectingProject,
            || Ok(()),
        );

        match result {
            Err(HandoffError::ProjectFinalize {
                source: ProjectDatabaseFinalizeError::Rejected,
                recovery: DatabaseMutationRecovery::Restored { database },
            }) => assert_eq!(database.as_str(), "sales"),
            Err(other) => panic!("unexpected handoff result: {other:?}"),
            Ok(_) => panic!("Project rejection must not publish a receipt"),
        }
        assert_eq!(
            database
                .revisions(&DatabaseId::from_existing("sales".into()))
                .expect("database revision remains registered")
                .runtime,
            0
        );
    }
}
