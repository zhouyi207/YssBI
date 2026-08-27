use std::time::Instant;

use crate::execution::ports::scientific::{
    AcfPacfRequest, AcfPacfResult, BackendExecutionControl, ExecutionInstrumentalVariableKind,
    ExecutionRegressionKind, ExecutionStatisticalTrend, KdePoint, KernelDensityRequest,
    KernelDensityResult, ScientificBackend, ScientificBackendError, ScientificInputViolation,
    StatisticsOperation, StatisticsParameters, StatisticsRequest, StatisticsResult,
};
use crate::execution::settings::{ExecutionMissingValuePolicy, ExecutionSettings};
use crate::sci::api::computation::{
    MissingValuePolicy, NumericTolerance, SciComputationSettings, StatisticalObservationMetadata,
    StatisticalSettingSource,
};
use crate::sci::api::control::{AbsoluteDeadline, ExecutionControl, SciCancellationSource};
use crate::sci::api::density::{
    KernelDensityInput, compute_kernel_density as compute_kernel_density_api,
};
use crate::sci::api::node_statistics::{
    InstrumentalVariableKind, RegressionKind, augmented_dickey_fuller, fit_instrumental_variables,
    fit_panel, fit_regression, var_fit, var_lag_order, vec_fit, vec_rank_test,
};
use crate::sci::api::time_series::acf_pacf::{
    AcfPacfInput, compute_acf_pacf as compute_acf_pacf_api,
};
use crate::sci::error::{SciError, SciInputViolation, SciOperationCode};

pub struct SciApiScientificBackend;

impl SciApiScientificBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl ScientificBackend for SciApiScientificBackend {
    fn statistics(
        &self,
        request: StatisticsRequest,
        control: &BackendExecutionControl,
    ) -> Result<StatisticsResult, ScientificBackendError> {
        preflight(control)?;
        let settings = map_settings(request.settings)?;
        validate_statistics_request(&request.operation, &request.parameters, &request.inputs)?;
        let metadata = observation_metadata(&request.inputs, settings);
        let value = execute_statistics(
            request.operation,
            request.parameters,
            request.inputs,
            metadata,
        )?;
        Ok(StatisticsResult { value })
    }

    fn kernel_density(
        &self,
        request: KernelDensityRequest,
        control: &BackendExecutionControl,
    ) -> Result<KernelDensityResult, ScientificBackendError> {
        preflight(control)?;
        validate_density_request(&request)?;
        let output = compute_kernel_density_api(KernelDensityInput {
            values: &request.values,
            grid_points: request.grid_points,
            min_x: request.min_x,
        });
        Ok(KernelDensityResult {
            points: output
                .points
                .into_iter()
                .map(|point| KdePoint {
                    x: point.x,
                    density: point.density,
                })
                .collect(),
        })
    }

    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError> {
        preflight(control)?;
        validate_acf_pacf_request(&request)?;
        let output = compute_acf_pacf_api(AcfPacfInput {
            residuals: request.values,
            max_lag: request.max_lag,
        })
        .map_err(map_sci_error)?;
        Ok(AcfPacfResult {
            acf: output.acf,
            pacf: output.pacf,
            n: output.n,
        })
    }
}

fn preflight(control: &BackendExecutionControl) -> Result<(), ScientificBackendError> {
    let control = map_control(control);
    if control.is_cancelled() {
        return Err(ScientificBackendError::Cancelled);
    }
    if control.is_expired(Instant::now()) {
        return Err(ScientificBackendError::DeadlineExceeded);
    }
    Ok(())
}

fn map_control(control: &BackendExecutionControl) -> ExecutionControl {
    let (source, cancellation) = SciCancellationSource::new();
    if control.cancellation.is_cancelled() {
        source.cancel();
    }
    ExecutionControl::new(cancellation, AbsoluteDeadline::at(control.deadline))
}

fn map_settings(
    settings: ExecutionSettings,
) -> Result<SciComputationSettings, ScientificBackendError> {
    let tolerance = settings.numeric_tolerance;
    if !tolerance.absolute.is_finite()
        || tolerance.absolute < 0.0
        || !tolerance.relative.is_finite()
        || tolerance.relative < 0.0
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let missing_values = match settings.statistical_missing_values {
        ExecutionMissingValuePolicy::Listwise => MissingValuePolicy::Listwise,
        ExecutionMissingValuePolicy::Reject => MissingValuePolicy::Reject,
    };
    Ok(SciComputationSettings {
        tolerance: NumericTolerance {
            absolute: tolerance.absolute,
            relative: tolerance.relative,
        },
        missing_values,
    })
}

