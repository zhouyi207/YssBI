use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use super::execution::session_slot::{ApplicationState, SessionRevalidationError};
use crate::events::{CommittedResourceMutation, committed_resource_mutation_from_project};
use yss_database_contract::{DatabaseDeclarationObservation, DatabaseId};
use yss_database_edit::EditState;
use yss_database_runtime::error::{DatabaseError, DatabaseOperation};
use yss_database_runtime::runtime::DatabaseRuntimeSession;
use yss_database_runtime::session_api::{
    CommittedDatabaseRuntimeChange, DatabaseCompensationAttempt, DatabaseCompensationFailureCode,
    DatabaseDeclarationTransition, DatabaseMutationOperation as RuntimeDatabaseMutationOperation,
    DatabaseMutationRequest as RuntimeDatabaseMutationRequest, DatabaseRuntimeChangeOutcome,
    commit_database_runtime_change, prepare_database_runtime_change,
};
use yss_database_schema::DatabaseRuntimeRevision;

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
        RuntimeDatabaseMutationRequest::new(
            self.database,
            DatabaseRuntimeRevision::from_existing(self.expected_runtime_revision),
            DatabaseDeclarationTransition {
                expected: self.expected_observation,
                next: self.next_observation,
            },
            self.operation,
        )
    }

    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub(crate) const fn expected_runtime_revision(&self) -> u64 {
        self.expected_runtime_revision
    }
}

#[derive(Debug)]
pub(crate) struct PreparedProjectDatabaseMutation {
    database: DatabaseId,
    expected_runtime_revision: u64,
    project_authority: Option<yss_project::database_authority::DatabaseAuthorityToken>,
}

impl PreparedProjectDatabaseMutation {
    #[cfg(test)]
    pub(crate) fn new(database: DatabaseId) -> Self {
        Self {
            database,
            expected_runtime_revision: 0,
            project_authority: None,
        }
    }

    pub(crate) fn from_project_authority(
        database: DatabaseId,
        expected_runtime_revision: u64,
        project_authority: yss_project::database_authority::DatabaseAuthorityToken,
    ) -> Self {
        Self {
            database,
            expected_runtime_revision,
            project_authority: Some(project_authority),
        }
    }

