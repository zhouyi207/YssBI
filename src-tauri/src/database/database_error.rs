/// Database error types
///
/// Defines all error types that can occur during database operations.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Database not found: {0}")]
    NotFound(String),

    #[error("Failed to connect: {0}")]
    ConnectionError(String),

    #[error("Failed to parse data: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Polars error: {0}")]
    PolarsError(#[from] polars::error::PolarsError),

    #[error("Invalid operation for current state")]
    InvalidState,

    #[error("Data source type mismatch")]
    TypeMismatch,
}
