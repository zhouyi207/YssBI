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

fn rfc3339_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests;
