mod dispatcher;
mod dto;
mod runtime;
mod rust_projection;
mod validation;
mod worker;

pub use dispatcher::LogsUnavailable;
pub use dto::{
    FrontendLogEntryDto, LogBatchDto, LogDomain, LogOrigin, LogRecordDto, LogSubscriptionDto,
};
pub use runtime::{LogInitializationError, LogRuntime, SubmitFrontendLogsError};

fn local_timestamp_now() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.3f")
        .to_string()
}

#[cfg(test)]
mod tests;
