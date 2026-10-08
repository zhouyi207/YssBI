//! One-way OLS CR1 covariance with cluster-count t inference.
use crate::regression::{
    linear::fit::fit_ols,
    models::common::{Result, coefficient_table, failed, invalid, names, validate},
};
use std::collections::BTreeSet;
use yss_sci_contract::{
    SciError,
    execution::{ScientificExecutionControl as Control, ScientificInputViolation as Violation},
    inference::ClusterInference,
    regression::{OlsCovariance, OlsOptions, fit::RegressionStatistics},
};
use yss_sci_linalg::Mat;

pub fn fit(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    groups: Vec<usize>,
    constant: bool,
    control: &Control,
) -> Result<ClusterInference> {
    control.check()?;
    let n = response.len();
    let k = predictors.len() + usize::from(constant);
    if groups.len() != n {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if n <= k {
        return Err(invalid(Violation::EmptyInput));
    }
    validate(&response, &predictors, control)?;
    let g = {
        let mut distinct = BTreeSet::new();
        for (index, &group) in groups.iter().enumerate() {
            if index % 1024 == 0 {
                control.check()?;
            }
            distinct.insert(group);
        }
        distinct.len()
    };
    if g < 2 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let fit = fit_ols(
        response,
        &predictors,
        OlsOptions {
            constant,
            covariance: OlsCovariance::Cluster {
                cluster_id: groups,
                xtreg_fe_style: false,
            },
        },
        yss_sci_contract::StatisticalObservationMetadata {
            original_observation_count: n,
            used_observation_count: n,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
        },
    )
    .map_err(SciError::into_computation_error)?;
    control.check()?;
    let covariance = fit.statistics.coefficient_statistics().covariance.clone();
    let cov = Mat::from_fn(k, k, |j, l| covariance[j][l]);
    let coefficients = coefficient_table(
        &fit.coefficients,
        names(k - usize::from(constant), constant),
        Some(&cov),
        Some(g - 1),
    )?;
    Ok(ClusterInference {
        observations: n,
        clusters: g,
        parameters: k,
        degrees_of_freedom: g - 1,
        covariance_correction: g as f64 / (g - 1) as f64 * (n - 1) as f64 / (n - k) as f64,
        r_squared: match &fit.statistics {
            RegressionStatistics::Linear { model, .. } => model.r2,
            _ => return Err(failed()),
        },
        coefficients,
        covariance,
    })
}
