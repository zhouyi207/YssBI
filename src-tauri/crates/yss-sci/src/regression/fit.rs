//! Dispatch among regression families using their numerical fit entry points.
use super::design::design_matrix;
use super::linear::fit::{fit_linear_regression, fit_ols_design};
use crate::error::invalid_input;
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::regression::fit::{RegressionFit, RegressionKind};
use yss_sci_contract::regression::linear::LinearRegressionMethod;
use yss_sci_contract::{
    SciError, SciInputViolation, SciOperationCode, StatisticalObservationMetadata,
};
use yss_sci_linalg::Col;

pub fn fit_regression(
    kind: RegressionKind,
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    weights: Option<Vec<f64>>,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    let y = Col::from_iter(response);
    let x = design_matrix(&predictors, y.nrows(), true, SciOperationCode::Regression)?;
    match kind {
        RegressionKind::Ols => fit_ols_design(&y, &x, &OlsOptions::default(), metadata),
        RegressionKind::Gls | RegressionKind::Wls => {
            let method = if matches!(kind, RegressionKind::Gls) {
                LinearRegressionMethod::Gls {
                    sigma: (0..y.nrows())
                        .map(|i| (0..y.nrows()).map(|j| f64::from(i == j)).collect())
                        .collect(),
                }
            } else {
                LinearRegressionMethod::Wls {
                    weights: weights.ok_or_else(|| {
                        invalid_input(SciOperationCode::Regression, SciInputViolation::EmptyInput)
                    })?,
                }
            };
            fit_linear_regression(
                y.iter().copied().collect(),
                &predictors,
                OlsOptions::default(),
                method,
                metadata,
            )
        }
        RegressionKind::Prais => {
            super::linear::fit::fit_prais_design(y, x, metadata, Default::default())
        }
        RegressionKind::Logit => {
            super::discrete::fit::fit_logit_design(y, x, metadata, Default::default())
        }
        RegressionKind::Probit => {
            super::discrete::fit::fit_probit_design(y, x, metadata, Default::default())
        }
    }
}
