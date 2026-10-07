//! Alignment, admission and graph-independent adapters for observed path models.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::execution::ScientificExecutionControl as Control;
mod mediation;
mod moderation;
mod recursive;
mod reports;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    moderation::register(builder);
    mediation::register(builder);
    recursive::register(builder);
}
