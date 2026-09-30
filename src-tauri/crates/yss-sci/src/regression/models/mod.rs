//! Regression estimators, sharing controlled numerical preparation and structured results.
mod common;
mod likelihood;
mod nonlinear;
mod regularized;
mod robust;
mod workflows;

pub use likelihood::{firth_logit, glm, likelihood};
pub use nonlinear::{
    automatic_spline_knots, curve, deming, nonlinear, nonlinear_formula, restricted_cubic_spline,
};
pub use regularized::{penalized, pls};
pub use robust::{quantile, robust};
pub use workflows::{
    baseline, grouped, hierarchical, stepwise, threshold, univariate_multivariable,
};

#[cfg(test)]
mod tests;
