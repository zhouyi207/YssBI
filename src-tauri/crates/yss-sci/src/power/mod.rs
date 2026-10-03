//! Prospective power and minimum-integer sample size under explicitly specified designs.
use crate::regression::models::common::{Result, failed, finite, parameter};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, power::*};
mod design;
mod distributions;
mod evaluate;
mod solve;
pub use solve::compute;