fn observation_metadata(
    inputs: &[Vec<f64>],
    settings: SciComputationSettings,
) -> StatisticalObservationMetadata {
    let observations = inputs.first().map_or(0, Vec::len);
    StatisticalObservationMetadata {
        original_observation_count: observations,
        used_observation_count: observations,
        dropped_null_count: 0,
        dropped_nan_count: 0,
        missing_value_policy: settings.missing_values,
        missing_value_policy_source: StatisticalSettingSource::ProjectDefault,
        effective_convergence_tolerance: settings.tolerance.absolute,
        convergence_tolerance_source: StatisticalSettingSource::ProjectDefault,
        convergence_tolerance_consumed: false,
    }
}

fn validate_statistics_request(
    operation: &StatisticsOperation,
    parameters: &StatisticsParameters,
    inputs: &[Vec<f64>],
) -> Result<(), ScientificBackendError> {
    validate_series(inputs)?;
    let expected = match operation {
        StatisticsOperation::Regression { kind } => {
            if *kind == ExecutionRegressionKind::Wls {
                let weights = parameters
                    .weights
                    .as_deref()
                    .ok_or_else(|| invalid(ScientificInputViolation::EmptyInput))?;
                let observations = inputs
                    .first()
                    .map(Vec::len)
                    .ok_or_else(|| invalid(ScientificInputViolation::EmptyInput))?;
                validate_auxiliary_series(weights, observations)?;
            } else if parameters.weights.is_some() {
                return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
            }
            2..=usize::MAX
        }
        StatisticsOperation::InstrumentalVariables { .. } => 4..=4,
        StatisticsOperation::Panel => 4..=usize::MAX,
        StatisticsOperation::PanelDidTwfe => 5..=usize::MAX,
        StatisticsOperation::AugmentedDickeyFuller => {
            validate_positive(parameters.lags)?;
            1..=1
        }
        StatisticsOperation::VarFit => {
            validate_positive(parameters.lags)?;
            2..=usize::MAX
        }
        StatisticsOperation::VarLagOrder => {
            validate_positive(parameters.max_lags)?;
            2..=usize::MAX
        }
        StatisticsOperation::VecFit => {
            validate_positive(parameters.lags)?;
            validate_positive(parameters.rank)?;
            2..=usize::MAX
        }
        StatisticsOperation::VecRankTest => {
            validate_positive(parameters.max_lags)?;
            2..=usize::MAX
        }
    };
    if expected.contains(&inputs.len()) {
        Ok(())
    } else {
        Err(invalid(ScientificInputViolation::ShapeMismatch))
    }
}

