//! Log collection, SQLite history and live channels. Business diagnostics have separate owners.

mod collection;
mod collector;
mod retention;
mod store;
mod stream;

#[cfg(test)]
mod persistence_tests;
#[cfg(test)]
mod retention_tests;

pub use collection::{LogCollection, LogCollectionInitializationError};
pub use collector::{LogFields, LogLevel, LogRecord};
pub use retention::LogRetentionPolicy;
pub use store::{LOG_DATABASE_NAME, LogStoreError};
pub use stream::{
    LogBatchDto, LogDomain, LogInitializationError, LogRecordDto, LogRuntime, LogStreamFailure,
    LogSubscriptionDto, LogsUnavailable,
};
