use std::path::{Path, PathBuf};

mod error;

pub use self::error::{
    DatabaseApplicationError, DatabaseApplicationInternalError, DatabaseApplicationOperation,
};
pub use crate::application::database_schema::name_from_path;
use crate::application::database_schema::{
    DatabaseSchemaSnapshot, database_display_name, extract_database_schema,
};
use crate::database::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseExportFormat, DatabaseInstance,
    DatabaseState, ingest_csv_to_duckdb, ingest_dataframe_to_duckdb, ingest_excel_to_duckdb,
    ingest_parquet_to_duckdb, sql_reader, write_display_name,
};
use crate::database::{EditHistory, EditState};
use crate::project::{
    ProjectDatabaseError, ProjectFilesystemError, ProjectInstanceId, ProjectSession, ProjectState,
    relative_project_duckdb_path, unique_name,
};
use crate::schema::{ColumnInfoDTO, DatabaseEngineDTO, column_info_from_duckdb};
use serde::Serialize;
use uuid::Uuid;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfoDTO>,
}

#[derive(Clone, Copy)]
struct DatabaseCreateRequest<'a> {
    project_instance_id: &'a crate::project::ProjectInstanceId,
    operation_id: crate::project::OperationId,
}

pub fn load_database(
    state: &ProjectState,
    project_instance_id: &crate::project::ProjectInstanceId,
    operation_id: crate::project::OperationId,
    engine: DatabaseEngineDTO,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let request = DatabaseCreateRequest {
        project_instance_id,
        operation_id,
    };
    let reservation = state.reserve_database_operation(project_instance_id, operation_id)?;
    let (session, _lease) = state.acquire_database_write_lease()?;
    if &session.instance_id != project_instance_id {
        return Err(ProjectFilesystemError::StaleProjectLifecycle {
            message: "project instance changed".into(),
        }
        .into());
    }
    let result = match engine {
        DatabaseEngineDTO::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => load_csv_via_duckdb(
            state,
            &session,
            request,
            path,
            delimiter,
            has_header,
            infer_schema_length,
        ),
        DatabaseEngineDTO::Parquet { path, columns } => {
            load_parquet_via_duckdb(state, &session, request, path, columns)
        }
        DatabaseEngineDTO::Excel { path, sheet } => {
            load_excel_via_duckdb(state, &session, request, path, sheet)
        }
        DatabaseEngineDTO::Sql {
            engine,
            connection_string,
            table,
        } => {
            let engine_sql =
                DatabaseEngineSql::try_from(engine).map_err(ProjectDatabaseError::operation)?;
            load_sql_via_duckdb(
                state,
                &session,
                request,
                engine_sql,
                connection_string,
                table,
            )
        }
        DatabaseEngineDTO::DuckDb { .. } => Err(ProjectDatabaseError::operation(
            "DuckDb datasets are discovered from the project store; reopen the project to refresh",
        )),
        DatabaseEngineDTO::InMemory { .. } => Err(ProjectDatabaseError::operation(
            "InMemory datasets cannot be loaded via load_database",
        )),
    }?;
    reservation.complete();
    Ok(result)
}

