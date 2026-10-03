use std::sync::Arc;

use yss_database_contract::{DatabaseDecl, EditState};
use yss_database_runtime::session_api;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::mutation::apply_database_mutation_in_session;
use super::{
    DatabaseApplicationOperation, DatabaseMutationResult, DatabaseOperationError,
    DatabaseUseCaseError,
};
use crate::session::{ApplicationSession, ApplicationState};

mod operation;
pub use operation::DatabaseMutation;
use operation::runtime_database_mutation;

impl ApplicationState {
    pub fn rename_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        name: String,
        operation_id: OperationId,
    ) -> Result<DatabaseMutationResult<()>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let project_snapshot = captured
            .project()
            .read_database_snapshot()
            .map_err(|error| {
                DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                    error,
                    DatabaseApplicationOperation::Rename,
                    &project_instance_id,
                    Some(&id),
                ))
            })?;
        if project_snapshot.project_instance_id() != captured.project_instance_id()
            || project_snapshot.project_session_id() != captured.project_session_id()
        {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::StaleProject {
                    project_instance_id,
                },
            ));
        }
        let declaration = project_snapshot
            .declarations()
            .iter()
            .find(|declaration| declaration.id.as_str() == id)
            .cloned()
            .ok_or_else(|| {
                DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                    database_id: id.clone(),
                })
            })?;
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::InvalidName {
                    database_id: id,
                    requested_name: name,
                },
            ));
        }
        if project_snapshot
            .declarations()
            .iter()
            .any(|other| other.id.as_str() != id && other.name.as_ref() == name)
        {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::NameConflict {
                    database_id: id,
                    requested_name: name,
                },
            ));
        }
        let mut after = declaration;
        after.name = name.clone().into_boxed_str();
        let receipt = apply_database_mutation_in_session(
            self,
            &captured,
            expected_revision,
            operation_id,
            yss_database_runtime::session_api::DatabaseMutationOperation::RenameDatabase {
                name: name.into_boxed_str(),
            },
            after,
            DatabaseApplicationOperation::Rename,
        )?;
        Ok(DatabaseMutationResult {
            data: (),
            mutation: receipt.mutation().clone(),
        })
    }

    pub fn delete_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<DatabaseMutationResult<()>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = delete_database_in_captured_session(
            self,
            &captured,
            id,
            expected_revision,
            operation_id,
        )?;
        self.refresh_database_session(&captured)?;
        Ok(result)
    }

    pub fn mutate_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        mutation: DatabaseMutation,
    ) -> Result<DatabaseMutationResult<EditState>, DatabaseUseCaseError> {
        let operation = mutation.operation();
        let captured = self.capture_database_session(&project_instance_id)?;
        let declaration = database_declaration(&captured, &id, operation)?;
        let receipt = apply_database_mutation_in_session(
            self,
            &captured,
            expected_revision,
            operation_id,
            runtime_database_mutation(&id, mutation)?,
            declaration,
            operation,
        )?;
        Ok(DatabaseMutationResult {
            data: receipt.edit_state().clone(),
            mutation: receipt.mutation().clone(),
        })
    }

    pub fn save_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<DatabaseMutationResult<EditState>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = save_database_in_captured_session(
            self,
            &captured,
            id,
            expected_revision,
            operation_id,
        )?;
        self.refresh_database_session(&captured)?;
        Ok(result)
    }
}

fn delete_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<DatabaseMutationResult<()>, DatabaseUseCaseError> {
    let project_instance_id = captured.project_instance_id().clone();
    let reservation = captured
        .project()
        .reserve_database_operation(&project_instance_id, operation_id)
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_database(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
                Some(expected_revision),
                None,
            ))
        })?;
    let declaration = database_declaration(captured, &id, DatabaseApplicationOperation::Delete)?;
    let receipt = apply_database_mutation_in_session(
        state,
        captured,
        expected_revision,
        operation_id,
        session_api::DatabaseMutationOperation::DeleteDatabase,
        declaration,
        DatabaseApplicationOperation::Delete,
    )?;
    reservation.complete();
    Ok(DatabaseMutationResult {
        data: (),
        mutation: receipt.mutation().clone(),
    })
}

fn save_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<DatabaseMutationResult<EditState>, DatabaseUseCaseError> {
    let declaration = database_declaration(captured, &id, DatabaseApplicationOperation::Save)?;
    let receipt = apply_database_mutation_in_session(
        state,
        captured,
        expected_revision,
        operation_id,
        yss_database_runtime::session_api::DatabaseMutationOperation::Save,
        declaration,
        DatabaseApplicationOperation::Save,
    )?;
    Ok(DatabaseMutationResult {
        data: receipt.edit_state().clone(),
        mutation: receipt.mutation().clone(),
    })
}

fn database_declaration(
    captured: &ApplicationSession,
    id: &str,
    operation: DatabaseApplicationOperation,
) -> Result<DatabaseDecl, DatabaseUseCaseError> {
    captured
        .project()
        .read_database_declaration(captured.project_instance_id(), id)
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_database(
                error,
                operation,
                captured.project_instance_id(),
                Some(id),
                None,
                None,
            ))
        })
}
