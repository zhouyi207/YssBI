//! Decision adapters share criterion preparation, not statistical algorithms.
use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{decision::*, execution::ScientificExecutionControl as Control};
mod columns;
mod conjoint;
mod experts;
mod fuzzy;
mod market;
mod matrices;
mod preferences;
mod ranking;
mod systems;
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    ranking::register(builder);
    systems::register(builder);
    preferences::register(builder);
    market::register(builder);
    matrices::register(builder);
    fuzzy::register(builder);
    experts::register(builder);
    conjoint::register(builder);
}
