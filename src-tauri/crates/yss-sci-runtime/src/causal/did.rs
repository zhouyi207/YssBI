//! Runtime entry point for DID randomization inference.
pub fn randomization_test(
    input: yss_sci_contract::causal::did::DidRandomizationInput,
    control: &yss_sci_contract::execution::ScientificExecutionControl,
) -> Result<
    yss_sci_contract::causal::did::DidPlaceboFakeGroupBlock,
    yss_sci_contract::causal::did::DidFakeGroupError,
> {
    yss_sci::causal::did::randomization_test(input, control)
}
use yss_sci_contract::causal::did::{
    DidFakeGroupEnginePayload, DidFakeGroupError, DidPlaceboFakeGroupBlock,
};

pub fn compute_fake_group_ri(
    payload: &DidFakeGroupEnginePayload,
    n_perm: usize,
    rng_seed: u64,
) -> Result<DidPlaceboFakeGroupBlock, DidFakeGroupError> {
    yss_sci::causal::did::compute_fake_group_ri(payload, n_perm, rng_seed)
}

/// TWFE DID entry point; report encoding stays in the runtime.
pub fn fit_did(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
    treatment: Vec<f64>,
) -> Result<serde_json::Value, yss_sci_contract::SciError> {
    let fit = yss_sci::causal::did::fit_did(response, predictors, entity, time, treatment)?;
    serde_json::to_value(fit)
        .map_err(|_| crate::error::computation_failed(yss_sci_contract::SciOperationCode::Panel))
}
