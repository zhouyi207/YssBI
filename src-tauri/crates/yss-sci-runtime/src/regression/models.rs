//! Stateless entry points; estimators and their execution checks remain in SCI.
pub use yss_sci::regression::models::{
    automatic_spline_knots, baseline, curve, deming, firth_logit, glm, grouped, hierarchical,
    likelihood, nonlinear, nonlinear_formula, penalized, pls, quantile, restricted_cubic_spline,
    robust, stepwise, threshold, univariate_multivariable,
};
