//! Instrumental-variable design preparation and fit projection.
use super::iv2sls::{IV2SLS, IV2SLSConfig};
use super::ivliml::{IVLIML, IVLIMLConfig};
use crate::error::{computation_failed, invalid_input};
use crate::regression::design::design_matrix;
use yss_sci_contract::causal::iv::{InstrumentalVariableFit, InstrumentalVariableKind};
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
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
        return Err(invalid_input(op, SciInputViolation::ShapeMismatch));
    }
    if response
        .iter()
        .chain(exogenous.iter().flatten())
        .chain(endogenous.iter().flatten())
        .chain(instruments.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(invalid_input(op, SciInputViolation::NonFiniteInput));
    }
    let exog = if exogenous.is_empty() {
        yss_sci_linalg::Mat::zeros(observations, 0)
    } else {
        design_matrix(exogenous, observations, false, op)?
    };
    let endog_reg = design_matrix(endogenous, observations, false, op)?;
    let instrument_columns = instruments;
    let instruments = design_matrix(instrument_columns, observations, false, op)?;
    let constant = options.constant;
    let cov_type = options.covariance.name().to_owned();
    let cov_params = options.covariance.parameters();
    let y = Col::from_iter(response);
    let result = match kind {
        InstrumentalVariableKind::TwoStageLeastSquares => IV2SLS {
            endog: y,
            exog,
            endog_reg,
            instruments,
            config: IV2SLSConfig {
                constant,
                cov_type,
                cov_params,
                small,
            },
            endog_names: None,
            z_var_names: None,
        }
        .fit(),
        InstrumentalVariableKind::LimitedInformationMaximumLikelihood => IVLIML {
            endog: y,
            exog,
            endog_reg,
            instruments,
            config: IVLIMLConfig {
                constant,
                cov_type,
                cov_params,
                small,
            },
            endog_names: None,
            z_var_names: None,
        }
        .fit(),
    }
    .map_err(|_| computation_failed(op))?;
    Ok(InstrumentalVariableFit {
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

fn diagnostic_model(fit: &InstrumentalVariableFit) -> Result<IV2SLS, SciError> {
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
        return Err(invalid_input(op, SciInputViolation::ShapeMismatch));
    }
    Ok(IV2SLS {
        endog: Col::from_iter(fit.fitted.iter().zip(&fit.residuals).map(|(f, r)| f + r)),
        exog: if data.exogenous.is_empty() {
            yss_sci_linalg::Mat::zeros(n, 0)
        } else {
            design_matrix(&data.exogenous, n, false, op)?
        },
        endog_reg: design_matrix(&data.endogenous, n, false, op)?,
        instruments: design_matrix(&data.instruments, n, false, op)?,
        config: IV2SLSConfig {
            constant: fit.options.constant,
            cov_type: fit.options.covariance.name().into(),
            cov_params: fit.options.covariance.parameters(),
            small: fit.small,
        },
        endog_names: None,
        z_var_names: None,
    })
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
    diagnostic_model(fit)?
        .first_stage(fit.family == "iv_liml")
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<yss_sci_contract::causal::iv::OveridTest>, SciError> {
    if fit.family != "iv_2sls" {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    diagnostic_model(fit)?
        .overidentification(&Col::from_iter(fit.coefficients.iter().copied()))
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn liml_overidentification(
    fit: &InstrumentalVariableFit,
) -> Result<Option<yss_sci_contract::causal::iv::LimlOveridTest>, SciError> {
    if fit.family != "iv_liml" {
        return Err(invalid_input(
            SciOperationCode::InstrumentalVariables,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    diagnostic_model(fit)?
        .liml_overidentification(&Col::from_iter(fit.coefficients.iter().copied()))
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
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    diagnostic_model(fit)?
        .endogeneity(&Col::from_iter(fit.coefficients.iter().copied()))
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}
