use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;
use yss_database_contract::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseId, DatabaseImportSource,
};
use yss_database_io::list_excel_sheets as list_workbook_sheets;
use yss_database_schema::DatabaseColumnFact;
use yss_database_source::list_tables as list_sql_source_tables;
use yss_display_naming::allocate_unique_display_name;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::export::export_database_in_captured_session;
use super::mutation::check_database_revision;
use super::query::query_database_metadata_in_captured_session;
use super::{
    DatabaseApplicationOperation, DatabaseMutationResult, DatabaseOperationError,
    DatabaseUseCaseError,
};
use crate::session::{ApplicationSession, ApplicationState};
use arrow::record_batch::RecordBatchReader;
use yss_database_store::DatasetStore;
use yss_relational_contract::RelationControl;

struct TemporarySource(PathBuf);
impl Drop for TemporarySource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(super) struct ImportReader {
    name: String,
    reader: Box<dyn RecordBatchReader>,
    temporary: Option<TemporarySource>,
}

impl ImportReader {
    pub(super) fn new(name: String, reader: Box<dyn RecordBatchReader>) -> Self {
        Self {
            name,
            reader,
            temporary: None,
        }
    }
}

fn import_error(error: impl std::fmt::Display) -> DatabaseUseCaseError {
    DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
        DatabaseApplicationOperation::Load,
        error.to_string(),
    ))
}

pub(super) fn load_database_in_captured_session(
    captured: &Arc<ApplicationSession>,
    operation_id: OperationId,
    source: DatabaseImportSource,
    name: Option<String>,
) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
    import_in_captured_session(captured, operation_id, |control| {
        let mut reader = read_source(source, control)?;
        if let Some(name) = name {
            if name.trim().is_empty() {
                return Err(DatabaseUseCaseError::Database(
                    DatabaseOperationError::InvalidName {
                        database_id: String::new(),
                        requested_name: name,
                    },
                ));
            }
            reader.name = name;
        }
        Ok(reader)
    })
}

pub(super) fn duplicate_database_in_captured_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    name: Option<String>,
) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
    let metadata =
        query_database_metadata_in_captured_session(application, captured, id, expected_revision)?;
    let temporary = TemporarySource(
        std::env::temp_dir().join(format!("yss-database-copy-{}.parquet", Uuid::new_v4())),
    );
    std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(&temporary.0)
        .map_err(import_error)?;
    export_database_in_captured_session(
        application,
        captured,
        id,
        &temporary.0.to_string_lossy(),
        "parquet",
        Some(expected_revision),
    )?;
    import_in_captured_session(captured, operation_id, move |_| {
        // The import owns the project filesystem lease before checking the source again.
        check_database_revision(
            captured,
            id,
            expected_revision,
            DatabaseApplicationOperation::Load,
        )?;
        let reader = yss_database_io::read_parquet_batches(&temporary.0, 50_000, None)
            .map_err(import_error)?;
        Ok(ImportReader {
            name: name.unwrap_or_else(|| format!("{} Copy", metadata.name)),
            reader: Box::new(reader),
            temporary: Some(temporary),
        })
    })
}

