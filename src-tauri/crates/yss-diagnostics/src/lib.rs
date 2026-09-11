mod dispatcher;
mod dto;
mod runtime;
mod rust_projection;
mod validation;
mod worker;

pub use dispatcher::DiagnosticsUnavailable;
pub use dto::{
    DiagnosticBatchDto, DiagnosticDomain, DiagnosticFields, DiagnosticLevel, DiagnosticOrigin,
    DiagnosticRecordDto, DiagnosticSubscriptionDto, FrontendDiagnosticEntryDto,
};
pub use runtime::{
    DiagnosticsInitializationError, DiagnosticsRuntime, SubmitFrontendDiagnosticsError,
};

fn local_timestamp_now() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.3f")
        .to_string()
}

#[cfg(test)]
mod tests;