fn load_csv_via_duckdb(
    state: &ProjectState,
    session: &ProjectSession,
    request: DatabaseCreateRequest<'_>,
    csv_path: String,
    delimiter: char,
    has_header: bool,
    infer_schema_length: Option<usize>,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(session)?;

    let meta = ingest_csv_to_duckdb(
        Path::new(&csv_path),
        &duckdb_abs,
        &table,
        delimiter,
        has_header,
        infer_schema_length,
    )
    .map_err(ProjectDatabaseError::operation)?;

    register_duckdb_instance(
        state,
        session,
        request,
        id,
        table,
        name_from_path(&csv_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn load_parquet_via_duckdb(
    state: &ProjectState,
    session: &ProjectSession,
    request: DatabaseCreateRequest<'_>,
    parquet_path: String,
    columns: Option<Vec<String>>,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(session)?;

    let meta = ingest_parquet_to_duckdb(
        Path::new(&parquet_path),
        &duckdb_abs,
        &table,
        columns.as_deref(),
    )
    .map_err(ProjectDatabaseError::operation)?;

    register_duckdb_instance(
        state,
        session,
        request,
        id,
        table,
        name_from_path(&parquet_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn load_excel_via_duckdb(
    state: &ProjectState,
    session: &ProjectSession,
    request: DatabaseCreateRequest<'_>,
    excel_path: String,
    sheet: String,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let (id, table, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(session)?;

    let meta = ingest_excel_to_duckdb(Path::new(&excel_path), &sheet, &duckdb_abs, &table)
        .map_err(ProjectDatabaseError::operation)?;

    register_duckdb_instance(
        state,
        session,
        request,
        id,
        table,
        name_from_path(&excel_path),
        meta,
        duckdb_abs,
        relative_path,
    )
}

fn prepare_duckdb_ingest_paths(
    session: &ProjectSession,
) -> Result<(String, String, PathBuf, String), ProjectDatabaseError> {
    crate::project::ensure_directory(&session.root.as_path().join(crate::project::DATABASE_DIR))
        .map_err(ProjectDatabaseError::operation)?;

    let id = format!("db-{}", Uuid::new_v4());
    let table = id.clone();
    let relative_path = relative_project_duckdb_path();
    let duckdb_abs = session.root.as_path().join(&relative_path);
    Ok((id, table, duckdb_abs, relative_path))
}

fn register_duckdb_instance(
    state: &ProjectState,
    session: &ProjectSession,
    request: DatabaseCreateRequest<'_>,
    id: String,
    table: String,
    base_name: String,
    meta: crate::database::DuckDbTableMeta,
    duckdb_abs: PathBuf,
    relative_path: String,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let name = unique_database_name(state, &base_name);
    write_display_name(&duckdb_abs, &table, &name).map_err(ProjectDatabaseError::operation)?;

    let columns = column_info_from_duckdb(&meta.columns);
    let column_count = columns.len();
    let row_count = meta.row_count;

    let engine_domain = DatabaseEngine::DuckDb {
        path: relative_path,
        table: table.clone(),
    };

    let decl = DatabaseDecl {
        id: id.clone(),
        engine: engine_domain,
        schema_version: 1,
        required: false,
        name: name.clone(),
    };

    let instance = DatabaseInstance {
        decl,
        state: DatabaseState::DuckDb {
            duckdb_path: duckdb_abs.to_string_lossy().to_string(),
            table,
            row_count,
            columns: meta.columns,
            history: EditHistory::new(),
        },
    };
    let mutation = state.commit_database_add_for_session(
        session,
        request.project_instance_id,
        request.operation_id,
        instance,
    )?;

    Ok(crate::event::ResourceMutationCommandResultDto {
        data: LoadDatabaseResult {
            id,
            name,
            row_count,
            column_count,
            columns,
        },
        mutation,
    })
}

fn load_sql_via_duckdb(
    state: &ProjectState,
    session: &ProjectSession,
    request: DatabaseCreateRequest<'_>,
    engine: DatabaseEngineSql,
    connection_string: String,
    table: String,
) -> Result<crate::event::ResourceMutationCommandResultDto<LoadDatabaseResult>, ProjectDatabaseError>
{
    let mut df = sql_reader::read_table_to_dataframe(&engine, &connection_string, &table)
        .map_err(ProjectDatabaseError::operation)?;
    let (id, table_id, duckdb_abs, relative_path) = prepare_duckdb_ingest_paths(session)?;
    let meta = ingest_dataframe_to_duckdb(&mut df, &duckdb_abs, &table_id)
        .map_err(ProjectDatabaseError::operation)?;
    let base_name = table.clone();
    register_duckdb_instance(
        state,
        session,
        request,
        id,
        table_id,
        base_name,
        meta,
        duckdb_abs,
        relative_path,
    )
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
    crate::database::excel_reader::list_sheets(path).map_err(|error| {
        DatabaseApplicationError::internal_message(DatabaseApplicationOperation::ListSheets, error)
    })
}

pub(crate) fn get_database_meta_from_instance(
    id: &str,
    db: &DatabaseInstance,
) -> Result<DatabaseMetaResult, String> {
    match extract_database_schema(db) {
        DatabaseSchemaSnapshot::Ready {
            name,
            columns,
            row_count,
            column_count,
        } => Ok(DatabaseMetaResult {
            id: id.to_string(),
            name,
            row_count,
            column_count,
            columns,
        }),
        DatabaseSchemaSnapshot::Failed { name, error } => {
            Err(format!("Database '{name}' failed to load: {error}"))
        }
    }
}

fn with_database_read_for_project<R>(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    operation: DatabaseApplicationOperation,
    require_ready: bool,
    read: impl FnOnce(&mut DatabaseInstance) -> Result<R, DatabaseApplicationError>,
) -> Result<R, DatabaseApplicationError> {
    state
        .with_database_snapshot_for_project(project_instance_id, id, |database| {
            if require_ready && matches!(&database.state, DatabaseState::Failed { .. }) {
                return Err(DatabaseApplicationError::InvalidAccess {
                    database_id: id.to_owned(),
                    operation,
                });
            }
            read(database)
        })
        .map_err(|error| {
            DatabaseApplicationError::from_project_database(
                error,
                operation,
                project_instance_id,
                Some(id),
                None,
                None,
            )
        })?
}

pub fn read_database_meta(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<DatabaseMetaResult, DatabaseApplicationError> {
    let operation = DatabaseApplicationOperation::ReadMetadata;
    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            get_database_meta_from_instance(id, database)
                .map_err(|error| DatabaseApplicationError::internal_message(operation, error))
        },
    )
}

pub fn read_database_rows(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    offset: usize,
    limit: usize,
) -> Result<crate::database::PageQueryResult, DatabaseApplicationError> {
    let operation = DatabaseApplicationOperation::ReadRows;
    if limit > crate::database::MAX_GET_DATAFRAME_ROWS {
        return Err(DatabaseApplicationError::RowLimitExceeded {
            database_id: id.to_owned(),
            operation,
            requested_rows: limit,
            max_rows: crate::database::MAX_GET_DATAFRAME_ROWS,
        });
    }

    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            database
                .query_page_with_rowids(offset, limit)
                .map_err(|error| DatabaseApplicationError::internal(operation, error))
        },
    )
}

pub fn read_column_statistics(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<Vec<crate::database::ColumnStats>, DatabaseApplicationError> {
    let operation = DatabaseApplicationOperation::ColumnStatistics;
    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            database
                .compute_column_stats()
                .map_err(|error| DatabaseApplicationError::internal(operation, error))
        },
    )
}

pub fn read_column_distributions(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<Vec<crate::database::ColumnDistribution>, DatabaseApplicationError> {
    let operation = DatabaseApplicationOperation::ColumnDistribution;
    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            database
                .compute_column_distributions()
                .map_err(|error| DatabaseApplicationError::internal(operation, error))
        },
    )
}

pub fn read_dataset_overview(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<crate::database::DatasetOverview, DatabaseApplicationError> {
    let operation = DatabaseApplicationOperation::DatasetOverview;
    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            database
                .compute_dataset_overview()
                .map_err(|error| DatabaseApplicationError::internal(operation, error))
        },
    )
}

pub fn read_database_edit_state(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<EditState, DatabaseApplicationError> {
    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        DatabaseApplicationOperation::ReadEditState,
        false,
        |database| Ok(database.edit_state()),
    )
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

pub(crate) fn export_database_for_project_with_before_publish(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    path: &str,
    format: &str,
    before_authority: impl FnOnce(&Path),
    at_final_publication: impl FnOnce(&Path),
) -> Result<(), DatabaseApplicationError> {
    let export_format = DatabaseExportFormat::parse(format).ok_or_else(|| {
        DatabaseApplicationError::ExportUnsupported {
            format: format.to_owned(),
        }
    })?;
    let read_operation = DatabaseApplicationOperation::ExportRead;
    let destination = Path::new(path);
    let temporary = reserve_export_temporary_file(destination)?;
    let result = (|| {
        state
            .with_database_snapshot_for_project(project_instance_id, id, |database| {
                if matches!(&database.state, DatabaseState::Failed { .. }) {
                    return Err(DatabaseApplicationError::InvalidAccess {
                        database_id: id.to_owned(),
                        operation: read_operation,
                    });
                }
                database
                    .export_to_path(&temporary, export_format)
                    .map_err(|error| {
                        DatabaseApplicationError::internal_message(
                            DatabaseApplicationOperation::ExportSerialize,
                            error,
                        )
                    })
            })
            .map_err(|error| {
                DatabaseApplicationError::from_project_database(
                    error,
                    read_operation,
                    project_instance_id,
                    Some(id),
                    None,
                    None,
                )
            })??;
        before_authority(&temporary);
        let _authority = state
            .acquire_database_publication_authority(project_instance_id)
            .map_err(|error| {
                DatabaseApplicationError::from_project_filesystem(
                    error,
                    DatabaseApplicationOperation::ExportPublish,
                    project_instance_id,
                    Some(id),
                )
            })?;
        at_final_publication(&temporary);
        atomic_replace_export(&temporary, destination).map_err(|error| {
            DatabaseApplicationError::internal(DatabaseApplicationOperation::ExportPublish, error)
        })
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(cleanup_after_export_error(&temporary, error)),
    }
}

pub fn export_database_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    path: &str,
    format: &str,
) -> Result<(), DatabaseApplicationError> {
    export_database_for_project_with_before_publish(
        state,
        project_instance_id,
        id,
        path,
        format,
        |_| {},
        |_| {},
    )
}

fn unique_database_name(state: &ProjectState, base_name: &str) -> String {
    let store = state.project_store.read().unwrap();
    let existing: Vec<String> = store
        .databases
        .values()
        .map(database_display_name)
        .collect();
    unique_name::unique_name(base_name, existing.iter().map(|s| s.as_str()))
}

pub fn rename_database(
    state: &ProjectState,
    project_instance_id: &crate::project::ProjectInstanceId,
    id: &str,
    expected_revision: crate::project::ResourceRevision,
    name: &str,
    operation_id: crate::project::OperationId,
) -> Result<crate::event::ResourceMutationCommandResultDto<()>, ProjectDatabaseError> {
    let reservation = state.reserve_database_operation(project_instance_id, operation_id)?;
    let (session, _lease) = state.acquire_database_write_lease()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectDatabaseError::InvalidName);
    }

    {
        let store = state.project_store.read().unwrap();
        if !store.databases.contains_key(id) {
            return Err(ProjectDatabaseError::DatabaseNotFound);
        }
        let duplicate = store
            .databases
            .iter()
            .any(|(other_id, db)| other_id != id && database_display_name(db) == name);
        if duplicate {
            return Err(ProjectDatabaseError::NameConflict);
        }
    }

    let (token, instance) = state.revisioned_database_snapshot_for_session(
        &session,
        project_instance_id,
        id,
        expected_revision,
    )?;
    let engine = instance.decl.engine;
    if let Some((relative_path, table)) = engine.duckdb_table() {
        let abs = session.root.as_path().join(relative_path);
        write_display_name(&abs, table, name).map_err(ProjectDatabaseError::operation)?;
    }
    run_database_external_io_test_hook();
    let mutation = state.commit_database_name(&session, &token, id, name, operation_id)?;
    let result = crate::event::ResourceMutationCommandResultDto { data: (), mutation };
    reservation.complete();
    Ok(result)
}

