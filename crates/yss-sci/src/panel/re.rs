//! Panel Random Effects estimators split by workflow.

use super::data::within_transform_by_group as within_transform;
use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::linear::OLS;
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use std::collections::HashMap;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

mod shared;
use shared::*;
mod be;
pub use be::*;
mod fgls;
pub use fgls::*;
mod mle;
pub use mle::*;
mod twoway;
pub use twoway::*;
mod time;
pub use time::*;
