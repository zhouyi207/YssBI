//! Application's staged consumer of the Execution finalization handoff.
//!
//! This module does not construct an Execution candidate or copy its result
//! state. It validates the handoff and exposes
//! only the neutral inspection projection needed by the later Presentation
//! mapping.

use std::collections::BTreeSet;

use thiserror::Error;

use yss_graph_execution::finalization::ExecutionFinalizationHandoff;
use yss_graph_execution::plan::PlanSourceIdentity;
use yss_graph_execution::result_store::ResultId;

/// One neutral inspection request mapped from an intent retained by the
/// committed handoff. The requester is an opaque identity value; no result,
/// grant owner is copied into this projection.
#[derive(Debug, Eq, PartialEq)]
pub struct ResultInspectionRequested {
    result_id: ResultId,
    requester: PlanSourceIdentity,
}

impl ResultInspectionRequested {
    pub const fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub fn requester(&self) -> &PlanSourceIdentity {
        &self.requester
    }
}

/// A typed invariant failure found while consuming a sealed handoff.
#[derive(Debug, Eq, Error, PartialEq)]
pub enum FinalizationInvariant {
    #[error("a committed result id is duplicated")]
    DuplicateResult { result_id: ResultId },
    #[error("a result observation intent has no committed result")]
    ObservationResultMissing { result_id: ResultId },
    #[error("a result observation intent is duplicated")]
    DuplicateObservation { result_id: ResultId },
}

#[derive(Debug, Eq, Error, PartialEq)]
pub enum FinalizationError {
    #[error(transparent)]
    Invariant(#[from] FinalizationInvariant),
}

/// Application's sealed post-commit outcome.
///
/// The handoff is retained as the source of truth. No result or observation
/// collection is rebuilt for the outcome, and no UI/presentation target is
/// stored here.
#[must_use = "a committed run outcome must be published or discarded"]
#[derive(Debug)]
pub struct CommittedRunOutcome {
    handoff: ExecutionFinalizationHandoff,
}

impl CommittedRunOutcome {
    pub fn handoff(&self) -> &ExecutionFinalizationHandoff {
        &self.handoff
    }

    pub fn results(&self) -> &[yss_graph_execution::finalization::ReadyResult] {
        self.handoff.results()
    }

    pub fn inspection_requests(&self) -> Box<[ResultInspectionRequested]> {
        self.handoff
            .observation_intents()
            .iter()
            .map(|intent| ResultInspectionRequested {
                result_id: intent.result_id,
                requester: intent.requester.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

/// Validate the exact Execution handoff after Result and Project authority have committed.
pub(crate) fn finalize_successful_run(
    handoff: ExecutionFinalizationHandoff,
) -> Result<CommittedRunOutcome, FinalizationError> {
    validate_handoff(&handoff).map_err(FinalizationError::Invariant)?;
    Ok(CommittedRunOutcome { handoff })
}

fn validate_handoff(handoff: &ExecutionFinalizationHandoff) -> Result<(), FinalizationInvariant> {
    let mut committed_results = BTreeSet::new();
    for result in handoff.results() {
        if !committed_results.insert(result.result_id()) {
            return Err(FinalizationInvariant::DuplicateResult {
                result_id: result.result_id(),
            });
        }
    }

    let mut observed_results = BTreeSet::new();
    for intent in handoff.observation_intents() {
        if !committed_results.contains(&intent.result_id) {
            return Err(FinalizationInvariant::ObservationResultMissing {
                result_id: intent.result_id,
            });
        }
        if !observed_results.insert(intent.result_id) {
            return Err(FinalizationInvariant::DuplicateObservation {
                result_id: intent.result_id,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_execution::finalization::test_support;

    #[test]
    fn committed_explicit_inspect_maps_once_and_ordinary_result_maps_nothing() {
        let result_id = ResultId::from_existing(17);
        let requester = test_support::requester();
        let candidate = test_support::candidate(result_id, requester.clone(), true);

        assert_eq!(candidate.results().len(), 1);
        assert_eq!(candidate.observation_intents().len(), 1);
        let handoff = candidate.into_finalization_handoff();
        assert_eq!(handoff.results()[0].result_id(), result_id);
        assert_eq!(handoff.observation_intents()[0].result_id, result_id);

        let outcome = finalize_successful_run(handoff).expect("valid owner fixture must finalize");
        let requests = outcome.inspection_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].result_id(), result_id);
        assert_eq!(requests[0].requester(), &requester);

        let ordinary_candidate = test_support::candidate(
            ResultId::from_existing(18),
            test_support::requester(),
            false,
        );
        let ordinary_outcome =
            finalize_successful_run(ordinary_candidate.into_finalization_handoff())
                .expect("ordinary owner fixture must finalize");
        assert!(ordinary_outcome.inspection_requests().is_empty());
    }
}
