use super::*;

pub(super) fn load_database_in_captured_session(
    captured: &Arc<ApplicationSession>,
    operation_id: OperationId,
    engine: DatabaseEngine,
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
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;
    let lease = captured
        .project()
        .acquire_filesystem_lease(session.root.clone())
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
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
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
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
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::internal_message(
                    DatabaseApplicationOperation::Load,
                    "DuckDb datasets are discovered from the active Database session",
                ),
            ));
        }
        DatabaseEngine::InMemory { .. } => {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::internal_message(
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
            DatabaseUseCaseError::Database(DatabaseOperationError::from_project_filesystem(
                error,
                DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
            ))
        })?;
    drop(lease);

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
    reservation.complete();
    Ok(DatabaseMutationResult {
        data,
        mutation: crate::events::committed_resource_mutation_from_project(mutation),
    })
}

fn ingest_csv_for_application(
    project: &ProjectState,
    session: &ProjectSession,
    path: String,
    delimiter: char,
    has_header: bool,
    infer_schema_length: Option<usize>,
) -> Result<(DatabaseDecl, LoadDatabaseResult), DatabaseUseCaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
        DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
) -> Result<(DatabaseDecl, LoadDatabaseResult), DatabaseUseCaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta = ingest_parquet_to_duckdb(Path::new(&path), &duckdb_abs, &table, columns.as_deref())
        .map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
) -> Result<(DatabaseDecl, LoadDatabaseResult), DatabaseUseCaseError> {
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta =
        ingest_excel_to_duckdb(Path::new(&path), &sheet, &duckdb_abs, &table).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
) -> Result<(DatabaseDecl, LoadDatabaseResult), DatabaseUseCaseError> {
    let mut dataframe =
        read_table_to_dataframe(&engine, &connection_string, &table_name).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let (id, table, duckdb_abs, relative_path) =
        prepare_duckdb_ingest_paths(session).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
                DatabaseApplicationOperation::Load,
                error.to_string(),
            ))
        })?;
    let meta =
        ingest_dataframe_to_duckdb(&mut dataframe, &duckdb_abs, &table).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
    meta: DuckDbTableMeta,
) -> Result<(DatabaseDecl, LoadDatabaseResult), DatabaseUseCaseError> {
    let name = unique_database_name(project, &name_from_path(&source_name));
    write_display_name(&duckdb_abs, &table, &name).map_err(|error| {
        DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
            DatabaseApplicationOperation::Load,
            error.to_string(),
        ))
    })?;
    let database_id = database_id(&id);
    let fact = DatabaseSchemaFact::from_duckdb(&database_id, &meta.columns).map_err(|error| {
        DatabaseUseCaseError::Database(DatabaseOperationError::internal_message(
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
