//! Backend-neutral hypothesis computation entry points.
pub use yss_sci::hypothesis::categorical::run as categorical_test;
pub use yss_sci::hypothesis::linear_hypothesis::{
    parse_at_values, run_asymptotic_hypothesis_test, run_hypothesis_test,
};
pub use yss_sci::hypothesis::nonparametric::run as rank_test;
pub use yss_sci::hypothesis::sample_mean::run as sample_mean_test;
pub use yss_sci::hypothesis::variance::run as variance_test;
