use yss_sci_contract::regression::{fit::RegressionFit, prais::PraisConfig};
use yss_sci_contract::{SciError, StatisticalObservationMetadata};

pub fn fit_prais(
    response: Vec<f64>,
    predictors: &[Vec<f64>],
    options: PraisConfig,
    metadata: StatisticalObservationMetadata,
) -> Result<RegressionFit, SciError> {
    yss_sci::regression::linear::fit::fit_prais(response, predictors, options, metadata)
}
