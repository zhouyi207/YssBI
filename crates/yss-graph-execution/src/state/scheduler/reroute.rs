//! Materialize a routed value once at its producer, then share it along the chain.
use crate::plan::{PlanInputSource, PlanNodeImplementation, PlanOperation, ValueRef};
use std::borrow::Cow;
use std::collections::BTreeSet;

pub(super) fn evaluation_boundaries<'a>(
    operations: &[PlanOperation],
    producers: &[Option<usize>],
    boundaries: &'a BTreeSet<ValueRef>,
) -> Cow<'a, BTreeSet<ValueRef>> {
    let mut resolved = Cow::Borrowed(boundaries);
    let mut pending = boundaries.iter().copied().collect::<Vec<_>>();
    while let Some(value) = pending.pop() {
        let Some(operation) = producers
            .get(value.index() as usize)
            .copied()
            .flatten()
            .and_then(|index| operations.get(index))
        else {
            continue;
        };
        if operation.specialization().implementation() != &PlanNodeImplementation::Reroute {
            continue;
        }
        let [input] = operation.inputs() else {
            continue;
        };
        let PlanInputSource::Value(source) = input.source() else {
            continue;
        };
        if !resolved.contains(source) {
            resolved.to_mut().insert(*source);
            pending.push(*source);
        }
    }
    resolved
}
