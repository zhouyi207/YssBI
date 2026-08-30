use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

mod error;

pub use self::error::{
    DatabaseApplicationError, DatabaseApplicationInternalError, DatabaseApplicationOperation,
};
use crate::application::database_mutation::{
    DatabaseMutationRequest as RuntimeDatabaseMutationRequest, PreparedProjectDatabaseMutation,
    ProjectDatabaseFinalizeError, ProjectDatabaseMutationError, ProjectDatabaseMutationPort,
    ProjectDatabaseMutationReceipt,
};
use crate::application::events::CommittedResourceMutation;
pub fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed")
        .to_owned()
}
use crate::application::execution::session_slot::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, SessionCaptureError,
    SessionRevalidationError,
};
use crate::database::EditState;
use crate::database::error::{DatabaseError, DatabaseErrorCode};
use crate::database::schema_snapshot::DatabaseSchemaFact;
use crate::database::session_api;
use crate::database::{
    ingest_csv_to_duckdb, ingest_dataframe_to_duckdb, ingest_excel_to_duckdb,
    ingest_parquet_to_duckdb, sql_reader, write_display_name,
};
use crate::project::{
    ProjectDatabaseError, ProjectSession, ProjectState, relative_project_duckdb_path,
};
use uuid::Uuid;
use yss_database_contract::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseExportFormat, DatabaseId,
};
use yss_display_naming::allocate_unique_display_name;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_tabular_contract::TabularSnapshot;
use yss_tabular_io::list_excel_sheets as list_workbook_sheets;

#[cfg(test)]
static DATABASE_EXTERNAL_IO_TEST_HOOK: std::sync::Mutex<
    Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_database_external_io_test_hook(
    hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
) {
    *DATABASE_EXTERNAL_IO_TEST_HOOK.lock().unwrap() = hook;
}

#[cfg(test)]
fn run_database_external_io_test_hook() {
    if let Some(hook) = DATABASE_EXTERNAL_IO_TEST_HOOK.lock().unwrap().clone() {
        hook();
    }
}

#[cfg(not(test))]
fn run_database_external_io_test_hook() {}

#[derive(Debug)]
pub(crate) struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<crate::database::schema_snapshot::DatabaseColumnFact>,
}

#[derive(Debug)]
pub(crate) struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<crate::database::schema_snapshot::DatabaseColumnFact>,
}

#[derive(Debug)]
pub(crate) struct DatabaseRowsResult {
    pub rows: TabularSnapshot,
    pub row_ids: Vec<i64>,
}

#[derive(Debug)]
pub(crate) struct DatabaseMutationResult<T> {
    pub data: T,
    pub mutation: CommittedResourceMutation,
}

#[derive(Debug, Error)]
pub enum ApplicationDatabaseError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured application session changed during database operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application database session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
    #[error(transparent)]
    Database(#[from] DatabaseApplicationError),
    #[error("database mutation failed")]
    Mutation(#[source] crate::application::database_mutation::DatabaseMutationApplicationError),
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct DatabaseCreateRequest<'a> {
    project_instance_id: &'a yss_project_identity::ProjectInstanceId,
    operation_id: yss_project_identity::OperationId,
}

struct ProjectDatabaseAuthority<'a> {
    project: &'a ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    expected_project_revision: ResourceRevision,
    after: DatabaseDecl,
}

