use crate::database_contract::DatabaseId;

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

#[derive(Debug, thiserror::Error)]
#[error("database operation failed")]
pub struct DatabaseError {
    code: DatabaseErrorCode,
    operation: DatabaseOperation,
    resource: Option<DatabaseId>,
    #[source]
    driver: Option<DatabaseDriverError>,
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
    #[error("SQLx driver failure")]
    Sqlx(#[source] sqlx::Error),
    #[error("DuckDB driver failure")]
    DuckDb(#[source] duckdb::Error),
    #[error("database filesystem failure")]
    Filesystem(#[source] std::io::Error),
    #[error("Polars driver failure")]
    Polars(#[source] polars::error::PolarsError),
}
