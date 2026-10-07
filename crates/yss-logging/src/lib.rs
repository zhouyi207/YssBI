//! Log collection, SQLite history and live channels. Business diagnostics have separate owners.

mod collection;
mod collector;
mod store;
mod stream;

#[cfg(test)]
mod persistence_tests;

pub use collection::{LogCollection, LogCollectionInitializationError};
pub use collector::{LogFields, LogLevel, LogRecord};
pub use store::{LOG_DATABASE_NAME, LogPage, LogQuery, LogStatistics, LogStoreError};
pub use stream::{
    FrontendLogEntryDto, LogBatchDto, LogDomain, LogInitializationError, LogOrigin, LogRecordDto,
    LogRuntime, LogStreamFailure, LogSubscriptionDto, LogsUnavailable, SubmitFrontendLogsError,
};
