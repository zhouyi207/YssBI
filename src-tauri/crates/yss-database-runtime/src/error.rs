//! Stable runtime error classification without retaining private driver payloads.

use yss_database_contract::DatabaseId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DatabaseErrorCode {
    InvalidRequest,
    AdmissionClosed,
    NotFound,
    Conflict,
    Schema,
    Constraint,
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
    Recovery,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
#[error("database operation failed")]
pub struct DatabaseError {
    code: DatabaseErrorCode,
    operation: DatabaseOperation,
    resource: Option<DatabaseId>,
}

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

    pub fn invalid_request(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::new(DatabaseErrorCode::InvalidRequest, operation, resource)
    }

    pub(crate) fn admission_closed(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
    ) -> Self {
        Self::new(DatabaseErrorCode::AdmissionClosed, operation, resource)
    }

    pub(crate) fn not_found(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::new(DatabaseErrorCode::NotFound, operation, resource)
    }

    pub(crate) fn conflict(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::new(DatabaseErrorCode::Conflict, operation, resource)
    }

    pub fn schema(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::new(DatabaseErrorCode::Schema, operation, resource)
    }

    pub(crate) fn driver(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::new(DatabaseErrorCode::Driver, operation, resource)
    }

    pub fn dataset(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
        source: yss_database_store::DatasetStoreError,
    ) -> Self {
        use yss_database_store::DatasetStoreError as Store;
        use yss_relational_contract::RelationError;
        let code = match &source {
            Store::NotFound | Store::RowNotFound => DatabaseErrorCode::NotFound,
            Store::Conflict => DatabaseErrorCode::Conflict,
            Store::InvalidValue | Store::DeltaLimit => DatabaseErrorCode::Constraint,
            Store::InvalidSchema | Store::CorruptCatalog | Store::UnsupportedFormat => {
                DatabaseErrorCode::Schema
            }
            Store::InvalidIdentity => DatabaseErrorCode::InvalidRequest,
            Store::Query(RelationError::Cancelled) => DatabaseErrorCode::Cancelled,
            Store::Query(RelationError::DeadlineExceeded) => DatabaseErrorCode::Deadline,
            Store::Query(RelationError::MemoryLimitExceeded) => DatabaseErrorCode::Constraint,
            _ => DatabaseErrorCode::Driver,
        };
        Self::new(code, operation, resource)
    }

    fn new(
        code: DatabaseErrorCode,
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
    ) -> Self {
        Self {
            code,
            operation,
            resource,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DatabaseError, DatabaseOperation};

    #[test]
    fn database_error_redacts_driver_details_from_public_views() {
        let secret = "driver detail: SELECT token FROM secrets";
        let error = DatabaseError::dataset(
            DatabaseOperation::Query,
            None,
            yss_database_store::DatasetStoreError::Batch(arrow::error::ArrowError::ComputeError(
                secret.into(),
            )),
        );

        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
        assert!(std::error::Error::source(&error).is_none());
    }
}
