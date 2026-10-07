//! Shared encoding and database error mapping.

use yss_harness_contract::{PersistenceFailure, PersistenceFailureCode};

pub(crate) fn encode<T: serde::Serialize>(value: &T) -> Result<String, PersistenceFailure> {
    serde_json::to_string(value).map_err(|_| invalid_record())
}

pub(crate) fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, PersistenceFailure> {
    serde_json::from_str(value).map_err(|_| invalid_record())
}

pub(crate) fn require_updated(rows: u64) -> Result<(), PersistenceFailure> {
    if rows == 1 {
        Ok(())
    } else {
        Err(PersistenceFailure::new(PersistenceFailureCode::NotFound))
    }
}

pub(crate) fn map_insert_error(error: sqlx::Error) -> PersistenceFailure {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        conflict()
    } else {
        unavailable()
    }
}

pub(crate) fn conflict() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Conflict)
}

pub(crate) fn unavailable() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Unavailable)
}

pub(crate) fn invalid_record() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::InvalidRecord)
}