/// Persist in-memory edits into the project's DuckDB table (`project.duckdb`).
/// DuckDB-backed datasets transition back to `DatabaseState::DuckDb` after a successful save.
pub fn save_database_changes(
    state: &ProjectState,
    project_instance_id: &crate::project::ProjectInstanceId,
    id: &str,
    expected_revision: crate::project::ResourceRevision,
    operation_id: crate::project::OperationId,
) -> Result<crate::event::ResourceMutationCommandResultDto<EditState>, ProjectDatabaseError> {
    state.with_database_writer(
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        |db, session| db.save_changes(Some(session.root.as_path())),
    )
}

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

fn validate_database_mutation(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    mutation: &DatabaseMutation,
) -> Result<(), DatabaseApplicationError> {
    let operation = mutation.operation();
    if let DatabaseMutation::DeleteRows {
        indices,
        row_ids: Some(row_ids),
    } = mutation
    {
        let mut distinct_indices = indices.clone();
        distinct_indices.sort_unstable();
        distinct_indices.dedup();
        if row_ids.len() != distinct_indices.len() {
            return Err(DatabaseApplicationError::InvalidInput {
                database_id: id.to_owned(),
                operation,
                field: "rowIds",
            });
        }
    }

    with_database_read_for_project(
        state,
        project_instance_id,
        id,
        operation,
        true,
        |database| {
            match (mutation, &database.state) {
                (
                    DatabaseMutation::DeleteColumn { .. },
                    DatabaseState::DuckDb { row_count, .. },
                ) if *row_count > crate::database::MAX_DELETE_COLUMN_SNAPSHOT_ROWS => {
                    return Err(DatabaseApplicationError::RowLimitExceeded {
                        database_id: id.to_owned(),
                        operation,
                        requested_rows: *row_count,
                        max_rows: crate::database::MAX_DELETE_COLUMN_SNAPSHOT_ROWS,
                    });
                }
                (
                    DatabaseMutation::CastColumn { force: true, .. },
                    DatabaseState::DuckDb { .. },
                ) => {
                    return Err(DatabaseApplicationError::OperationUnsupported {
                        database_id: Some(id.to_owned()),
                        operation,
                    });
                }
                _ => {}
            }
            Ok(())
        },
    )
}

