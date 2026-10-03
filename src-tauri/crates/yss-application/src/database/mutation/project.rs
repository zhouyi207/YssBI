use std::sync::Arc;

use yss_database_contract::DatabaseDecl;
use yss_database_runtime::session_api;
use yss_project::{ProjectDatabaseError, ProjectOperationError, ProjectState};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::{
    DatabaseMutationRequest as RuntimeDatabaseMutationRequest, PreparedProjectDatabaseMutation,
    ProjectDatabaseFinalizeError, ProjectDatabaseMutationError, ProjectDatabaseMutationPort,
    ProjectDatabaseMutationReceipt,
};
use crate::database::{DatabaseApplicationOperation, DatabaseOperationError, DatabaseUseCaseError};
use crate::session::{ApplicationSession, ApplicationState};

struct ProjectDatabaseAuthority<'a> {
    project: &'a ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    expected_project_revision: ResourceRevision,
    after: DatabaseDecl,
    delete: bool,
}

impl ProjectDatabaseMutationPort for ProjectDatabaseAuthority<'_> {
    fn prepare(
        &self,
        request: &RuntimeDatabaseMutationRequest,
    ) -> Result<PreparedProjectDatabaseMutation, ProjectDatabaseMutationError> {
        let token = self
            .project
            .prepare_database_mutation_authority(
                &self.project_instance_id,
                request.database().as_str(),
                self.expected_project_revision,
            )
            .map_err(|error| match error {
                ProjectDatabaseError::Project(ProjectOperationError::StaleProjectLifecycle {
                    ..
                }) => ProjectDatabaseMutationError::StaleSession,
                _ => ProjectDatabaseMutationError::AuthorityUnavailable,
            })?;
        Ok(PreparedProjectDatabaseMutation::from_project_authority(
            request.database().clone(),
            request.expected_runtime_revision(),
            token,
        ))
    }

    fn finalize(
        &self,
        prepared: PreparedProjectDatabaseMutation,
        database: &yss_database_runtime::session_api::DatabaseRuntimeChangeOutcome,
    ) -> Result<ProjectDatabaseMutationReceipt, ProjectDatabaseFinalizeError> {
        let Some((database_id, expected_runtime_revision, token)) =
            prepared.take_project_authority()
        else {
            return Err(ProjectDatabaseFinalizeError::Rejected);
        };
        let expected_after = expected_runtime_revision
            .checked_add(1)
            .ok_or(ProjectDatabaseFinalizeError::Rejected)?;
        if database.database() != &database_id
            || database.runtime_revision().get() != expected_after
            || database_id != self.after.id
        {
            return Err(ProjectDatabaseFinalizeError::Rejected);
        }
        let publication = if self.delete {
            self.project.commit_database_declaration_delete(
                &self.project_instance_id,
                self.after.id.as_str(),
                self.expected_project_revision,
                self.operation_id,
            )
        } else {
            self.project.commit_database_declaration_update(
                &self.project_instance_id,
                token,
                self.after.clone(),
                self.operation_id,
            )
        };
        publication
            .map(ProjectDatabaseMutationReceipt::from_project)
            .map_err(|error| match error {
                ProjectDatabaseError::Project(ProjectOperationError::StaleProjectLifecycle {
                    ..
                }) => ProjectDatabaseFinalizeError::StaleSession,
                error => ProjectDatabaseFinalizeError::Project(error),
            })
    }
}

pub(in crate::database) fn check_database_revision(
    captured: &ApplicationSession,
    id: &str,
    revision: ResourceRevision,
    operation: DatabaseApplicationOperation,
) -> Result<(), DatabaseUseCaseError> {
    captured
        .project()
        .prepare_database_mutation_authority(captured.project_instance_id(), id, revision)
        .map(|_| ())
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_database(
                error,
                operation,
                captured.project_instance_id(),
                Some(id),
                Some(revision),
                None,
            ))
        })
}

pub(in crate::database) fn apply_database_mutation_in_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    expected_project_revision: ResourceRevision,
    operation_id: OperationId,
    operation: yss_database_runtime::session_api::DatabaseMutationOperation,
    after: DatabaseDecl,
    application_operation: DatabaseApplicationOperation,
) -> Result<crate::database::mutation::DatabaseMutationApplicationReceipt, DatabaseUseCaseError> {
    let database = after.id.clone();
    let runtime_revision = captured
        .database()
        .runtime_revision(&database)
        .map(|revision| revision.get())
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                database_id: database.as_str().to_owned(),
            })
        })?;
    let observations = captured.database().observations();
    let expected_observation = observations.get(&database).cloned().ok_or_else(|| {
        DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
            database_id: database.as_str().to_owned(),
        })
    })?;
    let next_revision = expected_observation
        .revision()
        .get()
        .checked_add(1)
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
                application_operation,
                "database revision exhausted",
            ))
        })?;
    let next_observation = yss_database_contract::DatabaseDeclarationObservation::new(
        yss_database_contract::DatabaseDeclarationRevision::from_existing(next_revision),
        yss_database_contract::DatabaseDeclarationFingerprint::from_decl(&after),
    );
    let delete = matches!(
        operation,
        session_api::DatabaseMutationOperation::DeleteDatabase
    );
    let runtime_request = RuntimeDatabaseMutationRequest::new(
        database,
        runtime_revision,
        expected_observation,
        next_observation,
        operation,
        operation_id,
    );
    let authority = ProjectDatabaseAuthority {
        project: captured.project(),
        project_instance_id: captured.project_instance_id().clone(),
        operation_id,
        expected_project_revision,
        after,
        delete,
    };
    crate::database::mutation::mutate_database_in_captured_session(
        state,
        captured,
        runtime_request,
        &authority,
    )
    .map_err(|source| DatabaseUseCaseError::Mutation(Box::new(source.into())))
}
