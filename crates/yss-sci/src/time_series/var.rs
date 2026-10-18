//! VAR implementation split by workflow.

use super::distributions::{chi_squared_sf, normal_two_sided_p};
use crate::regression::design::covariance_rows;
use serde::{Deserialize, Serialize};
use yss_sci_contract::regression::fit::RegressionCoefficientStatistics;
use yss_sci_contract::time_series::fit::{
    EquationStatistics, MultivariateStatistics, SerialCorrelationTest, StabilityRoot,
};
pub use yss_sci_contract::time_series::var::VarFit;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

mod types;
pub use types::*;
mod varsoc;
pub use varsoc::*;
mod estimate;
mod postestimation;
pub use postestimation::*;
