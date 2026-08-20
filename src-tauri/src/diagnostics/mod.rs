pub(crate) mod dispatcher;
mod dto;
mod limits;
pub(crate) mod recent_layer;
mod runtime;
mod sanitizer;
mod validation;
mod worker;

pub use dto::{
    DiagnosticBatchDto, DiagnosticDomain, DiagnosticFields, DiagnosticLevel, DiagnosticOrigin,
    DiagnosticRecordDto, DiagnosticSubscriptionDto, FrontendDiagnosticEntryDto,
};
pub use runtime::DiagnosticsRuntime;

fn rfc3339_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests;