impl ProjectDatabaseMutationPort for ProjectDatabaseAuthority<'_> {
    fn prepare(
        &self,
        session_epoch: crate::application::execution::ApplicationSessionEpoch,
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
            session_epoch,
            request.database().clone(),
            request.expected_runtime_revision(),
            token,
        ))
    }

    fn finalize(
        &self,
        prepared: PreparedProjectDatabaseMutation,
        database: &crate::database::session_api::DatabaseRuntimeChangeOutcome,
    ) -> Result<ProjectDatabaseMutationReceipt, ProjectDatabaseFinalizeError> {
        let Some((session_epoch, database_id, expected_runtime_revision, token)) =
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
        self.project
            .commit_database_declaration_for_application(
                &self.project_instance_id,
                token,
                self.after.clone(),
                self.operation_id,
            )
            .map(|mutation| {
                ProjectDatabaseMutationReceipt::from_project(session_epoch, database_id, mutation)
            })
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
        engine: DatabaseEngine,
    ) -> Result<DatabaseMutationResult<LoadDatabaseResult>, ApplicationDatabaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = load_database_in_captured_session(&captured, operation_id, engine)?;
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
    ) -> Result<DatabaseMutationResult<()>, ApplicationDatabaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let declaration = captured
            .project()
            .get_data()
            .map_err(|error| {
                ApplicationDatabaseError::Database(
                    DatabaseApplicationError::from_project_filesystem(
                        error,
                        DatabaseApplicationOperation::Rename,
                        &project_instance_id,
                        Some(&id),
                    ),
                )
            })?
            .databases
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                ApplicationDatabaseError::Database(DatabaseApplicationError::NotFound {
                    database_id: id.clone(),
                })
            })?;
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::InvalidName {
                    database_id: id,
                    requested_name: name,
                },
            ));
        }
        if captured
            .project()
            .get_data()
            .map_err(|error| {
                ApplicationDatabaseError::Database(
                    DatabaseApplicationError::from_project_filesystem(
                        error,
                        DatabaseApplicationOperation::Rename,
                        &project_instance_id,
                        None,
                    ),
                )
            })?
            .databases
            .iter()
            .any(|(other_id, other)| other_id != &id && other.name.as_ref() == name)
        {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::NameConflict {
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
            crate::database::session_api::DatabaseMutationOperation::RenameDatabase {
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
    ) -> Result<DatabaseMutationResult<()>, ApplicationDatabaseError> {
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
    ) -> Result<DatabaseMutationResult<EditState>, ApplicationDatabaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let declaration = captured
            .project()
            .get_data()
            .map_err(|error| {
                ApplicationDatabaseError::Database(
                    DatabaseApplicationError::from_project_filesystem(
                        error,
                        DatabaseApplicationOperation::EditCell,
                        &project_instance_id,
                        Some(&id),
                    ),
                )
            })?
            .databases
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                ApplicationDatabaseError::Database(DatabaseApplicationError::NotFound {
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
    ) -> Result<DatabaseMutationResult<EditState>, ApplicationDatabaseError> {
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
    ) -> Result<DatabaseMetaResult, ApplicationDatabaseError> {
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_database_rows_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRowsResult, ApplicationDatabaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        if limit > crate::database::MAX_GET_DATAFRAME_ROWS {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::RowLimitExceeded {
                    database_id: id,
                    operation: DatabaseApplicationOperation::ReadRows,
                    requested_rows: limit,
                    max_rows: crate::database::MAX_GET_DATAFRAME_ROWS,
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_column_stats_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<Vec<yss_dataset_profile::ColumnStats>, ApplicationDatabaseError> {
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_column_distributions_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, ApplicationDatabaseError> {
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_dataset_overview_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<yss_dataset_profile::DatasetOverview, ApplicationDatabaseError> {
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_database_edit_state_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
    ) -> Result<EditState, ApplicationDatabaseError> {
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
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        Ok(result)
    }

    pub fn export_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        path: String,
        format: String,
    ) -> Result<(), ApplicationDatabaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        export_database_in_captured_session(self, &captured, &id, &path, &format)?;
        self.revalidate_captured_session(&captured)
            .map_err(ApplicationDatabaseError::SessionChanged)
    }

    fn capture_database_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, ApplicationDatabaseError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::StaleProject {
                    project_instance_id: project_instance_id.clone(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_database_session(&self) -> Result<(), ApplicationDatabaseError> {
        self.refresh_current_project()
            .map_err(ApplicationDatabaseError::SessionRefresh)
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
) -> Result<DatabaseMutationResult<()>, ApplicationDatabaseError> {
    let project_instance_id = captured.project_instance_id().clone();
    let reservation = captured
        .project()
        .reserve_database_operation(&project_instance_id, operation_id)
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_database(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
                Some(expected_revision),
                None,
            ))
        })?;
    let session = captured
        .project()
        .capture_project_session()
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
            ))
        })?;
    captured
        .project()
        .validate_project_session(&session)
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
            ))
        })?;
    let database = database_id(&id);
    if captured.database().revisions(&database).is_none() {
        return Err(ApplicationDatabaseError::Database(
            DatabaseApplicationError::NotFound {
                database_id: id.clone(),
            },
        ));
    }
    captured
        .database()
        .remove_physical_database(&database, session.root.as_path())
        .map_err(|error| {
            ApplicationDatabaseError::Database(map_database_runtime_error(
                error,
                DatabaseApplicationOperation::Delete,
                &id,
            ))
        })?;
    let mutation = captured
        .project()
        .commit_database_declaration_delete_for_application(
            &project_instance_id,
            &id,
            expected_revision,
            operation_id,
        )
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_database(
                error,
                DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(&id),
                Some(expected_revision),
                None,
            ))
        })?;
    reservation.complete();
    let _ = state;
    Ok(DatabaseMutationResult {
        data: (),
        mutation: crate::application::events::committed_resource_mutation_from_project(mutation),
    })
}

