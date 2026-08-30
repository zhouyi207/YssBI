use std::fmt;

use yss_database_contract::DatabaseId;

#[derive(Debug, thiserror::Error)]
#[error("database export failed")]
pub struct DatabaseExportError {
    #[source]
    source: DatabaseExportSource,
}

#[derive(Debug, thiserror::Error)]
enum DatabaseExportSource {
    #[error("database is unavailable for export")]
    Unavailable,
    #[error("DuckDB export failed")]
    DuckDb(#[source] yss_duckdb::DuckDbExportError),
    #[error("tabular I/O export failed")]
    TabularIo(#[source] yss_tabular_io::TabularIoError),
}

impl DatabaseExportError {
    pub(crate) fn unavailable() -> Self {
        Self {
            source: DatabaseExportSource::Unavailable,
        }
    }
}

impl From<yss_duckdb::DuckDbExportError> for DatabaseExportError {
    fn from(source: yss_duckdb::DuckDbExportError) -> Self {
        Self {
            source: DatabaseExportSource::DuckDb(source),
        }
    }
}

impl From<yss_tabular_io::TabularIoError> for DatabaseExportError {
    fn from(source: yss_tabular_io::TabularIoError) -> Self {
        Self {
            source: DatabaseExportSource::TabularIo(source),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DatabaseErrorCode {
    InvalidRequest,
    AdmissionClosed,
    NotFound,
    Conflict,
    Schema,
    Constraint,
    Unsupported,
    Driver,
    Cancelled,
    Deadline,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DatabaseOperation {
    OpenSession,
    CatalogSnapshot,
    DataSnapshot,
    PrepareMutation,
    CommitMutation,
    Query,
    Admission,
    Drain,
    Recovery,
}

#[derive(thiserror::Error)]
#[error("database operation failed")]
pub struct DatabaseError {
    code: DatabaseErrorCode,
    operation: DatabaseOperation,
    resource: Option<DatabaseId>,
    driver: Option<DatabaseDriverError>,
}

impl fmt::Debug for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseError")
            .field("code", &self.code)
            .field("operation", &self.operation)
            .field("resource", &self.resource)
            .finish()
    }
}

impl PartialEq for DatabaseError {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.operation == other.operation
            && self.resource == other.resource
    }
}

impl Eq for DatabaseError {}

#[allow(
    dead_code,
    reason = "constructors are staged for later database operation seams"
)]
impl DatabaseError {
    pub fn code(&self) -> DatabaseErrorCode {
        self.code
    }

    pub fn operation(&self) -> DatabaseOperation {
        self.operation
    }

    pub fn resource(&self) -> Option<&DatabaseId> {
        self.resource.as_ref()
    }

    pub(crate) fn invalid_request(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
    ) -> Self {
        Self::without_driver(DatabaseErrorCode::InvalidRequest, operation, resource)
    }

    pub(crate) fn admission_closed(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
    ) -> Self {
        Self::without_driver(DatabaseErrorCode::AdmissionClosed, operation, resource)
    }

    pub(crate) fn not_found(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::NotFound, operation, resource)
    }

    pub(crate) fn conflict(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Conflict, operation, resource)
    }

    pub(crate) fn schema(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Schema, operation, resource)
    }

    pub(crate) fn constraint(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Constraint, operation, resource)
    }

    pub(crate) fn unsupported(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Unsupported, operation, resource)
    }

    pub(crate) fn driver(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
        driver: DatabaseDriverError,
    ) -> Self {
        Self {
            code: DatabaseErrorCode::Driver,
            operation,
            resource,
            driver: Some(driver),
        }
    }

    pub(crate) fn cancelled(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Cancelled, operation, resource)
    }

    pub(crate) fn deadline(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Deadline, operation, resource)
    }

    fn without_driver(
        code: DatabaseErrorCode,
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
    ) -> Self {
        Self {
            code,
            operation,
            resource,
            driver: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[allow(
    dead_code,
    reason = "driver variants are staged until adapters move into Database"
)]
pub(crate) enum DatabaseDriverError {
    #[error("database operation failed")]
    Operation(Box<str>),
    #[error("database export failed")]
    Export(#[source] DatabaseExportError),
    #[error("SQLx driver failure")]
    Sqlx(#[source] sqlx::Error),
    #[error("DuckDB driver failure")]
    DuckDb(#[source] duckdb::Error),
    #[error("database filesystem failure")]
    Filesystem(#[source] std::io::Error),
    #[error("Polars driver failure")]
    Polars(#[source] polars::error::PolarsError),
}

#[cfg(test)]
mod tests {
    use super::{DatabaseDriverError, DatabaseError, DatabaseOperation};

    #[test]
    fn database_error_redacts_driver_details_from_public_views() {
        let secret = "driver detail: SELECT token FROM secrets";
        let error = DatabaseError::driver(
            DatabaseOperation::Query,
            None,
            DatabaseDriverError::Filesystem(std::io::Error::new(std::io::ErrorKind::Other, secret)),
        );

        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
        assert!(std::error::Error::source(&error).is_none());
    }
}
