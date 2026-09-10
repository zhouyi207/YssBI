//! Stable runtime error classification with redacted driver details.

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
    #[error("dataset export failed")]
    Dataset(#[source] yss_dataset_store::DatasetStoreError),
    #[error("relation export failed")]
    Relation(#[source] yss_relational_contract::RelationError),
    #[error("Arrow export failed")]
    Arrow(#[source] arrow::error::ArrowError),
    #[error("file export failed")]
    Io(#[source] std::io::Error),
    #[error("batch export failed")]
    Encoding(#[source] yss_tabular_io::TabularIoError),
}
impl From<yss_dataset_store::DatasetStoreError> for DatabaseExportError {
    fn from(source: yss_dataset_store::DatasetStoreError) -> Self {
        Self {
            source: DatabaseExportSource::Dataset(source),
        }
    }
}
impl From<yss_relational_contract::RelationError> for DatabaseExportError {
    fn from(source: yss_relational_contract::RelationError) -> Self {
        Self {
            source: DatabaseExportSource::Relation(source),
        }
    }
}
impl From<arrow::error::ArrowError> for DatabaseExportError {
    fn from(source: arrow::error::ArrowError) -> Self {
        Self {
            source: DatabaseExportSource::Arrow(source),
        }
    }
}
impl From<std::io::Error> for DatabaseExportError {
    fn from(source: std::io::Error) -> Self {
        Self {
            source: DatabaseExportSource::Io(source),
        }
    }
}
impl From<yss_tabular_io::TabularIoError> for DatabaseExportError {
    fn from(source: yss_tabular_io::TabularIoError) -> Self {
        Self {
            source: DatabaseExportSource::Encoding(source),
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

    pub fn schema(operation: DatabaseOperation, resource: Option<DatabaseId>) -> Self {
        Self::without_driver(DatabaseErrorCode::Schema, operation, resource)
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

    pub fn dataset(
        operation: DatabaseOperation,
        resource: Option<DatabaseId>,
        source: yss_dataset_store::DatasetStoreError,
    ) -> Self {
        use yss_dataset_store::DatasetStoreError as Store;
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
        Self {
            code,
            operation,
            resource,
            driver: Some(DatabaseDriverError::Dataset(source)),
        }
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
pub(crate) enum DatabaseDriverError {
    #[error("database export failed")]
    Export(#[source] DatabaseExportError),
    #[error("dataset driver failure")]
    Dataset(#[source] yss_dataset_store::DatasetStoreError),
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
            yss_dataset_store::DatasetStoreError::Batch(arrow::error::ArrowError::ComputeError(
                secret.into(),
            )),
        );

        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
        assert!(std::error::Error::source(&error).is_none());
    }
}