fn save_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<DatabaseMutationResult<EditState>, ApplicationDatabaseError> {
    let project_instance_id = captured.project_instance_id().clone();
    let declaration = captured
        .project()
        .get_data()
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
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
            ApplicationDatabaseError::Database(DatabaseApplicationError::NotFound {
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
        crate::database::session_api::DatabaseMutationOperation::Save,
        declaration,
        DatabaseApplicationOperation::Save,
    )?;
    Ok(DatabaseMutationResult {
        data: receipt.edit_state().clone(),
        mutation: receipt.mutation().clone(),
    })
}

fn export_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: &str,
    path: &str,
    format: &str,
) -> Result<(), ApplicationDatabaseError> {
    let export_format = format.parse::<DatabaseExportFormat>().map_err(|_| {
        DatabaseApplicationError::ExportUnsupported {
            format: format.to_owned(),
        }
    })?;
    let database = database_id(id);
    let basis = captured
        .database()
        .capture_query_basis(&database)
        .map_err(|error| {
            map_database_runtime_error(error, DatabaseApplicationOperation::ExportRead, id)
        })?;
    let destination = Path::new(path);
    let temporary = reserve_export_temporary_file(destination)?;
    let result: Result<(), ApplicationDatabaseError> = (|| {
        captured
            .database()
            .export_physical_to_path(&database, &temporary, export_format)
            .map_err(|error| {
                ApplicationDatabaseError::Database(map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ExportSerialize,
                    id,
                ))
            })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            ApplicationDatabaseError::Database(map_database_runtime_error(
                error,
                DatabaseApplicationOperation::ExportRead,
                id,
            ))
        })?;
        state
            .revalidate_captured_session(captured)
            .map_err(ApplicationDatabaseError::SessionChanged)?;
        atomic_replace_export(&temporary, destination).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal(
                DatabaseApplicationOperation::ExportPublish,
                error,
            ))
        })
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = cleanup_export_temporary_file(&temporary);
            Err(error)
        }
    }
}

fn load_database_in_captured_session(
    captured: &Arc<ApplicationSession>,
    operation_id: OperationId,
    engine: DatabaseEngine,
) -> Result<DatabaseMutationResult<LoadDatabaseResult>, ApplicationDatabaseError> {
    let project_instance_id = captured.project_instance_id().clone();
    let reservation = captured
        .project()
        .reserve_database_operation(&project_instance_id, operation_id)
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_database(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
                None,
                None,
            ))
        })?;
    let session = captured
        .project()
        .capture_project_session()
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;
    let lease = captured
        .project()
        .filesystem()
        .acquire(session.root.clone())
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;
    captured
        .project()
        .validate_project_session(&session)
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;

    let (declaration, data) = match engine {
        DatabaseEngine::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => ingest_csv_for_application(
            captured.project(),
            &session,
            path,
            delimiter,
            has_header,
            infer_schema_length,
        )?,
        DatabaseEngine::Parquet { path, columns } => {
            ingest_parquet_for_application(captured.project(), &session, path, columns)?
        }
        DatabaseEngine::Excel { path, sheet } => {
            ingest_excel_for_application(captured.project(), &session, path, sheet)?
        }
        DatabaseEngine::Sql {
            engine,
            connection_string,
            table,
        } => ingest_sql_for_application(
            captured.project(),
            &session,
            engine,
            connection_string,
            table,
        )?,
        DatabaseEngine::DuckDb { .. } => {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::internal_message(
                    DatabaseApplicationOperation::Load,
                    "DuckDb datasets are discovered from the active Database session",
                ),
            ));
        }
        DatabaseEngine::InMemory { .. } => {
            return Err(ApplicationDatabaseError::Database(
                DatabaseApplicationError::internal_message(
                    DatabaseApplicationOperation::Load,
                    "InMemory datasets cannot be loaded through the project importer",
                ),
            ));
        }
    };
    captured
        .project()
        .validate_project_session(&session)
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;
    drop(lease);

    let mutation = captured
        .project()
        .commit_database_declaration_add_for_application(
            &project_instance_id,
            declaration,
            operation_id,
        )
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::from_project_database(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
                None,
                None,
            ))
        })?;
    reservation.complete();
    Ok(DatabaseMutationResult {
        data,
        mutation: crate::application::events::committed_resource_mutation_from_project(mutation),
    })
}

