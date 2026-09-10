use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

mod error;
mod export;
mod import;
#[cfg(test)]
mod tests;
use export::export_database_in_captured_session;
use import::load_database_in_captured_session;

pub use self::error::{
    DatabaseApplicationInternalError, DatabaseApplicationOperation, DatabaseOperationError,
};
use crate::database_mutation::{
    DatabaseMutationRequest as RuntimeDatabaseMutationRequest, PreparedProjectDatabaseMutation,
    ProjectDatabaseFinalizeError, ProjectDatabaseMutationError, ProjectDatabaseMutationPort,
    ProjectDatabaseMutationReceipt,
};
use crate::events::CommittedResourceMutation;
pub fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed")
        .to_owned()
}
use crate::execution::session_slot::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, SessionCaptureError,
    SessionRevalidationError,
};
use uuid::Uuid;
use yss_database_contract::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseExportFormat, DatabaseId,
    DatabaseImportSource,
};
use yss_database_edit::EditState;
use yss_database_runtime::MAX_GET_DATAFRAME_ROWS;
use yss_database_runtime::error::{DatabaseError, DatabaseErrorCode};
use yss_database_runtime::session_api;
use yss_database_schema::DatabaseColumnFact;
use yss_display_naming::allocate_unique_display_name;
use yss_project::{ProjectDatabaseError, ProjectState};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_sql_source::list_tables as list_sql_source_tables;
use yss_tabular_contract::TabularSnapshot;
use yss_tabular_io::list_excel_sheets as list_workbook_sheets;

#[derive(Debug)]
pub struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<DatabaseColumnFact>,
}

#[derive(Debug)]
pub struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<DatabaseColumnFact>,
}

#[derive(Debug)]
pub struct DatabaseRowsResult {
    pub rows: TabularSnapshot,
    pub row_ids: Vec<i64>,
}

#[derive(Debug)]
pub struct DatabaseMutationResult<T> {
    pub data: T,
    pub mutation: CommittedResourceMutation,
}

#[derive(Debug, Error)]
#[error("database mutation failed")]
pub struct DatabaseMutationFailure {
    #[source]
    source: crate::database_mutation::DatabaseMutationApplicationError,
}

impl From<crate::database_mutation::DatabaseMutationApplicationError> for DatabaseMutationFailure {
    fn from(source: crate::database_mutation::DatabaseMutationApplicationError) -> Self {
        Self { source }
    }
}

#[derive(Debug, Error)]
pub enum DatabaseUseCaseError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured application session changed during database operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application database session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
    #[error(transparent)]
    Database(#[from] DatabaseOperationError),
    #[error("database mutation failed")]
    Mutation(#[source] Box<DatabaseMutationFailure>),
}

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
                ProjectDatabaseError::Project(ProjectFilesystemError::StaleProjectLifecycle {
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
                ProjectDatabaseError::Project(ProjectFilesystemError::StaleProjectLifecycle {
                    ..
                }) => ProjectDatabaseFinalizeError::StaleSession,
                error => ProjectDatabaseFinalizeError::Project(error),
            })
    }
}

