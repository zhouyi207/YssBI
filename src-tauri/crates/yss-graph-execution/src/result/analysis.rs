//! Scientific analyses derived from an Execution-owned linear regression result.

use std::time::{Duration, Instant};

use yss_sci_contract::SciError;
use yss_sci_contract::hypothesis::{HypothesisError, HypothesisTestInput, HypothesisTestOutput};
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, LinearRegressionResult, ScientificCancellationToken,
    ScientificComputationError, ScientificExecutionControl,
};
use yss_sci_contract::serial_tests::{SerialTestsInput, SerialTestsOutput};

pub fn acf_pacf(
    result: &LinearRegressionResult,
    max_lag: usize,
) -> Result<AcfPacfResult, ScientificComputationError> {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    };
    yss_sci_runtime::acf_pacf(
        AcfPacfRequest {
            values: result.residuals.clone(),
            max_lag,
        },
        &control,
    )
}

pub fn serial_tests(
    result: &LinearRegressionResult,
    lags: usize,
    bg_nomiss0: bool,
) -> Result<SerialTestsOutput, SciError> {
    yss_sci_runtime::time_series::serial_tests::compute_serial_tests(SerialTestsInput {
        residuals: result.residuals.clone(),
        exog: Some(
            (0..result.residuals.len())
                .map(|row| result.design.iter().map(|column| column[row]).collect())
                .collect(),
        ),
        lags,
        bg_nomiss0,
    })
}

pub fn hypothesis(
    result: &LinearRegressionResult,
    hypothesis: String,
) -> Result<HypothesisTestOutput, HypothesisError> {
    yss_sci_runtime::hypothesis::run_hypothesis_test(HypothesisTestInput {
        betas: result.coefficients.clone(),
        cov_beta: result.report.cov_beta.clone(),
        df_residual: result.report.model_basic_info.df_residual,
        param_names: result
            .report
            .coefficients
            .iter()
            .map(|coefficient| coefficient.variable.clone())
            .collect(),
        hypothesis,
    })
}
