use super::*;

pub(super) fn export_database_in_captured_session(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    id: &str,
    path: &str,
    format: &str,
) -> Result<(), DatabaseUseCaseError> {
    let export_format = format.parse::<DatabaseExportFormat>().map_err(|_| {
        DatabaseOperationError::ExportUnsupported {
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
    let result: Result<(), DatabaseUseCaseError> = (|| {
        captured
            .database()
            .export_physical_to_path(&database, &temporary, export_format)
            .map_err(|error| {
                DatabaseUseCaseError::Database(map_database_runtime_error(
                    error,
                    DatabaseApplicationOperation::ExportSerialize,
                    id,
                ))
            })?;
        session_api::revalidate_query_basis(captured.database(), &basis).map_err(|error| {
            DatabaseUseCaseError::Database(map_database_runtime_error(
                error,
                DatabaseApplicationOperation::ExportRead,
                id,
            ))
        })?;
        state
            .revalidate_captured_session(captured)
            .map_err(DatabaseUseCaseError::SessionChanged)?;
        yss_file_replace::atomic_replace(&temporary, destination).map_err(|error| {
            DatabaseUseCaseError::Database(DatabaseOperationError::internal(
                DatabaseApplicationOperation::ExportPublish,
                error,
            ))
        })
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(cleanup_after_export_error(&temporary, error)),
    }
}

fn reserve_export_temporary_file(destination: &Path) -> Result<PathBuf, DatabaseOperationError> {
    let parent = destination
        .parent()
        .ok_or(DatabaseOperationError::InvalidExportDestination)?;
    let file_name = destination
        .file_name()
        .ok_or(DatabaseOperationError::InvalidExportDestination)?;
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
                return Err(DatabaseOperationError::internal(
                    DatabaseApplicationOperation::ExportReserve,
                    error,
                ));
            }
        }
    }
    Err(DatabaseOperationError::internal_message(
        DatabaseApplicationOperation::ExportReserve,
        "unable to reserve a unique sibling export path",
    ))
}

fn cleanup_export_temporary_file(temporary: &Path) -> Result<(), DatabaseOperationError> {
    match std::fs::remove_file(temporary) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DatabaseOperationError::internal(
            DatabaseApplicationOperation::ExportCleanup,
            error,
        )),
    }
}

fn cleanup_after_export_error(
    temporary: &Path,
    primary: DatabaseUseCaseError,
) -> DatabaseUseCaseError {
    let Err(cleanup) = cleanup_export_temporary_file(temporary) else {
        return primary;
    };
    match primary {
        DatabaseUseCaseError::Database(primary) => {
            DatabaseUseCaseError::Database(DatabaseOperationError::CleanupAfterFailure {
                primary: Box::new(primary),
                cleanup: Box::new(cleanup),
            })
        }
        primary => {
            tracing::error!(
                target: "yssbi::database",
                diagnostic_domain = "application",
                diagnostic_event = "databaseExportCleanupFailed",
                error = ?cleanup,
                "database export cleanup failed after a session transition"
            );
            primary
        }
    }
}