fn read_source(
    source: DatabaseImportSource,
    control: RelationControl,
) -> Result<ImportReader, DatabaseUseCaseError> {
    let mut temporary = None;
    let (source_name, reader): (String, Box<dyn RecordBatchReader>) = match source {
        DatabaseImportSource::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => {
            let delimiter = u8::try_from(u32::from(delimiter)).map_err(import_error)?;
            let reader = yss_database_io::read_csv_batches(
                Path::new(&path),
                delimiter,
                has_header,
                infer_schema_length.unwrap_or(10_000),
                50_000,
            )
            .map_err(import_error)?;
            (path, Box::new(reader))
        }
        DatabaseImportSource::Parquet { path, columns } => {
            let reader = yss_database_io::read_parquet_batches(Path::new(&path), 50_000, None)
                .map_err(import_error)?;
            let projection = columns
                .map(|columns| {
                    columns
                        .iter()
                        .map(|name| reader.schema().index_of(name))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()
                .map_err(import_error)?;
            let reader = yss_database_io::read_parquet_batches(
                Path::new(&path),
                50_000,
                projection.as_deref(),
            )
            .map_err(import_error)?;
            (path, Box::new(reader))
        }
        DatabaseImportSource::Excel { path, sheet } => {
            let staged = TemporarySource(
                std::env::temp_dir().join(format!("yss-excel-import-{}.csv", Uuid::new_v4())),
            );
            std::fs::File::options()
                .write(true)
                .create_new(true)
                .open(&staged.0)
                .map_err(import_error)?;
            yss_database_io::export_excel_sheet_to_csv(Path::new(&path), &sheet, &staged.0)
                .map_err(import_error)?;
            let reader = yss_database_io::read_csv_batches(&staged.0, b',', true, 10_000, 50_000)
                .map_err(import_error)?;
            temporary = Some(staged);
            (path, Box::new(reader))
        }
        DatabaseImportSource::Sql {
            engine,
            connection_string,
            table,
        } => {
            let reader = yss_database_source::read_table_batches(
                &engine,
                &connection_string,
                &table,
                control.clone(),
            )
            .map_err(import_error)?;
            (table, Box::new(reader))
        }
    };
    Ok(ImportReader {
        name: name_from_path(&source_name),
        reader,
        temporary,
    })
}

pub(super) fn import_in_captured_session(
    captured: &Arc<ApplicationSession>,
    operation_id: OperationId,
    read_source: impl FnOnce(RelationControl) -> Result<ImportReader, DatabaseUseCaseError>,
) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
    let project_instance_id = captured.project_instance_id().clone();
    let reservation = captured
        .project()
        .reserve_database_operation(&project_instance_id, operation_id)
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_database(
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
        .map_err(import_error)?;
    let lease = captured
        .project()
        .acquire_filesystem_lease(session.root.clone())
        .map_err(import_error)?;
    captured
        .project()
        .validate_project_session(&session)
        .map_err(import_error)?;
    let store = DatasetStore::open(session.root.as_path()).map_err(import_error)?;
    let control = RelationControl {
        cancellation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(300),
        max_input_bytes: 16 * 1024 * 1024,
    };
    let ImportReader {
        name: source_name,
        reader,
        temporary,
    } = read_source(control.clone())?;
    let names = store.catalog_metadata().map_err(import_error)?;
    let name = allocate_unique_display_name(
        &source_name,
        names.iter().map(|metadata| metadata.name.as_ref()),
    );
    let id = DatabaseId::from_existing(Uuid::new_v4().to_string().into());
    let schema = reader.schema();
    let batches = reader.map(|batch| {
        control
            .check()
            .map_err(|error| arrow::error::ArrowError::ExternalError(Box::new(error)))?;
        batch
    });
    let prepared = store
        .prepare_import(
            id.clone(),
            &name,
            &operation_id.to_string(),
            schema,
            batches,
        )
        .map_err(import_error)?;
    drop(temporary);
    let metadata = prepared.metadata();
    let columns = yss_database_arrow::database_schema_fact(&id, &metadata.schema)
        .map_err(import_error)?
        .columns()
        .to_vec();
    let data = LoadDatabaseResult {
        id: id.as_str().into(),
        name: name.clone(),
        row_count: metadata.row_count,
        column_count: columns.len(),
        columns,
    };
    let declaration = DatabaseDecl {
        id,
        engine: DatabaseEngine::Dataset {},
        schema_version: yss_project::CURRENT_PROJECT_SCHEMA_VERSION,
        required: false,
        name: name.into(),
    };
    captured
        .project()
        .validate_project_session(&session)
        .map_err(import_error)?;
    let committed = store.commit(prepared).map_err(import_error)?;
    // Catalog commit is durable. A failed Project publication leaves its handoff for recovery.
    let mutation = captured
        .project()
        .commit_database_declaration_add(&project_instance_id, declaration, operation_id)
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_database(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
                None,
                None,
            ))
        })?;
    store
        .acknowledge_publication(&committed.publication)
        .map_err(import_error)?;
    reservation.complete();
    drop(lease);
    Ok(DatabaseMutationResult {
        data,
        mutation: crate::events::committed_resource_mutation_from_project(mutation),
    })
}

#[derive(Debug)]
pub struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<DatabaseColumnFact>,
}

impl ApplicationState {
    pub fn duplicate_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        id: String,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        name: Option<String>,
    ) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = duplicate_database_in_captured_session(
            self,
            &captured,
            &id,
            expected_revision,
            operation_id,
            name,
        )?;
        self.refresh_database_session(&captured)?;
        Ok(result)
    }

    pub fn load_database_for_application(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        source: DatabaseImportSource,
        name: Option<String>,
    ) -> Result<DatabaseMutationResult<LoadDatabaseResult>, DatabaseUseCaseError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let result = load_database_in_captured_session(&captured, operation_id, source, name)?;
        self.refresh_database_session(&captured)?;
        Ok(result)
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

fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed")
        .to_owned()
}
