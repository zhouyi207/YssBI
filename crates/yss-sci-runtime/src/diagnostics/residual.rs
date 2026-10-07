use yss_sci_contract::diagnostics::residual::{
    NormalityTestResult, ResidualDiagnostic, ResidualDiagnosticResult,
};
use yss_sci_contract::regression::linear::LinearRegressionResult;

pub fn diagnose(
    model: &LinearRegressionResult,
    test: ResidualDiagnostic,
) -> Result<ResidualDiagnosticResult, String> {
    yss_sci::diagnostics::residual::diagnose(model, test)
}
pub fn normality(values: &[f64]) -> Result<NormalityTestResult, String> {
    yss_sci::diagnostics::residual::normality(values)
}
