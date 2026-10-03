use super::evaluation::{EvaluationDesign, binary_link, delta_standard_error};
use crate::{
    inference::intervals::critical,
    regression::models::common::{Result, finite, parameter},
};
use statrs::distribution::Normal;
use yss_sci_contract::{
    execution::ScientificExecutionControl as Control,
    regression::{
        fit::{FittedRegression, RegressionStatistics},
        postestimation::*,
    },
};

pub fn adjusted_predictions(
    model: FittedRegression<'_>,
    options: PredictionOptions,
    control: &Control,
) -> Result<AdjustedPrediction> {
    control.check()?;
    let (beta, design, names, constant, covariance, df, link) = match model {
        FittedRegression::Linear(m) => {
            let info = &m.report.model_basic_info;
            let df = match info.covariance_type.as_str() {
                "nonrobust" => Some(info.df_residual as f64),
                // Cluster group count is not retained in the neutral linear fit.
                // Robust adjusted means use asymptotic normal inference.
                _ => None,
            };
            (
                &m.coefficients,
                &m.design,
                m.report
                    .coefficients
                    .iter()
                    .map(|c| c.variable.clone())
                    .collect::<Vec<_>>(),
                m.constant,
                &m.report.cov_beta,
                df,
                None,
            )
        }
        FittedRegression::Binary(m) => {
            let RegressionStatistics::Binary { link, model, .. } = &m.statistics else {
                return Err(parameter());
            };
            if !model.converged {
                return Err(parameter());
            }
            (
                &m.coefficients,
                &m.design,
                m.parameter_names.clone(),
                m.constant,
                &m.statistics.coefficient_statistics().covariance,
                None,
                Some(*link),
            )
        }
    };
    let k = beta.len();
    if k == 0
        || design.len() != k
        || beta.iter().any(|v| !v.is_finite())
        || covariance.len() != k
        || covariance
            .iter()
            .any(|r| r.len() != k || r.iter().any(|v| !v.is_finite()))
    {
        return Err(parameter());
    }
    let grid = EvaluationDesign::new(
        design,
        &names,
        constant,
        options.evaluation,
        &options.at,
        control,
    )?;
    let q = critical(options.confidence_level, df)?;
    let normal = Normal::new(0., 1.).map_err(|_| parameter())?;
    let mut estimate = 0.;
    let mut gradient = vec![0.; k];
    for i in 0..grid.rows {
        control.check()?;
        let eta = finite((0..k).map(|j| grid.value(i, j) * beta[j]).sum())?;
        let (prediction, derivative) = link.map_or((eta, 1.), |link| {
            let (p, d, _, _) = binary_link(link, eta, &normal);
            (p, d)
        });
        estimate += prediction / grid.rows as f64;
        for (j, g) in gradient.iter_mut().enumerate() {
            *g += derivative * grid.value(i, j) / grid.rows as f64;
        }
    }
    let standard_error = delta_standard_error(&gradient, covariance)?;
    Ok(AdjustedPrediction {
        observations: design[0].len(),
        family: match link {
            None => "linear",
            Some(yss_sci_contract::regression::fit::BinaryRegressionLink::Logit) => "logit",
            Some(yss_sci_contract::regression::fit::BinaryRegressionLink::Probit) => "probit",
        },
        evaluation: options.evaluation,
        at: options.at,
        confidence_level: options.confidence_level,
        degrees_of_freedom: df,
        estimate: finite(estimate)?,
        standard_error,
        lower: finite(estimate - q * standard_error)?,
        upper: finite(estimate + q * standard_error)?,
    })
}
