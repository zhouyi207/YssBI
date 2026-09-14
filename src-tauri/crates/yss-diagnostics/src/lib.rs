//! Explicit diagnostic data and bounded delivery; no logging or subscriber integration.

mod dispatcher;
mod dto;
mod limits;
mod runtime;
mod sanitizer;
mod validation;
mod worker;

pub use dispatcher::DiagnosticsUnavailable;
pub use dto::{
    DiagnosticBatchDto, DiagnosticDomain, DiagnosticEvent, DiagnosticFields, DiagnosticLevel,
    DiagnosticOrigin, DiagnosticRecordDto, DiagnosticSubscriptionDto, FrontendDiagnosticEntryDto,
};
pub use runtime::{DiagnosticSubmissionError, DiagnosticsInitializationError, DiagnosticsRuntime};

fn local_timestamp_now() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.3f")
        .to_string()
}

#[cfg(test)]
mod tests;