fn ingest_csv_for_application(
    project: &ProjectState,
    session: &ProjectSession,
    path: String,
    delimiter: char,
    has_header: bool,
    infer_schema_length: Option<usize>,
) -> Result<(DatabaseDecl, LoadDatabaseResult), ApplicationDatabaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta = ingest_csv_to_duckdb(
        Path::new(&path),
        &duckdb_abs,
        &table,
        delimiter,
        has_header,
        infer_schema_length,
    )
    .map_err(|error| {
        ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
            DatabaseApplicationOperation::Load,
            error.to_string(),
        ))
    })?;
    build_import_declaration(project, path, id, table, duckdb_abs, relative_path, meta)
}

fn ingest_parquet_for_application(
    project: &ProjectState,
    session: &ProjectSession,
    path: String,
    columns: Option<Vec<String>>,
) -> Result<(DatabaseDecl, LoadDatabaseResult), ApplicationDatabaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta = ingest_parquet_to_duckdb(Path::new(&path), &duckdb_abs, &table, columns.as_deref())
        .map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error,
            ))
        })?;
    build_import_declaration(project, path, id, table, duckdb_abs, relative_path, meta)
}

fn ingest_excel_for_application(
    project: &ProjectState,
    session: &ProjectSession,
    path: String,
    sheet: String,
) -> Result<(DatabaseDecl, LoadDatabaseResult), ApplicationDatabaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta =
        ingest_excel_to_duckdb(Path::new(&path), &sheet, &duckdb_abs, &table).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error,
            ))
        })?;
    build_import_declaration(project, path, id, table, duckdb_abs, relative_path, meta)
}

fn ingest_sql_for_application(
    project: &ProjectState,
    session: &ProjectSession,
    engine: DatabaseEngineSql,
    connection_string: String,
    table_name: String,
) -> Result<(DatabaseDecl, LoadDatabaseResult), ApplicationDatabaseError> {
    let mut dataframe =
        sql_reader::read_table_to_dataframe(&engine, &connection_string, &table_name).map_err(
            |error| {
                ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                    DatabaseApplicationOperation::Load,
                    error,
                ))
            },
        )?;
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta =
        ingest_dataframe_to_duckdb(&mut dataframe, &duckdb_abs, &table).map_err(|error| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::Load,
                error,
            ))
        })?;
    build_import_declaration(
        project,
        table_name,
        id,
        table,
        duckdb_abs,
        relative_path,
        meta,
    )
}

fn build_import_declaration(
    project: &ProjectState,
    source_name: String,
    id: String,
    table: String,
    duckdb_abs: PathBuf,
    relative_path: String,
    meta: crate::database::DuckDbTableMeta,
) -> Result<(DatabaseDecl, LoadDatabaseResult), ApplicationDatabaseError> {
    let name = unique_database_name(project, &name_from_path(&source_name));
    write_display_name(&duckdb_abs, &table, &name).map_err(|error| {
        ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
            DatabaseApplicationOperation::Load,
            error.to_string(),
        ))
    })?;
    let database_id = database_id(&id);
    let fact = DatabaseSchemaFact::from_duckdb(&database_id, &meta.columns).map_err(|error| {
        ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
            DatabaseApplicationOperation::Load,
            error.to_string(),
        ))
    })?;
    let columns = fact.columns().to_vec();
    let declaration = DatabaseDecl {
        id: database_id,
        engine: DatabaseEngine::DuckDb {
            path: relative_path,
            table,
        },
        schema_version: 1,
        required: false,
        name: name.clone().into_boxed_str(),
    };
    Ok((
        declaration,
        LoadDatabaseResult {
            id,
            name,
            row_count: meta.row_count,
            column_count: columns.len(),
            columns,
        },
    ))
}

