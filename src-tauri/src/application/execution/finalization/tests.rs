use super::*;

#[test]
fn candidate_transfers_only_through_consuming_handoff() {
    let result_id = crate::execution::result_store::ResultId::from_existing(7);
    let candidate = SuccessfulExecutionCandidate {
        results: vec![ReadyResult {
            result_id,
            value: crate::execution::result_store::StoredResult::Scalar(3.5),
        }]
        .into_boxed_slice(),
        effects: CandidateExecutionEffects {
            projections: vec![CandidateEffectProjection {
                resource: crate::execution::plan::PlanResourceId::from_existing(
                    "variables/answer".into(),
                ),
            }]
            .into_boxed_slice(),
        },
        observation_intents: vec![ResultObservationIntent {
            result_id,
            requester: crate::execution::plan::PlanSourceIdentity::new(
                crate::execution::plan::PlanGraphId::from_existing("events/main".into()),
                None,
                None,
            ),
        }]
        .into_boxed_slice(),
        resource_grants: SealedCandidateGrantSet {
            grants: Box::new([]),
        },
    };

    assert_eq!(candidate.results()[0].result_id(), result_id);
    assert_eq!(
        candidate.effect_projections()[0].resource().as_str(),
        "variables/answer"
    );
    assert_eq!(candidate.observation_intents()[0].result_id, result_id);

    let handoff = candidate.into_finalization_handoff();
    assert_eq!(handoff.results()[0].result_id(), result_id);
    assert_eq!(
        handoff.effect_projections()[0].resource().as_str(),
        "variables/answer"
    );
    assert_eq!(handoff.observation_intents()[0].result_id, result_id);
}