fn validate_series(inputs: &[Vec<f64>]) -> Result<(), ScientificBackendError> {
    let Some(observations) = inputs.first().map(Vec::len) else {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    };
    if observations == 0 || inputs.iter().any(Vec::is_empty) {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if inputs.iter().any(|series| series.len() != observations) {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    if inputs.iter().flatten().any(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
    }
    Ok(())
}

fn validate_auxiliary_series(
    values: &[f64],
    observations: usize,
) -> Result<(), ScientificBackendError> {
    if values.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if values.len() != observations {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
    }
    Ok(())
}

fn validate_positive(value: usize) -> Result<(), ScientificBackendError> {
    if value == 0 {
        Err(invalid(ScientificInputViolation::ParameterOutOfRange))
    } else {
        Ok(())
    }
}

fn execute_statistics(
    operation: StatisticsOperation,
    parameters: StatisticsParameters,
    inputs: Vec<Vec<f64>>,
    metadata: StatisticalObservationMetadata,
) -> Result<serde_json::Value, ScientificBackendError> {
    match operation {
        StatisticsOperation::Regression { kind } => {
            let mut inputs = inputs.into_iter();
            let response = next_input(&mut inputs)?;
            let result = fit_regression(
                map_regression_kind(kind),
                response,
                inputs.collect(),
                parameters.weights,
                metadata,
            )
            .map_err(map_sci_error)?;
            serde_json::to_value(result).map_err(|_| ScientificBackendError::ComputationFailed)
        }
        StatisticsOperation::InstrumentalVariables { kind } => {
            let mut inputs = inputs.into_iter();
            fit_instrumental_variables(
                map_instrumental_variable_kind(kind),
                next_input(&mut inputs)?,
                next_input(&mut inputs)?,
                next_input(&mut inputs)?,
                next_input(&mut inputs)?,
            )
            .map_err(map_sci_error)
        }
        StatisticsOperation::Panel => execute_panel(inputs, false),
        StatisticsOperation::PanelDidTwfe => execute_panel(inputs, true),
        StatisticsOperation::AugmentedDickeyFuller => {
            let series = inputs
                .first()
                .ok_or_else(|| invalid(ScientificInputViolation::ShapeMismatch))?;
            augmented_dickey_fuller(series, parameters.lags, trend_name(parameters.trend))
                .map_err(map_sci_error)
        }
        StatisticsOperation::VarFit => var_fit(inputs, parameters.lags).map_err(map_sci_error),
        StatisticsOperation::VarLagOrder => {
            var_lag_order(inputs, parameters.max_lags).map_err(map_sci_error)
        }
        StatisticsOperation::VecFit => vec_fit(
            inputs,
            parameters.rank,
            parameters.lags,
            trend_name(parameters.trend),
        )
        .map_err(map_sci_error),
        StatisticsOperation::VecRankTest => {
            vec_rank_test(inputs, parameters.max_lags, trend_name(parameters.trend))
                .map_err(map_sci_error)
        }
    }
}

fn next_input(
    inputs: &mut impl Iterator<Item = Vec<f64>>,
) -> Result<Vec<f64>, ScientificBackendError> {
    inputs
        .next()
        .ok_or_else(|| invalid(ScientificInputViolation::ShapeMismatch))
}

fn execute_panel(
    mut inputs: Vec<Vec<f64>>,
    includes_treatment: bool,
) -> Result<serde_json::Value, ScientificBackendError> {
    let treatment = if includes_treatment {
        Some(
            inputs
                .pop()
                .ok_or_else(|| invalid(ScientificInputViolation::ShapeMismatch))?,
        )
    } else {
        None
    };
    let time = inputs
        .pop()
        .ok_or_else(|| invalid(ScientificInputViolation::ShapeMismatch))?;
    let entity = inputs
        .pop()
        .ok_or_else(|| invalid(ScientificInputViolation::ShapeMismatch))?;
    let mut inputs = inputs.into_iter();
    let response = next_input(&mut inputs)?;
    fit_panel(response, inputs.collect(), entity, time, treatment).map_err(map_sci_error)
}

fn map_regression_kind(kind: ExecutionRegressionKind) -> RegressionKind {
    match kind {
        ExecutionRegressionKind::Ols => RegressionKind::Ols,
        ExecutionRegressionKind::Gls => RegressionKind::Gls,
        ExecutionRegressionKind::Logit => RegressionKind::Logit,
        ExecutionRegressionKind::Probit => RegressionKind::Probit,
        ExecutionRegressionKind::Prais => RegressionKind::Prais,
        ExecutionRegressionKind::Wls => RegressionKind::Wls,
    }
}

fn map_instrumental_variable_kind(
    kind: ExecutionInstrumentalVariableKind,
) -> InstrumentalVariableKind {
    match kind {
        ExecutionInstrumentalVariableKind::TwoStageLeastSquares => {
            InstrumentalVariableKind::TwoStageLeastSquares
        }
        ExecutionInstrumentalVariableKind::LimitedInformationMaximumLikelihood => {
            InstrumentalVariableKind::LimitedInformationMaximumLikelihood
        }
    }
}

fn trend_name(trend: ExecutionStatisticalTrend) -> &'static str {
    match trend {
        ExecutionStatisticalTrend::None => "none",
        ExecutionStatisticalTrend::Constant => "constant",
        ExecutionStatisticalTrend::Trend => "trend",
    }
}

fn validate_density_request(request: &KernelDensityRequest) -> Result<(), ScientificBackendError> {
    if request.values.is_empty() {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if request.values.iter().any(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
    }
    if request.grid_points < 2 || request.min_x.is_some_and(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    Ok(())
}

fn validate_acf_pacf_request(request: &AcfPacfRequest) -> Result<(), ScientificBackendError> {
    if request.values.len() < 4 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    if request.values.iter().any(|value| !value.is_finite()) {
        return Err(invalid(ScientificInputViolation::NonFiniteInput));
    }
    Ok(())
}

fn invalid(violation: ScientificInputViolation) -> ScientificBackendError {
    ScientificBackendError::InvalidInput { violation }
}

fn map_sci_error(error: SciError) -> ScientificBackendError {
    match error {
        SciError::InvalidInput {
            operation,
            violation,
        } => {
            exhaust_sci_operation(operation);
            invalid(match violation {
                SciInputViolation::EmptyInput => ScientificInputViolation::EmptyInput,
                SciInputViolation::NonFiniteInput => ScientificInputViolation::NonFiniteInput,
                SciInputViolation::ShapeMismatch => ScientificInputViolation::ShapeMismatch,
                SciInputViolation::ParameterOutOfRange => {
                    ScientificInputViolation::ParameterOutOfRange
                }
            })
        }
        SciError::ComputationFailed { operation } => {
            exhaust_sci_operation(operation);
            ScientificBackendError::ComputationFailed
        }
    }
}

fn exhaust_sci_operation(operation: SciOperationCode) {
    match operation {
        SciOperationCode::Regression
        | SciOperationCode::InstrumentalVariables
        | SciOperationCode::Panel
        | SciOperationCode::Adf
        | SciOperationCode::VarFit
        | SciOperationCode::VarLagOrder
        | SciOperationCode::VecFit
        | SciOperationCode::VecRank
        | SciOperationCode::KernelDensity
        | SciOperationCode::AcfPacf
        | SciOperationCode::SerialTests
        | SciOperationCode::TTest
        | SciOperationCode::WaldTest => {}
    }
}

#[cfg(test)]
mod tests;
