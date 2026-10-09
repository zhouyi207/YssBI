pub mod categorical;
pub mod linear_hypothesis;
mod linear_test;
pub mod nonparametric;
pub mod sample_mean;
mod t_test;
pub mod variance;
mod wald_test;

use t_test::t_test;
use wald_test::wald_test;

pub(super) use crate::regression::models::common::{failed, finite, invalid, parameter};
use yss_sci_contract::execution::ScientificExecutionControl;
pub(super) use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificInputViolation as Violation,
};

pub(super) fn checkpoint(control: &ScientificExecutionControl, index: usize) -> Result<(), Error> {
    if index.is_multiple_of(1024) {
        control.check()?;
    }
    Ok(())
}
