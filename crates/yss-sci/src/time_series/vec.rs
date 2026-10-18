//! VEC implementation split by workflow.

use super::distributions::{chi_squared_sf, normal_two_sided_p};
use super::var::cholesky_lower_in_place;
use super::vec_vecrank_cv::{max_eigen_critical_row, trace_critical_row};
use crate::regression::design::covariance_rows;
use serde::{Deserialize, Serialize};
use yss_sci_contract::regression::fit::RegressionCoefficientStatistics;
use yss_sci_contract::time_series::fit::{
    EquationStatistics, MultivariateStatistics, SerialCorrelationTest, StabilityRoot,
};
use yss_sci_contract::time_series::forecast::Deterministic;
pub use yss_sci_contract::time_series::vec::VecFit;
use yss_sci_contract::time_series::vec::{
    CointegratingEquationStatistics, CointegrationStatistics,
};
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

mod types;
pub use types::*;
mod stage;
use stage::*;
mod estimate;
pub use estimate::*;
mod vecrank;
pub use vecrank::*;
mod stats;
use stats::*;
mod postestimation;
pub use postestimation::*;