pub fn mutate_database_resource(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    expected_revision: crate::project::ResourceRevision,
    operation_id: crate::project::OperationId,
    mutation: DatabaseMutation,
) -> Result<crate::event::ResourceMutationCommandResultDto<EditState>, DatabaseApplicationError> {
    let operation = mutation.operation();
    validate_database_mutation(state, project_instance_id, id, &mutation)?;
    state
        .with_database_writer(
            project_instance_id,
            id,
            expected_revision,
            operation_id,
            move |database, _| match mutation {
                DatabaseMutation::EditCell {
                    row,
                    column,
                    value,
                    row_id,
                } => database.edit_cell(row, &column, value, row_id),
                DatabaseMutation::AddRow { index } => database.add_row(index),
                DatabaseMutation::DeleteRows { indices, row_ids } => {
                    database.delete_rows(&indices, row_ids.as_deref())
                }
                DatabaseMutation::AddColumn { name, dtype } => database.add_column(&name, &dtype),
                DatabaseMutation::DeleteColumn { name } => database.delete_column(&name),
                DatabaseMutation::CastColumn {
                    column,
                    dtype,
                    force,
                } => database.cast_column(&column, &dtype, force),
                DatabaseMutation::RenameColumn { old_name, new_name } => {
                    database.rename_column(&old_name, &new_name)
                }
                DatabaseMutation::Undo => database.undo_edit(),
                DatabaseMutation::Redo => database.redo_edit(),
            },
        )
        .map_err(|error| {
            DatabaseApplicationError::from_project_database(
                error,
                operation,
                project_instance_id,
                Some(id),
                Some(expected_revision),
                None,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectData;

    fn install_loaded_database(state: &ProjectState) -> ProjectInstanceId {
        let mut project = ProjectData::new();
        let decl = DatabaseDecl {
            id: "sales".into(),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };
        project.databases.insert("sales".into(), decl.clone());
        state.activate_project_fixture("database-application".into(), project);
        let dataframe = polars::df!("amount" => &[1_i64, 2_i64]).unwrap();
        state.project_store.write().unwrap().databases.insert(
            "sales".into(),
            DatabaseInstance {
                decl,
                state: DatabaseState::Loaded {
                    dataframe: std::sync::Arc::new(dataframe.clone()),
                    original: std::sync::Arc::new(dataframe),
                    history: EditHistory::new(),
                },
            },
        );
        state.capture_project_session().unwrap().instance_id
    }

    #[test]
    fn public_rows_use_case_reports_typed_row_limit() {
        let state = ProjectState::new();
        let project_instance_id = install_loaded_database(&state);
        let requested_rows = crate::database::MAX_GET_DATAFRAME_ROWS + 1;

        let error = read_database_rows(&state, &project_instance_id, "sales", 0, requested_rows)
            .unwrap_err();

        assert!(matches!(
            error,
            DatabaseApplicationError::RowLimitExceeded {
                database_id,
                operation: DatabaseApplicationOperation::ReadRows,
                requested_rows: actual,
                max_rows: crate::database::MAX_GET_DATAFRAME_ROWS,
            } if database_id == "sales" && actual == requested_rows
        ));
    }
}
