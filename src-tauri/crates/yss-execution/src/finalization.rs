//! Execution-owned result finalization handoff.
//!
//! Execution creates the sealed candidate after result/resource validation.
//! Application can consume the one-way handoff, but it cannot construct a
//! candidate, replace one of its results, or detach its grant evidence.

use crate::plan::{
    PlanCompileId, PlanResourceId, PlanResourceVersion, PlanSourceIdentity, ResourceAccess,
    ResourceKind,
};
use crate::result_store::{ResultId, StoredResult};

/// A result that became ready during one Execution run.
#[derive(Debug, PartialEq)]
pub struct ReadyResult {
    result_id: ResultId,
    value: StoredResult,
}

impl ReadyResult {
    pub(crate) fn from_scheduler(
        result_id: ResultId,
        value: StoredResult,
        category: crate::plan::ResultCategory,
    ) -> Self {
        Self {
            result_id,
            value: StoredResult::with_category(value, category),
        }
    }

    pub fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn value(&self) -> &StoredResult {
        &self.value
    }

    pub fn category(&self) -> crate::plan::ResultCategory {
        self.value.category()
    }
}

/// Execution-private evidence that a candidate was checked against the
/// compiled resource basis. The fields never cross the Application seam.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "grant evidence is retained by the sealed candidate until finalization"
)]
pub(crate) struct SealedCandidateGrant {
    compile_id: PlanCompileId,
    resource: PlanResourceId,
    version: PlanResourceVersion,
    kind: ResourceKind,
    access: ResourceAccess,
}

impl SealedCandidateGrant {
    pub(crate) fn new(
        compile_id: PlanCompileId,
        resource: PlanResourceId,
        version: PlanResourceVersion,
        kind: ResourceKind,
        access: ResourceAccess,
    ) -> Self {
        Self {
            compile_id,
            resource,
            version,
            kind,
            access,
        }
    }
}

#[derive(Debug)]
#[allow(
    dead_code,
    reason = "grant evidence is retained by the sealed candidate until finalization"
)]
pub(crate) struct SealedCandidateGrantSet {
    grants: Box<[SealedCandidateGrant]>,
}

impl SealedCandidateGrantSet {
    pub(crate) fn new(grants: Box<[SealedCandidateGrant]>) -> Self {
        Self { grants }
    }
}

/// A neutral request to inspect a committed result.
///
/// The requester is an opaque plan source. It contains no window, panel,
/// route, title, target, or delivery sink.
#[derive(Debug, Eq, PartialEq)]
pub struct ResultObservationIntent {
    pub result_id: ResultId,
    pub requester: PlanSourceIdentity,
}

/// Execution's sealed successful candidate.
///
/// There is intentionally no public constructor, `Clone`, `Default`, serde
/// implementation, type erasure, or public field access. Only an Execution
/// owner can build this from scheduler-owned sealed evidence.
#[must_use = "a successful execution candidate must be finalized or discarded"]
#[derive(Debug)]
pub struct SuccessfulExecutionCandidate {
    results: Box<[ReadyResult]>,
    observation_intents: Box<[ResultObservationIntent]>,
    #[allow(
        dead_code,
        reason = "the private grant seal moves with the candidate and handoff"
    )]
    resource_grants: SealedCandidateGrantSet,
}

impl SuccessfulExecutionCandidate {
    /// Build a candidate from Execution-owned scheduler output and sealed
    /// resource evidence. The visibility deliberately excludes Application.
    pub(crate) fn from_scheduler(
        results: Box<[ReadyResult]>,
        observation_intents: Box<[ResultObservationIntent]>,
        resource_grants: SealedCandidateGrantSet,
    ) -> Self {
        Self {
            results,
            observation_intents,
            resource_grants,
        }
    }

    pub fn results(&self) -> &[ReadyResult] {
        &self.results
    }

    pub fn observation_intents(&self) -> &[ResultObservationIntent] {
        &self.observation_intents
    }

    /// Consume the candidate into the only Application finalization handoff.
    /// The private grant evidence moves with the candidate.
    pub fn into_finalization_handoff(self) -> ExecutionFinalizationHandoff {
        ExecutionFinalizationHandoff { candidate: self }
    }
}

/// The one-way handoff accepted by Application finalization.
///
/// It owns the original candidate, so later finalization cannot rebuild a
/// result, grant, or observation-intent collection independently.
#[must_use = "a finalization handoff must be committed or rejected"]
#[derive(Debug)]
pub struct ExecutionFinalizationHandoff {
    candidate: SuccessfulExecutionCandidate,
}

impl ExecutionFinalizationHandoff {
    pub fn results(&self) -> &[ReadyResult] {
        self.candidate.results()
    }

    pub fn observation_intents(&self) -> &[ResultObservationIntent] {
        self.candidate.observation_intents()
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    use crate::plan::{PlanGraphId, PlanSourceIdentity};

    /// Test-only owner fixture. Production code has no equivalent constructor.
    pub fn candidate(
        result_id: ResultId,
        requester: PlanSourceIdentity,
        explicit_inspection: bool,
    ) -> SuccessfulExecutionCandidate {
        let observation_intents = if explicit_inspection {
            vec![ResultObservationIntent {
                result_id,
                requester,
            }]
        } else {
            Vec::new()
        };

        SuccessfulExecutionCandidate::from_scheduler(
            vec![ReadyResult::from_scheduler(
                result_id,
                StoredResult::Scalar(3.5),
                crate::plan::ResultCategory::Value,
            )]
            .into_boxed_slice(),
            observation_intents.into_boxed_slice(),
            SealedCandidateGrantSet {
                grants: Box::new([]),
            },
        )
    }

    pub fn requester() -> PlanSourceIdentity {
        PlanSourceIdentity::new(
            PlanGraphId::from_existing("functions/Inspect.yssbi-function".into()),
            Some(crate::plan::PlanNodeId::from_existing(
                "00000000-0000-0000-0000-000000000009".into(),
            )),
            None,
        )
    }
}