fn apply_database_mutation_in_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_project_revision: ResourceRevision,
    operation_id: OperationId,
    operation: crate::database::session_api::DatabaseMutationOperation,
    after: DatabaseDecl,
    application_operation: DatabaseApplicationOperation,
) -> Result<
    crate::application::database_mutation::DatabaseMutationApplicationReceipt,
    ApplicationDatabaseError,
> {
    let database = database_id(&id);
    let runtime_revision = captured
        .database()
        .revisions(&database)
        .map(|revisions| revisions.runtime)
        .ok_or_else(|| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::NotFound {
                database_id: id.clone(),
            })
        })?;
    let observations = captured.database().observations();
    let expected_observation = observations
        .iter()
        .find(|(database_id, _)| *database_id == &database)
        .map(|(_, observation)| observation.clone())
        .ok_or_else(|| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::NotFound {
                database_id: id.clone(),
            })
        })?;
    let next_revision = expected_observation
        .revision()
        .get()
        .checked_add(1)
        .ok_or_else(|| {
            ApplicationDatabaseError::Database(DatabaseApplicationError::internal_message(
                application_operation,
                "database revision exhausted",
            ))
        })?;
    let next_observation = yss_database_contract::DatabaseDeclarationObservation::new(
        yss_database_contract::DatabaseDeclarationRevision::from_existing(next_revision),
        yss_database_contract::DatabaseDeclarationFingerprint::from_decl(&after),
    );
    let runtime_request = RuntimeDatabaseMutationRequest::new(
        database,
        runtime_revision,
        expected_observation,
        next_observation,
        operation,
    );
    let authority = ProjectDatabaseAuthority {
        project: captured.project(),
        project_instance_id,
        operation_id,
        expected_project_revision,
        after,
    };
    crate::application::database_mutation::mutate_database_in_captured_session(
        state,
        captured,
        runtime_request,
        &authority,
    )
    .map_err(ApplicationDatabaseError::Mutation)
}

fn map_database_runtime_error(
    error: DatabaseError,
    operation: DatabaseApplicationOperation,
    database_id: &str,
) -> DatabaseApplicationError {
    let resource = error
        .resource()
        .map(|resource| resource.as_str())
        .unwrap_or(database_id)
        .to_owned();
    match error.code() {
        DatabaseErrorCode::NotFound => DatabaseApplicationError::NotFound {
            database_id: resource,
        },
        DatabaseErrorCode::InvalidRequest => DatabaseApplicationError::InvalidInput {
            database_id: resource,
            operation,
            field: "databaseId",
        },
        DatabaseErrorCode::AdmissionClosed
        | DatabaseErrorCode::Conflict
        | DatabaseErrorCode::Schema
        | DatabaseErrorCode::Unsupported => DatabaseApplicationError::InvalidAccess {
            database_id: resource,
            operation,
        },
        DatabaseErrorCode::Constraint
        | DatabaseErrorCode::Driver
        | DatabaseErrorCode::Cancelled
        | DatabaseErrorCode::Deadline => DatabaseApplicationError::internal(operation, error),
    }
}