    pub(crate) fn take_project_authority(
        mut self,
    ) -> Option<(
        DatabaseId,
        u64,
        yss_project::database_authority::DatabaseAuthorityToken,
    )> {
        self.project_authority
            .take()
            .map(|authority| (self.database, self.expected_runtime_revision, authority))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum ProjectDatabaseMutationError {
    #[error("Project database authority is not installed")]
    AuthorityUnavailable,
    #[error("Project database mutation session is stale")]
    StaleSession,
}

#[derive(Debug, Error)]
pub(crate) enum ProjectDatabaseFinalizeError {
    #[error("Project database finalization rejected the runtime change")]
    Rejected,
    #[error("Project database finalization observed a stale session")]
    StaleSession,
    #[error("Project database finalization was rejected")]
    Project(#[source] yss_project::ProjectDatabaseError),
}

#[derive(Debug)]
pub(crate) struct ProjectDatabaseMutationReceipt {
    mutation: CommittedResourceMutation,
}

impl ProjectDatabaseMutationReceipt {
    pub(crate) fn from_project(
        mutation: yss_project::project_writers::ProjectResourceMutationFacts,
    ) -> Self {
        Self {
            mutation: committed_resource_mutation_from_project(mutation),
        }
    }

    pub(crate) fn mutation(&self) -> &CommittedResourceMutation {
        &self.mutation
    }
}

pub(crate) trait ProjectDatabaseMutationPort: Send + Sync {
    fn prepare(
        &self,
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
    _owner: CommittedDatabaseRuntimeChange,
    _physical: Option<yss_database_runtime::runtime::PreparedDatabasePhysicalMutation>,
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

impl fmt::Display for UnresolvedDatabaseCompensation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "database `{}` requires recovery after {:?}",
            self.database.as_str(),
            self.failure
        )
    }
}

#[derive(Debug, Error)]
pub(crate) enum DatabaseMutationRecovery {
    #[error("database `{}` compensation restored", database.as_str())]
    Restored { database: DatabaseId },
    #[error("{owner}")]
    RecoveryRequired {
        owner: Box<UnresolvedDatabaseCompensation>,
    },
}

#[derive(Debug, Error)]
pub(crate) enum DatabaseMutationApplicationError {
    #[error("database admission failed")]
    Admission(#[source] DatabaseError),
    #[error("Project database preparation failed")]
    ProjectPrepare(#[source] ProjectDatabaseMutationError),
    #[error("Database runtime preparation failed")]
    DatabasePrepare(#[source] DatabaseError),
    #[error("Database runtime commit failed")]
    DatabaseCommit(#[source] DatabaseError),
    #[error("captured application session changed after Database commit; {recovery}")]
    StaleSession {
        source: SessionRevalidationError,
        recovery: DatabaseMutationRecovery,
    },
    #[error("Project finalization failed after Database commit; {recovery}")]
    ProjectFinalize {
        source: ProjectDatabaseFinalizeError,
        recovery: DatabaseMutationRecovery,
    },
}

pub(crate) fn mutate_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<super::execution::session_slot::ApplicationSession>,
    request: DatabaseMutationRequest,
    project: &dyn ProjectDatabaseMutationPort,
) -> Result<DatabaseMutationApplicationReceipt, DatabaseMutationApplicationError> {
    let _admission = captured
        .database()
        .admit_operation(DatabaseOperation::PrepareMutation)
        .map_err(DatabaseMutationApplicationError::Admission)?;
    coordinate_database_handoff(captured.database(), request, project, || {
        state.revalidate_captured_session(captured)
    })
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
    database: &DatabaseRuntimeSession,
    request: DatabaseMutationRequest,
    project: &dyn ProjectDatabaseMutationPort,
    final_session_gate: impl FnOnce() -> Result<(), SessionRevalidationError>,
) -> Result<DatabaseMutationApplicationReceipt, HandoffError> {
    let prepared_project = project
        .prepare(&request)
        .map_err(HandoffError::ProjectPrepare)?;
    let physical = database
        .prepare_physical_mutation(&request.database, &request.operation)
        .map_err(HandoffError::DatabasePrepare)?;
    let edit_state = physical.edit_state();
    let prepared_database = match prepare_database_runtime_change(database, request.into_runtime())
    {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = physical.rollback();
            return Err(HandoffError::DatabasePrepare(error));
        }
    };
    let committed_database = match commit_database_runtime_change(database, prepared_database) {
        Ok(committed) => committed,
        Err(error) => {
            let _ = physical.rollback();
            return Err(HandoffError::DatabaseCommit(error));
        }
    };
    database.install_physical_mutation(&physical);
    let database_outcome = committed_database.outcome();
    if let Err(source) = final_session_gate() {
        return Err(HandoffError::StaleSession {
            source,
            recovery: compensate_committed_change(committed_database, physical),
        });
    }

    let project_receipt = match project.finalize(prepared_project, database_outcome) {
        Ok(receipt) => receipt,
        Err(source) => {
            return Err(HandoffError::ProjectFinalize {
                source,
                recovery: compensate_committed_change(committed_database, physical),
            });
        }
    };
    committed_database.confirm();
    Ok(DatabaseMutationApplicationReceipt {
        edit_state,
        mutation: project_receipt.mutation().clone(),
    })
}

fn compensate_committed_change(
    committed: CommittedDatabaseRuntimeChange,
    physical: yss_database_runtime::runtime::PreparedDatabasePhysicalMutation,
) -> DatabaseMutationRecovery {
    let database = committed.outcome().database().clone();
    if physical.rollback().is_err() {
        return DatabaseMutationRecovery::RecoveryRequired {
            owner: Box::new(UnresolvedDatabaseCompensation {
                database,
                failure: DatabaseCompensationFailureCode::Driver,
                _owner: committed,
                _physical: Some(physical),
            }),
        };
    }
    match committed.compensate() {
        DatabaseCompensationAttempt::Restored(_) => DatabaseMutationRecovery::Restored { database },
        DatabaseCompensationAttempt::Retryable { owner, failure } => {
            DatabaseMutationRecovery::RecoveryRequired {
                owner: Box::new(UnresolvedDatabaseCompensation {
                    database,
                    failure: failure.code(),
                    _owner: owner,
                    _physical: None,
                }),
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct DatabaseMutationApplicationReceipt {
    edit_state: EditState,
    mutation: CommittedResourceMutation,
}

impl DatabaseMutationApplicationReceipt {
    pub(crate) fn edit_state(&self) -> &EditState {
        &self.edit_state
    }

    pub(crate) fn mutation(&self) -> &CommittedResourceMutation {
        &self.mutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_database_edit::EditHistory;
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_database_runtime::{DatabaseInstance, DatabaseState};

    struct RejectingProject;

    impl ProjectDatabaseMutationPort for RejectingProject {
        fn prepare(
            &self,
            request: &DatabaseMutationRequest,
        ) -> Result<PreparedProjectDatabaseMutation, ProjectDatabaseMutationError> {
            Ok(PreparedProjectDatabaseMutation::new(
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
            yss_database_contract::DatabaseDeclarationObservationSet::try_from_iter([(
                declaration.id.clone(),
                DatabaseDeclarationObservation::new(
                    DatabaseDeclarationRevision::from_existing(1),
                    DatabaseDeclarationFingerprint::from_decl(&declaration),
                ),
            )])
            .expect("observation set is valid");
        let dataframe = polars::df!("value" => &[1_i64]).expect("test dataframe is valid");
        let instance = DatabaseInstance {
            decl: declaration.clone(),
            state: DatabaseState::Loaded {
                dataframe: Arc::new(dataframe.clone()),
                original: Arc::new(dataframe),
                history: EditHistory::new(),
            },
        };
        DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing("session".into()),
                    NonZeroU64::new(1).expect("generation is non-zero"),
                    None,
                    vec![declaration].into(),
                    observations,
                ),
                [instance],
            )
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
                value: yss_tabular_contract::TabularScalar::Null,
                row_id: None,
            },
        )
    }

    #[test]
    fn project_handoff_failure_consumes_exact_database_owner_into_typed_compensation() {
        let database = database_session();
        let result = coordinate_database_handoff(
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
                .runtime_revision(&DatabaseId::from_existing("sales".into()))
                .expect("database revision remains registered")
                .get(),
            0
        );
    }
}
