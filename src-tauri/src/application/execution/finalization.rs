//! Sealed handoff between the staged Execution result and Application
//! finalization. Construction remains private until the staged run graph is
//! promoted by the atomic production cutover.

use crate::execution::plan::{
    PlanCompileId, PlanResourceId, PlanResourceVersion, PlanSourceIdentity, ResourceAccess,
    ResourceKind,
};
use crate::execution::result_store::{ResultId, StoredResult};

/// A result that has become ready inside one Execution session.
///
/// The fields stay private so callers can inspect the result without being
/// able to manufacture a result that bypasses the Execution-owned checks.
#[derive(Debug, PartialEq)]
pub struct ReadyResult {
    result_id: ResultId,
    value: StoredResult,
}

impl ReadyResult {
    pub fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn value(&self) -> &StoredResult {
        &self.value
    }
}

/// A safe, neutral description of an effect candidate.
///
/// Application may inspect this projection, but it does not receive the
/// mutable effect buffer or a capability that can apply the effect.
#[derive(Debug, Eq, PartialEq)]
pub struct CandidateEffectProjection {
    resource: PlanResourceId,
}

impl CandidateEffectProjection {
    pub fn resource(&self) -> &PlanResourceId {
        &self.resource
    }
}

#[derive(Debug)]
struct CandidateExecutionEffects {
    projections: Box<[CandidateEffectProjection]>,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "grant evidence is intentionally retained without an Application read surface"
)]
struct SealedCandidateGrant {
    compile_id: PlanCompileId,
    resource: PlanResourceId,
    version: PlanResourceVersion,
    kind: ResourceKind,
    access: ResourceAccess,
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "grant evidence is intentionally retained without an Application read surface"
)]
struct SealedCandidateGrantSet {
    grants: Box<[SealedCandidateGrant]>,
}

/// A neutral request for a result to be inspected after authority commits.
///
/// This fact contains no window, panel, route, title, or delivery sink.
#[derive(Debug, Eq, PartialEq)]
pub struct ResultObservationIntent {
    pub result_id: ResultId,
    pub requester: PlanSourceIdentity,
}

/// Execution's sealed successful candidate.
///
/// There is deliberately no public constructor, `Clone`, `Default`, serde
/// implementation, or public field. The staged Execution run graph will
/// construct it once its private resource grants and result checks are in
/// place.
#[must_use = "a successful execution candidate must be finalized or discarded"]
#[derive(Debug)]
pub struct SuccessfulExecutionCandidate {
    results: Box<[ReadyResult]>,
    effects: CandidateExecutionEffects,
    observation_intents: Box<[ResultObservationIntent]>,
    #[allow(
        dead_code,
        reason = "the sealed grant set is consumed with the candidate by the staged handoff"
    )]
    resource_grants: SealedCandidateGrantSet,
}

impl SuccessfulExecutionCandidate {
    pub fn results(&self) -> &[ReadyResult] {
        &self.results
    }

    pub fn effect_projections(&self) -> &[CandidateEffectProjection] {
        &self.effects.projections
    }

    pub fn observation_intents(&self) -> &[ResultObservationIntent] {
        &self.observation_intents
    }

    /// Consume the exact candidate into the only Application finalization
    /// handoff. The private grant evidence moves with it.
    pub fn into_finalization_handoff(self) -> ExecutionFinalizationHandoff {
        ExecutionFinalizationHandoff { candidate: self }
    }
}

/// The one-way handoff accepted by the Application finalizer.
///
/// It remains non-cloneable and keeps the original candidate intact. Later
/// finalization work can consume this type to create a committed outcome
/// without reconstructing results, effects, or observation intents.
#[must_use = "a finalization handoff must be committed or rejected"]
#[derive(Debug)]
pub struct ExecutionFinalizationHandoff {
    candidate: SuccessfulExecutionCandidate,
}

impl ExecutionFinalizationHandoff {
    pub fn results(&self) -> &[ReadyResult] {
        self.candidate.results()
    }

    pub fn effect_projections(&self) -> &[CandidateEffectProjection] {
        self.candidate.effect_projections()
    }

    pub fn observation_intents(&self) -> &[ResultObservationIntent] {
        self.candidate.observation_intents()
    }
}

#[cfg(test)]
mod tests;
