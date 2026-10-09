//! Instrumental-variable design preparation and fit projection.
use super::IvModel;
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::design_matrix;
use yss_sci_contract::causal::iv::{InstrumentalVariableFit, InstrumentalVariableKind};
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};
use yss_sci_linalg::Col;

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: &[Vec<f64>],
    endogenous: &[Vec<f64>],
    instruments: &[Vec<f64>],
    options: OlsOptions,
    small: bool,
) -> Result<InstrumentalVariableFit, SciError> {
    let op = SciOperationCode::InstrumentalVariables;
    let observations = response.len();
    if observations == 0 || endogenous.is_empty() || instruments.len() < endogenous.len() {
        return Err(invalid_input(op, ScientificInputViolation::ShapeMismatch));
    }
    if response
        .iter()
        .chain(exogenous.iter().flatten())
        .chain(endogenous.iter().flatten())
        .chain(instruments.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(invalid_input(op, ScientificInputViolation::NonFiniteInput));
    }
    let exog = if exogenous.is_empty() {
        yss_sci_linalg::Mat::zeros(observations, 0)
    } else {
        design_matrix(exogenous, observations, false, op)?
    };
    let endog_reg = design_matrix(endogenous, observations, false, op)?;
    let instrument_columns = instruments;
    let instruments = design_matrix(instrument_columns, observations, false, op)?;
    let y = Col::from_iter(response);
    let model = IvModel {
        endog: y,
        exog,
        endog_reg,
        instruments,
        options,
        small,
    };
    let result = match kind {
        InstrumentalVariableKind::TwoStageLeastSquares => model.fit_2sls(),
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood => model.fit_liml(),
    };
    let options = model.options;
    let result = result.map_err(|_| computation_failed(op))?;
    Ok(InstrumentalVariableFit {
        response_name: "response".into(),
        parameter_names: std::iter::once("_cons".into())
            .take(usize::from(options.constant))
            .chain((0..exogenous.len() + endogenous.len()).map(|j| format!("x{}", j + 1)))
            .collect(),
        instrument_names: (0..instrument_columns.len())
            .map(|j| format!("z{}", j + 1))
            .collect(),
        family: match kind {
            InstrumentalVariableKind::TwoStageLeastSquares => "iv_2sls",
            InstrumentalVariableKind::LimitedInformationMaximumLikelihood => "iv_liml",
        }
        .into(),
        options,
        small,
        coefficients: result.betas.iter().copied().collect(),
        fitted: result.fitted.iter().copied().collect(),
        residuals: result.residuals.iter().copied().collect(),
        inference: result.inference,
        statistics: result.statistics,
        design: yss_sci_contract::causal::iv::InstrumentalVariableDesign {
            exogenous: exogenous.to_vec(),
            endogenous: endogenous.to_vec(),
            instruments: instrument_columns.to_vec(),
        },
    })
}

fn validate_diagnostic_fit(fit: &InstrumentalVariableFit) -> Result<(), SciError> {
    let op = SciOperationCode::InstrumentalVariables;
    let n = fit.residuals.len();
    let data = &fit.design;
    let k = data.exogenous.len() + data.endogenous.len() + usize::from(fit.options.constant);
    if n == 0
        || n <= k
        || fit.fitted.len() != n
        || fit.coefficients.len() != k
        || data.endogenous.is_empty()
        || data.instruments.len() < data.endogenous.len()
        || !matches!(fit.family.as_str(), "iv_2sls" | "iv_liml")
        || fit
            .fitted
            .iter()
            .chain(&fit.residuals)
            .chain(&fit.coefficients)
            .any(|v| !v.is_finite())
        || data
            .exogenous
            .iter()
            .chain(&data.endogenous)
            .chain(&data.instruments)
            .any(|v| v.len() != n || v.iter().any(|v| !v.is_finite()))
    {
        return Err(invalid_input(op, ScientificInputViolation::ShapeMismatch));
    }
    Ok(())
}

pub fn first_stage(
    fit: &InstrumentalVariableFit,
) -> Result<
    (
        Vec<yss_sci_contract::causal::iv::FirstStageResult>,
        yss_sci_contract::causal::iv::FirstStageSummary,
    ),
    SciError,
> {
    validate_diagnostic_fit(fit)?;
    let kind = if fit.family == "iv_liml" {
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood
    } else {
        InstrumentalVariableKind::TwoStageLeastSquares
    };
    super::first_stage::analyze(&fit.design, &fit.options, kind)
}

pub fn overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<yss_sci_contract::causal::iv::OveridTest>, SciError> {
    if fit.family != "iv_2sls" {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            ScientificInputViolation::ParameterOutOfRange,
        ));
    }
    validate_diagnostic_fit(fit)?;
    super::postestimation::overidentification(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn liml_overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<yss_sci_contract::causal::iv::LimlOveridTest>, SciError> {
    if fit.family != "iv_liml" {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            ScientificInputViolation::ParameterOutOfRange,
        ));
    }
    validate_diagnostic_fit(fit)?;
    super::postestimation::liml_overidentification(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn endogeneity(
    fit: &InstrumentalVariableFit,
) -> Result<
    (
        Option<yss_sci_contract::causal::iv::HausmanTest>,
        Option<yss_sci_contract::causal::iv::EndogenousTest>,
    ),
    SciError,
> {
    if fit.family != "iv_2sls" {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            ScientificInputViolation::ParameterOutOfRange,
        ));
    }
    validate_diagnostic_fit(fit)?;
    super::postestimation::endogeneity(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}
