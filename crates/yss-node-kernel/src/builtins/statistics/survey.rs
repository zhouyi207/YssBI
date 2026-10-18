//! Survey node alignment and resource admission; design calculations belong to SCI.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, survey::*};
mod estimates;
mod weights;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    weights::register(builder);
    estimates::register(builder);
}
