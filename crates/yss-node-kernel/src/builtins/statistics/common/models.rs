use super::*;
use yss_sci_contract::regression::fit::{FittedRegression, RegressionFit};
pub(in crate::builtins::statistics) fn model_dimensions(
    v: &RuntimeValue,
) -> Result<(usize, usize, bool), KernelError> {
    if let RuntimeValue::LinearRegression(m) = v {
        return Ok((m.fitted.len(), m.design.len(), false));
    }
    let RuntimeValue::List(residuals) = field(v, "residuals")? else {
        return Err(KernelError::InvalidNumericInput);
    };
    let RuntimeValue::List(design) = field(v, "design")? else {
        return Err(KernelError::InvalidNumericInput);
    };
    Ok((residuals.len(), design.len(), true))
}
pub(in crate::builtins::statistics) fn with_model<T>(
    value: &RuntimeValue,
    inv: &KernelInvocation<'_>,
    f: impl FnOnce(FittedRegression<'_>) -> Result<T, KernelError>,
) -> Result<T, KernelError> {
    match value {
        RuntimeValue::LinearRegression(model) => f(FittedRegression::Linear(model)),
        RuntimeValue::Record(_) => {
            let model: RegressionFit = decode_model_value(value, inv)?;
            f(FittedRegression::Binary(&model))
        }
        _ => Err(KernelError::InvalidNumericInput),
    }
}

use yss_sci_contract::regression::models::{
    ModelStatistics, RegressionCoefficient, RegressionModelResult,
};

pub(in crate::builtins::statistics) fn regression_outputs<T: serde::Serialize>(
    inv: &KernelInvocation<'_>,
    response: &[f64],
    model: &RegressionModelResult,
    factor_names: Vec<String>,
    details: &T,
) -> Result<Vec<RuntimeValue>, KernelError> {
    #[derive(serde::Serialize)]
    struct Report<'a, T> {
        method: &'a str,
        observations: usize,
        coefficients: &'a [RegressionCoefficient],
        covariance: &'a Option<Vec<Vec<f64>>>,
        statistics: &'a ModelStatistics,
        iterations: usize,
        converged: bool,
        factor_names: Vec<String>,
        details: &'a T,
    }
    let report = value(
        Report {
            method: &model.method,
            observations: model.observations,
            coefficients: &model.coefficients,
            covariance: &model.covariance,
            statistics: &model.statistics,
            iterations: model.iterations,
            converged: model.converged,
            factor_names,
            details,
        },
        inv,
    )?;
    let mut rows = inv.control.reserve(response.len())?;
    for (i, &actual) in response.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        rows.push([(i + 1) as f64, actual, model.fitted[i], model.residuals[i]]);
    }
    Ok(vec![
        report,
        numeric_table(
            &rows,
            1,
            ["observation", "response", "fitted", "residual"],
            |row| *row,
            inv,
        )?,
    ])
}
