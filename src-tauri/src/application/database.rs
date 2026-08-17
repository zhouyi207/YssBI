use std::path::{Path, PathBuf};

pub use crate::application::database_schema::name_from_path;
use crate::application::database_schema::{
    DatabaseSchemaSnapshot, column_info_from_duckdb, database_display_name, extract_database_schema,
};
use crate::database::{
    DatabaseDecl, DatabaseEngine, DatabaseEngineSql, DatabaseInstance, DatabaseState,
    drop_data_table, ingest_csv_to_duckdb, ingest_dataframe_to_duckdb, ingest_excel_to_duckdb,
    ingest_parquet_to_duckdb, read_table_meta, sql_reader, write_display_name,
};
use crate::database::{EditHistory, EditState};
use crate::error::CommandError;
use crate::project::{
    ProjectDatabaseError, ProjectFilesystemError, ProjectInstanceId, ProjectSession, ProjectState,
    relative_project_duckdb_path, unique_name,
};
use crate::schema::{ColumnInfoDTO, DatabaseEngineDTO};
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
    operation_id: crate::node_system::document::OperationId,
}

pub fn load_database(
    state: &ProjectState,
    project_instance_id: &crate::project::ProjectInstanceId,
    operation_id: crate::node_system::document::OperationId,
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

pub fn bind_duckdb_instance(decl: &DatabaseDecl, project_root: Option<&Path>) -> DatabaseInstance {
    let DatabaseEngine::DuckDb { path, table, .. } = &decl.engine else {
        unreachable!("bind_duckdb_instance expects DuckDb engine");
    };

    let state = match project_root {
        Some(root) => {
            let abs = root.join(path);
            match read_table_meta(&abs, table) {
                Ok(meta) => DatabaseState::DuckDb {
                    duckdb_path: abs.to_string_lossy().to_string(),
                    table: table.clone(),
                    row_count: meta.row_count,
                    columns: meta.columns,
                    history: EditHistory::new(),
                },
                Err(error) => DatabaseState::Failed { error },
            }
        }
        None => DatabaseState::Failed {
            error: "Project path not set; cannot bind DuckDB database".into(),
        },
    };

    DatabaseInstance {
        decl: decl.clone(),
        state,
    }
}

pub fn get_database_meta(state: &ProjectState, id: &str) -> Result<DatabaseMetaResult, String> {
    state
        .ensure_project_operational()
        .map_err(|error| format!("{}: {error}", error.code()))?;
    let store = state.project_store.read().unwrap();
    let db = store.databases.get(id).ok_or("Database not found")?;
    get_database_meta_from_instance(id, db)
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

fn database_project_error(error: crate::project::ProjectFilesystemError) -> CommandError {
    CommandError::from(error)
}

fn reserve_export_temporary_file(destination: &Path) -> Result<PathBuf, CommandError> {
    let parent = destination
        .parent()
        .ok_or_else(|| CommandError::expected("database_export_temp_reservation_failed"))?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| CommandError::expected("database_export_temp_reservation_failed"))?;
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
                return Err(CommandError::diagnosed(
                    "database_export_temp_reservation_failed",
                    error,
                ));
            }
        }
    }
    Err(CommandError::diagnosed(
        "database_export_temp_reservation_failed",
        "unable to reserve a unique sibling export path",
    ))
}

pub(crate) fn cleanup_export_temporary_file(temporary: &Path) -> Result<(), CommandError> {
    match std::fs::remove_file(temporary) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::diagnosed(
            "database_export_cleanup_failed",
            error,
        )),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandErrorSummary {
    code: &'static str,
    incident_id: Option<String>,
}

impl CommandErrorSummary {
    fn from_error(error: &CommandError) -> Self {
        Self {
            code: error.code(),
            incident_id: error.incident_id().map(str::to_owned),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupErrorDetails {
    cleanup_error: CommandErrorSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrimaryErrorDetails {
    primary_error: CommandErrorSummary,
}

fn cleanup_after_export_error(temporary: &Path, primary: CommandError) -> CommandError {
    let Err(cleanup) = cleanup_export_temporary_file(temporary) else {
        return primary;
    };
    if primary.code() == "stale_project_lifecycle" {
        let details = CleanupErrorDetails {
            cleanup_error: CommandErrorSummary::from_error(&cleanup),
        };
        primary.with_details(details)
    } else {
        let details = PrimaryErrorDetails {
            primary_error: CommandErrorSummary::from_error(&primary),
        };
        cleanup.with_details(details)
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
) -> Result<(), CommandError> {
    let mut dataframe = state
        .with_database_snapshot_for_project(project_instance_id, id, |database| {
            database
                .access(crate::database::DatabaseAccess::Execution)
                .map(|view| view.dataframe)
                .map_err(|error| error.to_string())
        })
        .map_err(CommandError::from)?
        .map_err(|error| CommandError::diagnosed("database_computation_failed", error))?;
    let destination = Path::new(path);
    let temporary = reserve_export_temporary_file(destination)?;
    let result = (|| {
        crate::database::export_dataframe(
            &mut dataframe,
            temporary.to_string_lossy().as_ref(),
            format,
        )
        .map_err(|error| CommandError::diagnosed("database_export_serialization_failed", error))?;
        before_authority(&temporary);
        let _authority = state
            .acquire_database_publication_authority(project_instance_id)
            .map_err(database_project_error)?;
        at_final_publication(&temporary);
        atomic_replace_export(&temporary, destination)
            .map_err(|error| CommandError::diagnosed("database_export_publication_failed", error))
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(cleanup_after_export_error(&temporary, error)),
    }
}

pub(crate) fn export_database_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    path: &str,
    format: &str,
) -> Result<(), CommandError> {
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
    expected_revision: crate::node_system::document::ResourceRevision,
    name: &str,
    operation_id: crate::node_system::document::OperationId,
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

pub fn remove_duckdb_table_if_needed(
    engine: &DatabaseEngine,
    project_root: Option<&Path>,
) -> Result<(), String> {
    let Some((relative_path, table)) = engine.duckdb_table() else {
        return Ok(());
    };
    let Some(root) = project_root else {
        return Ok(());
    };
    drop_data_table(&root.join(relative_path), table)
}

/// Persist in-memory edits into the project's DuckDB table (`project.duckdb`).
/// DuckDB-backed datasets transition back to `DatabaseState::DuckDb` after a successful save.
pub fn save_database_changes(
    state: &ProjectState,
    project_instance_id: &crate::project::ProjectInstanceId,
    id: &str,
    expected_revision: crate::node_system::document::ResourceRevision,
    operation_id: crate::node_system::document::OperationId,
) -> Result<crate::event::ResourceMutationCommandResultDto<EditState>, ProjectDatabaseError> {
    state.with_database_writer(
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        |db, session| db.save_changes(Some(session.root.as_path())),
    )
}