impl ApplicationState {
    pub fn load_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        source: DatabaseImportSource,
    ) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = load_database_in_captured_session(&captured, operation_id, source)?;
        self.refresh_database_session()?;
        Ok(result)
    }

    pub fn rename_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        name: String,
        operation_id: OperationId,
    ) -> Result<DatabaseMutationResult<()>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let declaration = captured
            .project()
            .get_data()
            .map_err(|error| {
                DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                    error,
                    DatabaseApplicationOperation::Rename,
                    &project_instance_id,
                    Some(&id),
                ))
            })?
            .databases
            .get(&id)
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
        if captured
            .project()
            .get_data()
            .map_err(|error| {
                DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                    error,
                    DatabaseApplicationOperation::Rename,
                    &project_instance_id,
                    None,
                ))
            })?
            .databases
            .iter()
            .any(|(other_id, other)| other_id != &id && other.name.as_ref() == name)
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
            project_instance_id,
            id,
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
        self.refresh_database_session()?;
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
        let captured = self.capture_database_session(&project_instance_id)?;
        let declaration = captured
            .project()
            .get_data()
            .map_err(|error| {
                DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                    error,
                    DatabaseApplicationOperation::EditCell,
                    &project_instance_id,
                    Some(&id),
                ))
            })?
            .databases
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                    database_id: id.clone(),
                })
            })?;
        let receipt = apply_database_mutation_in_session(
            self,
            &captured,
            project_instance_id,
            id.clone(),
            expected_revision,
            operation_id,
            runtime_database_mutation(&id, mutation)?,
            declaration,
            DatabaseApplicationOperation::EditCell,
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
        self.refresh_database_session()?;
        Ok(result)
    }

    pub fn query_database_meta_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<DatabaseMetaResult, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(error, DatabaseApplicationOperation::ReadMetadata, &id)
            })?;
        let snapshot =
            session_api::metadata_snapshot(captured.database(), database).map_err(|error| {
                map_database_runtime_error(error, DatabaseApplicationOperation::ReadMetadata, &id)
            })?;
        let result = DatabaseMetaResult {
            id,
            name: snapshot.name().to_owned(),
            row_count: snapshot.row_count(),
            column_count: snapshot.schema().columns().len(),
            columns: snapshot.schema().columns().to_vec(),
        };
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(
                error,
                DatabaseApplicationOperation::ReadMetadata,
                &result.id,
            )
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_database_rows_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRowsResult, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        if limit > MAX_GET_DATAFRAME_ROWS {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::RowLimitExceeded {
                    database_id: id,
                    operation: DatabaseApplicationOperation::ReadRows,
                    requested_rows: limit,
                    max_rows: MAX_GET_DATAFRAME_ROWS,
                },
            ));
        }
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(error, DatabaseApplicationOperation::ReadRows, &id)
            })?;
        let page = session_api::page_snapshot(captured.database(), database, offset, limit)
            .map_err(|error| {
                map_database_runtime_error(error, DatabaseApplicationOperation::ReadRows, &id)
            })?;
        let result = DatabaseRowsResult {
            rows: page.rows().clone(),
            row_ids: page.row_ids().to_vec(),
        };
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ReadRows, &id)
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_column_stats_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<Vec<yss_dataset_profile::ColumnStats>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ColumnStatistics,
                    &id,
                )
            })?;
        let result =
            session_api::column_statistics(captured.database(), database).map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ColumnStatistics,
                    &id,
                )
            })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ColumnStatistics, &id)
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_column_distributions_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ColumnDistribution,
                    &id,
                )
            })?;
        let result =
            session_api::column_distributions(captured.database(), database).map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ColumnDistribution,
                    &id,
                )
            })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ColumnDistribution, &id)
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_dataset_overview_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<yss_dataset_profile::DatasetOverview, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::DatasetOverview,
                    &id,
                )
            })?;
        let result =
            session_api::dataset_overview(captured.database(), database).map_err(|error| {
                map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::DatasetOverview,
                    &id,
                )
            })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::DatasetOverview, &id)
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_database_edit_state_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<EditState, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let database = database_id(&id);
        let basis = captured
            .database()
            .capture_query_basis(&database)
            .map_err(|error| {
                map_database_runtime_error(error, DatabaseApplicationOperation::ReadEditState, &id)
            })?;
        let result = session_api::edit_state(captured.database(), database).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ReadEditState, &id)
        })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ReadEditState, &id)
        })?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn export_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        path: String,
        format: String,
    ) -> Result<(), DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        export_database_in_captured_session(self, &captured, &id, &path, &format)?;
        self.revalidate_captured_session(&captured)
            .map_err(DatabaseUseCaseError::SessionChanged)
    }

    fn capture_database_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, DatabaseUseCaseError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::StaleProject {
                    project_instance_id: project_instance_id.clone(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_database_session(&self) -> Result<(), DatabaseUseCaseError> {
        self.rebuild_application_session()
            .map_err(DatabaseUseCaseError::SessionRefresh)
    }
}

fn database_id(id: &str) -> DatabaseId {
    DatabaseId::from_existing(id.to_owned().into_boxed_str())
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
    let declaration = captured
        .project()
        .get_data()
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
            ))
        })?
        .databases
        .get(&id)
        .cloned()
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                database_id: id.clone(),
            })
        })?;
    let receipt = apply_database_mutation_in_session(
        state,
        captured,
        project_instance_id,
        id,
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
    let project_instance_id = captured.project_instance_id().clone();
    let declaration = captured
        .project()
        .get_data()
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Save,
                &project_instance_id,
                Some(&id),
            ))
        })?
        .databases
        .get(&id)
        .cloned()
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                database_id: id.clone(),
            })
        })?;
    let receipt = apply_database_mutation_in_session(
        state,
        captured,
        project_instance_id,
        id,
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

#[allow(
    clippy::too_many_arguments,
    reason = "the coordinator keeps the complete Project/Database authority tuple explicit"
)]
fn apply_database_mutation_in_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_project_revision: ResourceRevision,
    operation_id: OperationId,
    operation: yss_database_runtime::session_api::DatabaseMutationOperation,
    after: DatabaseDecl,
    application_operation: DatabaseApplicationOperation,
) -> Result<crate::database_mutation::DatabaseMutationApplicationReceipt, DatabaseUseCaseError> {
    let database = database_id(&id);
    let runtime_revision = captured
        .database()
        .runtime_revision(&database)
        .map(|revision| revision.get())
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                database_id: id.clone(),
            })
        })?;
    let observations = captured.database().observations();
    let expected_observation = observations
        .iter()
        .find(|(database_id, _)| *database_id == &database)
        .map(|(_, observation)| observation.clone())
        .ok_or_else(|| {
            DatabaseUseCaseError::Database(DatabaseOperationError::NotFound {
                database_id: id.clone(),
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
        project_instance_id,
        operation_id,
        expected_project_revision,
        after,
        delete,
    };
    crate::database_mutation::mutate_database_in_captured_session(
        state,
        captured,
        runtime_request,
        &authority,
    )
    .map_err(|source| DatabaseUseCaseError::Mutation(Box::new(source.into())))
}

fn map_database_runtime_error(
    error: DatabaseError,
    operation: DatabaseApplicationOperation,
    database_id: &str,
) -> DatabaseOperationError {
    let resource = error
        .resource()
        .map(|resource| resource.as_str())
        .unwrap_or(database_id)
        .to_owned();
    match error.code() {
        DatabaseErrorCode::NotFound => DatabaseOperationError::NotFound {
            database_id: resource,
        },
        DatabaseErrorCode::InvalidRequest => DatabaseOperationError::InvalidInput {
            database_id: resource,
            operation,
            field: "databaseId",
        },
        DatabaseErrorCode::AdmissionClosed
        | DatabaseErrorCode::Conflict
        | DatabaseErrorCode::Schema
        | DatabaseErrorCode::Unsupported => DatabaseOperationError::InvalidAccess {
            database_id: resource,
            operation,
        },
        DatabaseErrorCode::Constraint
        | DatabaseErrorCode::Driver
        | DatabaseErrorCode::Cancelled
        | DatabaseErrorCode::Deadline => DatabaseOperationError::internal(operation, error),
    }
}

fn runtime_database_mutation(
    database_id: &str,
    mutation: DatabaseMutation,
) -> Result<yss_database_runtime::session_api::DatabaseMutationOperation, DatabaseUseCaseError> {
    use yss_database_runtime::session_api::DatabaseMutationOperation;
    let operation = mutation.operation();
    let invalid = |field| {
        DatabaseUseCaseError::Database(DatabaseOperationError::InvalidInput {
            database_id: database_id.to_owned(),
            operation,
            field,
        })
    };
    match mutation {
        DatabaseMutation::EditCell {
            row,
            column,
            value,
            row_id,
        } => {
            let value = serde_json::from_value(value).map_err(|_| invalid("value"))?;
            Ok(DatabaseMutationOperation::EditCell {
                row,
                column: column.into_boxed_str(),
                value,
                row_id,
            })
        }
        DatabaseMutation::AddRow { index } => Ok(DatabaseMutationOperation::AddRow {
            index: index.unwrap_or(usize::MAX),
        }),
        DatabaseMutation::DeleteRows { indices, row_ids } => {
            let mut distinct_indices = indices;
            distinct_indices.sort_unstable();
            distinct_indices.dedup();
            if let Some(row_ids) = &row_ids
                && row_ids.len() != distinct_indices.len()
            {
                return Err(invalid("rowIds"));
            }
            Ok(DatabaseMutationOperation::DeleteRows {
                indices: distinct_indices.into_boxed_slice(),
                row_ids: row_ids.map(Vec::into_boxed_slice),
            })
        }
        DatabaseMutation::AddColumn { name, dtype } => Ok(DatabaseMutationOperation::AddColumn {
            name: name.into_boxed_str(),
            data_type: yss_tabular_arrow::editable_data_type(&dtype)
                .map_err(|_| invalid("dtype"))?,
        }),
        DatabaseMutation::DeleteColumn { name } => Ok(DatabaseMutationOperation::DeleteColumn {
            name: name.into_boxed_str(),
        }),
        DatabaseMutation::CastColumn {
            column,
            dtype,
            force,
        } => Ok(DatabaseMutationOperation::CastColumn {
            name: column.into_boxed_str(),
            data_type: yss_tabular_arrow::editable_data_type(&dtype)
                .map_err(|_| invalid("dtype"))?,
            force,
        }),
        DatabaseMutation::RenameColumn { old_name, new_name } => {
            Ok(DatabaseMutationOperation::RenameColumn {
                old_name: old_name.into_boxed_str(),
                new_name: new_name.into_boxed_str(),
            })
        }
        DatabaseMutation::Undo => Ok(DatabaseMutationOperation::Undo),
        DatabaseMutation::Redo => Ok(DatabaseMutationOperation::Redo),
    }
}

pub fn list_sqlite_tables(path: &str) -> Result<Vec<String>, DatabaseOperationError> {
    list_sql_source_tables(&DatabaseEngineSql::Sqlite { auto_create: false }, path).map_err(
        |error| {
            DatabaseOperationError::internal_message(
                DatabaseApplicationOperation::ListTables,
                error.to_string(),
            )
        },
    )
}

pub fn list_sql_tables(
    engine: &str,
    connection_string: &str,
) -> Result<Vec<String>, DatabaseOperationError> {
    let engine = match engine {
        "postgres" | "postgresql" => DatabaseEngineSql::Postgres { ssl: true },
        "mysql" | "mariadb" => DatabaseEngineSql::Mysql {
            charset: "utf8mb4".to_string(),
        },
        engine => {
            return Err(DatabaseOperationError::SqlEngineUnsupported {
                engine: engine.to_owned(),
            });
        }
    };
    list_sql_source_tables(&engine, connection_string).map_err(|error| {
        DatabaseOperationError::internal_message(
            DatabaseApplicationOperation::ListTables,
            error.to_string(),
        )
    })
}

pub fn list_excel_sheets(path: &str) -> Result<Vec<String>, DatabaseOperationError> {
    list_workbook_sheets(Path::new(path)).map_err(|error| {
        DatabaseOperationError::internal(DatabaseApplicationOperation::ListSheets, error)
    })
}

/// Editable DataView operations admitted by the database runtime.
pub enum DatabaseMutation {
    EditCell {
        row: usize,
        column: String,
        value: serde_json::Value,
        row_id: Option<i64>,
    },
    AddRow {
        index: Option<usize>,
    },
    DeleteRows {
        indices: Vec<usize>,
        row_ids: Option<Vec<i64>>,
    },
    AddColumn {
        name: String,
        dtype: String,
    },
    DeleteColumn {
        name: String,
    },
    CastColumn {
        column: String,
        dtype: String,
        force: bool,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    Undo,
    Redo,
}

impl DatabaseMutation {
    pub fn operation(&self) -> DatabaseApplicationOperation {
        match self {
            Self::EditCell { .. } => DatabaseApplicationOperation::EditCell,
            Self::AddRow { .. } => DatabaseApplicationOperation::AddRow,
            Self::DeleteRows { .. } => DatabaseApplicationOperation::DeleteRows,
            Self::AddColumn { .. } => DatabaseApplicationOperation::AddColumn,
            Self::DeleteColumn { .. } => DatabaseApplicationOperation::DeleteColumn,
            Self::CastColumn { .. } => DatabaseApplicationOperation::CastColumn,
            Self::RenameColumn { .. } => DatabaseApplicationOperation::RenameColumn,
            Self::Undo => DatabaseApplicationOperation::UndoEdit,
            Self::Redo => DatabaseApplicationOperation::RedoEdit,
        }
    }
}
