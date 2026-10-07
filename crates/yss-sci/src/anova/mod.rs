//! ANOVA owns design construction, nested-model tests and repeated contrasts.
mod design;
mod multivariate;
mod repeated;
#[cfg(test)]
mod tests;

pub use multivariate::manova;
pub use repeated::repeated_measures;

use design::{Design, fit, response_matrix, term_indices};
use statrs::distribution::{ContinuousCDF, FisherSnedecor};
use yss_sci_contract::{anova::*, execution::*};

type Result<T> = std::result::Result<T, ScientificComputationError>;

fn invalid(violation: ScientificInputViolation) -> ScientificComputationError {
    ScientificComputationError::InvalidInput { violation }
}
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ScientificComputationError::ComputationFailed)
    }
}
fn nonnegative(value: f64, scale: f64) -> Result<f64> {
    if value < -1e-10 * scale.abs().max(f64::MIN_POSITIVE) {
        return Err(ScientificComputationError::ComputationFailed);
    }
    finite(value.max(0.0))
}
fn f_probability(statistic: f64, df1: f64, df2: f64) -> Result<f64> {
    if !statistic.is_finite() || statistic < 0.0 || df1 <= 0.0 || df2 <= 0.0 {
        return Err(ScientificComputationError::ComputationFailed);
    }
    finite(
        FisherSnedecor::new(df1, df2)
            .map_err(|_| ScientificComputationError::ComputationFailed)?
            .sf(statistic),
    )
}
fn factor_levels(factors: &[Factor]) -> Vec<FactorLevels> {
    factors
        .iter()
        .enumerate()
        .map(|(i, factor)| FactorLevels {
            name: format!("factor{}", i + 1),
            levels: (0..factor.levels).collect(),
        })
        .collect()
}

/// Covariates use parallel slopes and are centered at their observed means.
pub fn anova(
    response: &[f64],
    factors: &[Factor],
    covariates: &[Vec<f64>],
    options: AnovaOptions,
    control: &ScientificExecutionControl,
) -> Result<AnovaResult> {
    control.check()?;
    let design = Design::new(response.len(), factors, covariates, options.model, control)?;
    let (y, scales) = response_matrix(&[response], control)?;
    let full = fit(
        &design,
        &y,
        &(0..design.columns.len()).collect::<Vec<_>>(),
        control,
    )?;
    let error_df = response.len() - design.columns.len();
    let error_scaled = full[(0, 0)];
    if error_scaled <= 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let rescale = |value: f64| finite((value * scales[0]) * scales[0]);
    let error_ss = rescale(error_scaled)?;
    let total_scaled = (0..y.nrows()).map(|i| y[(i, 0)].powi(2)).sum::<f64>();
    let total_ss = rescale(total_scaled)?;
    let mut table = Vec::with_capacity(design.terms.len());
    for (index, term) in design.terms.iter().enumerate() {
        control.check()?;
        let (reduced, augmented) = term_indices(&design, index, options.sums_of_squares);
        let reduced_ss = fit(&design, &y, &reduced, control)?[(0, 0)];
        let augmented_ss = if augmented.len() == design.columns.len() {
            error_scaled
        } else {
            fit(&design, &y, &augmented, control)?[(0, 0)]
        };
        let ss = nonnegative(reduced_ss - augmented_ss, reduced_ss)?;
        let df = term.columns.len();
        let f = finite((ss / df as f64) / (error_scaled / error_df as f64))?;
        let sum_squares = rescale(ss)?;
        table.push(AnovaTerm {
            term: term.name.clone(),
            sum_squares,
            df,
            mean_square: sum_squares / df as f64,
            f_statistic: f,
            p_value: f_probability(f, df as f64, error_df as f64)?,
            partial_eta_squared: ss / (ss + error_scaled),
        });
    }
    control.check()?;
    Ok(AnovaResult {
        method: if covariates.is_empty() {
            "anova"
        } else {
            "ancova"
        }
        .into(),
        observations: response.len(),
        factors: factor_levels(factors),
        covariate_means: design.covariate_means,
        options,
        table,
        error: AnovaError {
            sum_squares: error_ss,
            df: error_df,
            mean_square: error_ss / error_df as f64,
        },
        total_sum_squares: total_ss,
        total_df: response.len() - 1,
        r_squared: nonnegative(1.0 - error_scaled / total_scaled, 1.0)?,
    })
}
