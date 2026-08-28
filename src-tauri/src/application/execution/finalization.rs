//! Application's staged consumer of the Execution finalization handoff.
//!
//! This module does not construct an Execution candidate or copy its result
//! state. It validates the handoff, seals the outer run identity, and exposes
//! only the neutral inspection projection needed by the later Presentation
//! mapping.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::execution::finalization::ExecutionFinalizationHandoff;
use crate::execution::plan::{PlanGraphId, PlanProjectSessionId, PlanSourceIdentity};
use crate::execution::result_store::ResultId;
use crate::execution::run_registry::RunId;

/// The outer identity used by the same run's start and terminal records.
///
/// This type is private so a caller cannot provide a presentation context
/// assembled independently from the run identity it is finalizing.
#[derive(Debug, Eq, PartialEq)]
struct RunPresentationContext {
    project_session_id: PlanProjectSessionId,
    root_graph: PlanGraphId,
    run_id: RunId,
}

impl RunPresentationContext {
    fn seal(
        project_session_id: PlanProjectSessionId,
        root_graph: PlanGraphId,
        run_id: RunId,
    ) -> Self {
        Self {
            project_session_id,
            root_graph,
            run_id,
        }
    }
}

/// One neutral inspection request mapped from an intent retained by the
/// committed handoff. The requester remains borrowed from that handoff.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ResultInspectionRequested<'a> {
    result_id: ResultId,
    requester: &'a PlanSourceIdentity,
}

impl ResultInspectionRequested<'_> {
    pub(crate) const fn result_id(&self) -> ResultId {
        self.result_id
    }

    pub(crate) fn requester(&self) -> &PlanSourceIdentity {
        self.requester
    }
}

/// A typed invariant failure found while consuming a sealed handoff.
#[derive(Debug, Eq, Error, PartialEq)]
pub(crate) enum FinalizationInvariant {
    #[error("a committed result id is duplicated")]
    DuplicateResult { result_id: ResultId },
    #[error("a result observation intent has no committed result")]
    ObservationResultMissing { result_id: ResultId },
    #[error("a result observation intent is duplicated")]
    DuplicateObservation { result_id: ResultId },
}

#[derive(Debug, Eq, Error, PartialEq)]
pub(crate) enum FinalizationError {
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
pub(crate) struct CommittedRunOutcome {
    handoff: ExecutionFinalizationHandoff,
    presentation_context: RunPresentationContext,
}

impl CommittedRunOutcome {
    pub(crate) fn inspection_requests(&self) -> Vec<ResultInspectionRequested<'_>> {
        self.handoff
            .observation_intents()
            .iter()
            .map(|intent| ResultInspectionRequested {
                result_id: intent.result_id,
                requester: &intent.requester,
            })
            .collect()
    }

    pub(crate) fn project_session_id(&self) -> &PlanProjectSessionId {
        &self.presentation_context.project_session_id
    }

    pub(crate) fn root_graph(&self) -> &PlanGraphId {
        &self.presentation_context.root_graph
    }

    pub(crate) const fn run_id(&self) -> RunId {
        self.presentation_context.run_id
    }
}

/// Consume the exact Execution handoff after Result and Project authority have
/// committed, then seal the outer run identity in one outcome.
pub(crate) fn finalize_successful_run(
    handoff: ExecutionFinalizationHandoff,
    project_session_id: PlanProjectSessionId,
    root_graph: PlanGraphId,
    run_id: RunId,
) -> Result<CommittedRunOutcome, FinalizationError> {
    validate_handoff(&handoff).map_err(FinalizationError::Invariant)?;

    Ok(CommittedRunOutcome {
        handoff,
        presentation_context: RunPresentationContext::seal(project_session_id, root_graph, run_id),
    })
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
    use crate::execution::finalization::test_support;
    use crate::execution::plan::PlanGraphId;

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

        let outcome = finalize_successful_run(
            handoff,
            PlanProjectSessionId::from_existing("project-session-7".into()),
            PlanGraphId::from_existing("events/Caller.yssbi-event".into()),
            RunId::from_existing(41),
        )
        .expect("valid owner fixture must finalize");
        let requests = outcome.inspection_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].result_id(), result_id);
        assert_eq!(requests[0].requester(), &requester);
        assert_eq!(outcome.project_session_id().as_str(), "project-session-7");
        assert_eq!(outcome.root_graph().as_str(), "events/Caller.yssbi-event");
        assert_eq!(outcome.run_id(), RunId::from_existing(41));

        let ordinary_candidate = test_support::candidate(
            ResultId::from_existing(18),
            test_support::requester(),
            false,
        );
        let ordinary_outcome = finalize_successful_run(
            ordinary_candidate.into_finalization_handoff(),
            PlanProjectSessionId::from_existing("project-session-7".into()),
            PlanGraphId::from_existing("events/Caller.yssbi-event".into()),
            RunId::from_existing(41),
        )
        .expect("ordinary owner fixture must finalize");
        assert!(ordinary_outcome.inspection_requests().is_empty());
    }
}
