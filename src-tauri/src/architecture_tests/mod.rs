//! Crate-wide architecture invariants.

mod cargo_targets;
mod debt;
mod dependency_audit;
mod external_policy;
mod model;
mod policy;

#[cfg(test)]
mod semantic_guards;
#[cfg(test)]
mod tests;
