use super::*;
use arrow::record_batch::RecordBatchReader;
use yss_dataset_store::DatasetStore;
use yss_relational_contract::RelationControl;

struct TemporarySource(PathBuf);
impl Drop for TemporarySource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
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
    let mut temporary = None;
    let (source_name, reader): (String, Box<dyn RecordBatchReader>) = match source {
        DatabaseImportSource::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => {
            let delimiter = u8::try_from(u32::from(delimiter)).map_err(import_error)?;
            let reader = yss_tabular_io::read_csv_batches(
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
            let reader = yss_tabular_io::read_parquet_batches(Path::new(&path), 50_000, None)
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
            let reader = yss_tabular_io::read_parquet_batches(
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
            yss_tabular_io::export_excel_sheet_to_csv(Path::new(&path), &sheet, &staged.0)
                .map_err(import_error)?;
            let reader = yss_tabular_io::read_csv_batches(&staged.0, b',', true, 10_000, 50_000)
                .map_err(import_error)?;
            temporary = Some(staged);
            (path, Box::new(reader))
        }
        DatabaseImportSource::Sql {
            engine,
            connection_string,
            table,
        } => {
            let reader = yss_sql_source::read_table_batches(
                &engine,
                &connection_string,
                &table,
                control.clone(),
            )
            .map_err(import_error)?;
            (table, Box::new(reader))
        }
    };
    let names = store.catalog_metadata().map_err(import_error)?;
    let name = allocate_unique_display_name(
        &name_from_path(&source_name),
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
    let columns = yss_tabular_arrow::database_schema_fact(&id, &metadata.schema)
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
