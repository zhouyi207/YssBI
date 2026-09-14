//! Log collection, SQLite history and live channels. Business diagnostics have separate owners.

mod collector;
mod commands;
mod plugin;
mod store;
mod stream;

#[cfg(test)]
mod persistence_tests;

pub use collector::{LogFields, LogLevel, LogRecord};
pub use plugin::init;
pub use store::{LOG_DATABASE_NAME, LogPage, LogQuery, LogStatistics, LogStoreError};
pub use stream::{
    FrontendLogEntryDto, LogBatchDto, LogDomain, LogInitializationError, LogOrigin, LogRecordDto,
    LogRuntime, LogSubscriptionDto, LogsUnavailable, SubmitFrontendLogsError,
};