fn runtime_database_mutation(
    database_id: &str,
    mutation: DatabaseMutation,
) -> Result<crate::database::session_api::DatabaseMutationOperation, ApplicationDatabaseError> {
    use crate::database::session_api::DatabaseMutationOperation;
    let operation = mutation.operation();
    let invalid = |field| {
        ApplicationDatabaseError::Database(DatabaseApplicationError::InvalidInput {
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
            data_type: dtype.parse().map_err(|_| invalid("dtype"))?,
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
            data_type: dtype.parse().map_err(|_| invalid("dtype"))?,
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

fn prepare_duckdb_ingest_paths(
    session: &ProjectSession,
) -> Result<(String, String, PathBuf, String), ProjectDatabaseError> {
    yss_project_filesystem::ensure_directory(
        &session
            .root
            .as_path()
            .join(yss_project_layout::DATABASE_DIR),
    )
    .map_err(ProjectDatabaseError::operation)?;

    let id = format!("db-{}", Uuid::new_v4());
    let table = id.clone();
    let relative_path = relative_project_duckdb_path();
    let duckdb_abs = session.root.as_path().join(&relative_path);
    Ok((id, table, duckdb_abs, relative_path))
}

pub fn list_sqlite_tables(path: &str) -> Result<Vec<String>, DatabaseApplicationError> {
    sql_reader::list_tables(&DatabaseEngineSql::Sqlite { auto_create: false }, path).map_err(
        |error| {
            DatabaseApplicationError::internal_message(
                DatabaseApplicationOperation::ListTables,
                error,
            )
        },
    )
}

pub fn list_sql_tables(
    engine: &str,
    connection_string: &str,
) -> Result<Vec<String>, DatabaseApplicationError> {
    let engine = match engine {
        "postgres" | "postgresql" => DatabaseEngineSql::Postgres { ssl: true },
        "mysql" | "mariadb" => DatabaseEngineSql::Mysql {
            charset: "utf8mb4".to_string(),
        },
        engine => {
            return Err(DatabaseApplicationError::SqlEngineUnsupported {
                engine: engine.to_owned(),
            });
        }
    };
    sql_reader::list_tables(&engine, connection_string).map_err(|error| {
        DatabaseApplicationError::internal_message(DatabaseApplicationOperation::ListTables, error)
    })
}

pub fn list_excel_sheets(path: &str) -> Result<Vec<String>, DatabaseApplicationError> {
    list_workbook_sheets(Path::new(path)).map_err(|error| {
        DatabaseApplicationError::internal(DatabaseApplicationOperation::ListSheets, error)
    })
}

fn reserve_export_temporary_file(destination: &Path) -> Result<PathBuf, DatabaseApplicationError> {
    let parent = destination
        .parent()
        .ok_or(DatabaseApplicationError::InvalidExportDestination)?;
    let file_name = destination
        .file_name()
        .ok_or(DatabaseApplicationError::InvalidExportDestination)?;
    for _ in 0..8 {
        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            file_name.to_string_lossy(),
            Uuid::new_v4()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                drop(file);
                return Ok(temporary);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(DatabaseApplicationError::internal(
                    DatabaseApplicationOperation::ExportReserve,
                    error,
                ));
            }
        }
    }
    Err(DatabaseApplicationError::internal_message(
        DatabaseApplicationOperation::ExportReserve,
        "unable to reserve a unique sibling export path",
    ))
}

pub(crate) fn cleanup_export_temporary_file(
    temporary: &Path,
) -> Result<(), DatabaseApplicationError> {
    match std::fs::remove_file(temporary) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DatabaseApplicationError::internal(
            DatabaseApplicationOperation::ExportCleanup,
            error,
        )),
    }
}

fn cleanup_after_export_error(
    temporary: &Path,
    primary: DatabaseApplicationError,
) -> DatabaseApplicationError {
    let Err(cleanup) = cleanup_export_temporary_file(temporary) else {
        return primary;
    };
    DatabaseApplicationError::CleanupAfterFailure {
        primary: Box::new(primary),
        cleanup: Box::new(cleanup),
    }
}

#[cfg(not(windows))]
fn atomic_replace_export(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::rename(temporary, destination)
}

#[cfg(windows)]
fn atomic_replace_export(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn unique_database_name(state: &ProjectState, base_name: &str) -> String {
    state
        .get_data()
        .map(|data| {
            allocate_unique_display_name(
                base_name,
                data.databases
                    .values()
                    .map(|database| database.name.as_ref()),
            )
        })
        .unwrap_or_else(|_| base_name.to_owned())
}

/// Persist in-memory edits into the project's DuckDB table (`project.duckdb`).
/// DuckDB-backed datasets transition back to `DatabaseState::DuckDb` after a successful save.
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
