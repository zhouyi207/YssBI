//! Binary-response model records.

use yss_sci_contract::regression::{
    discrete::BinaryOptions,
    fit::{BinaryRegressionLink, RegressionFit},
};
use yss_sci_contract::{SciError, StatisticalObservationMetadata};

pub fn fit_binary(
    link: BinaryRegressionLink,
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    options: BinaryOptions,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    yss_sci::regression::discrete::fit::fit_binary(link, response, predictors, options, metadata)
}

pub fn predict_binary(
    link: BinaryRegressionLink,
    coefficients: &[f64],
    predictors: &[Vec<f64>],
    constant: bool,
) -> Result<Vec<f64>, SciError> {
    yss_sci::regression::discrete::fit::predict_binary(link, coefficients, predictors, constant)
}

pub use yss_sci::regression::discrete::postestimation::{
    classification, marginal_effects, odds_ratios,
};
