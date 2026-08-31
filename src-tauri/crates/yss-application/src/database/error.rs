use std::error::Error;
use std::fmt;

use yss_project::ProjectDatabaseError;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseApplicationOperation {
    Load,
    ListTables,
    ListSheets,
    ReadMetadata,
    ReadRows,
    ColumnStatistics,
    ColumnDistribution,
    DatasetOverview,
    ReadEditState,
    EditCell,
    AddRow,
    DeleteRows,
    AddColumn,
    DeleteColumn,
    CastColumn,
    RenameColumn,
    UndoEdit,
    RedoEdit,
    Save,
    Rename,
    Delete,
    ExportRead,
    ExportSerialize,
    ExportReserve,
    ExportPublish,
    ExportCleanup,
}

impl fmt::Display for DatabaseApplicationOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("database {operation} failed")]
pub struct DatabaseApplicationInternalError {
    operation: DatabaseApplicationOperation,
    #[source]
    source: Box<dyn Error + Send + Sync>,
}

impl DatabaseApplicationInternalError {
    pub fn operation(&self) -> DatabaseApplicationOperation {
        self.operation
    }

    fn new(
        operation: DatabaseApplicationOperation,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            operation,
            source: Box::new(source),
        }
    }

    fn message(operation: DatabaseApplicationOperation, message: impl Into<String>) -> Self {
        Self::new(operation, InternalMessage(message.into()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct InternalMessage(String);

#[derive(Debug, thiserror::Error)]
pub enum DatabaseApplicationError {
    #[error("database was not found")]
    NotFound { database_id: String },
    #[error("project instance is stale")]
    StaleProject {
        project_instance_id: ProjectInstanceId,
    },
    #[error("database revision is stale")]
    StaleRevision {
        database_id: String,
        expected_revision: ResourceRevision,
    },
    #[error("database cannot be accessed for {operation}")]
    InvalidAccess {
        database_id: String,
        operation: DatabaseApplicationOperation,
    },
    #[error("database row limit exceeded for {operation}")]
    RowLimitExceeded {
        database_id: String,
        operation: DatabaseApplicationOperation,
        requested_rows: usize,
        max_rows: usize,
    },
    #[error("database export format is unsupported")]
    ExportUnsupported { format: String },
    #[error("SQL engine is unsupported")]
    SqlEngineUnsupported { engine: String },
    #[error("database import source is unsupported")]
    ImportUnsupported { engine: &'static str },
    #[error("database name is invalid")]
    InvalidName {
        database_id: String,
        requested_name: String,
    },
    #[error("database name already exists")]
    NameConflict {
        database_id: String,
        requested_name: String,
    },
    #[error("database already exists")]
    AlreadyExists { database_id: Option<String> },
    #[error("database input is invalid")]
    InvalidInput {
        database_id: String,
        operation: DatabaseApplicationOperation,
        field: &'static str,
    },
    #[error("database operation is unsupported")]
    OperationUnsupported {
        database_id: Option<String>,
        operation: DatabaseApplicationOperation,
    },
    #[error("database export destination is invalid")]
    InvalidExportDestination,
    #[error("project database operation failed")]
    Project {
        project_instance_id: ProjectInstanceId,
        database_id: Option<String>,
        #[source]
        source: ProjectFilesystemError,
    },
    #[error("database export cleanup failed after another failure")]
    CleanupAfterFailure {
        primary: Box<DatabaseApplicationError>,
        #[source]
        cleanup: Box<DatabaseApplicationError>,
    },
    #[error(transparent)]
    Internal(#[from] DatabaseApplicationInternalError),
}

impl DatabaseApplicationError {
    #[cfg(any(test, feature = "test-support"))]
    pub fn internal_for_test(
        operation: DatabaseApplicationOperation,
        message: impl Into<String>,
    ) -> Self {
        Self::internal_message(operation, message)
    }

    pub(super) fn from_project_filesystem(
        source: ProjectFilesystemError,
        operation: DatabaseApplicationOperation,
        project_instance_id: &ProjectInstanceId,
        database_id: Option<&str>,
    ) -> Self {
        Self::from_project_database(
            ProjectDatabaseError::Project(source),
            operation,
            project_instance_id,
            database_id,
            None,
            None,
        )
    }

    pub(super) fn internal(
        operation: DatabaseApplicationOperation,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        DatabaseApplicationInternalError::new(operation, source).into()
    }

    pub(super) fn internal_message(
        operation: DatabaseApplicationOperation,
        message: impl Into<String>,
    ) -> Self {
        DatabaseApplicationInternalError::message(operation, message).into()
    }

    pub(crate) fn from_project_database(
        error: ProjectDatabaseError,
        operation: DatabaseApplicationOperation,
        project_instance_id: &ProjectInstanceId,
        database_id: Option<&str>,
        expected_revision: Option<ResourceRevision>,
        requested_name: Option<&str>,
    ) -> Self {
        match error {
            ProjectDatabaseError::Project(ProjectFilesystemError::StaleProjectLifecycle {
                ..
            }) => Self::StaleProject {
                project_instance_id: project_instance_id.clone(),
            },
            ProjectDatabaseError::Project(source) => Self::Project {
                project_instance_id: project_instance_id.clone(),
                database_id: database_id.map(str::to_owned),
                source,
            },
            ProjectDatabaseError::StaleDatabaseRevision => match (database_id, expected_revision) {
                (Some(database_id), Some(expected_revision)) => Self::StaleRevision {
                    database_id: database_id.to_owned(),
                    expected_revision,
                },
                _ => Self::internal_message(operation, "stale database revision lacked context"),
            },
            ProjectDatabaseError::DatabaseAlreadyExists => Self::AlreadyExists {
                database_id: database_id.map(str::to_owned),
            },
            ProjectDatabaseError::DatabaseNotFound => match database_id {
                Some(database_id) => Self::NotFound {
                    database_id: database_id.to_owned(),
                },
                None => {
                    Self::internal_message(operation, "database-not-found error lacked identity")
                }
            },
            ProjectDatabaseError::InvalidName => match (database_id, requested_name) {
                (Some(database_id), Some(requested_name)) => Self::InvalidName {
                    database_id: database_id.to_owned(),
                    requested_name: requested_name.to_owned(),
                },
                _ => Self::internal_message(operation, "invalid database name lacked context"),
            },
            ProjectDatabaseError::NameConflict => match (database_id, requested_name) {
                (Some(database_id), Some(requested_name)) => Self::NameConflict {
                    database_id: database_id.to_owned(),
                    requested_name: requested_name.to_owned(),
                },
                _ => Self::internal_message(operation, "database name conflict lacked context"),
            },
            ProjectDatabaseError::Operation(message) => Self::internal_message(operation, message),
        }
    }
}
