//! Experiment adapters own factor labels, alignment and relation projection.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::execution::ScientificExecutionControl as Control;
mod design;
mod models;
mod range;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    range::register(builder);
    design::register(builder);
    models::register(builder);
}
