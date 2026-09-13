//! Runtime entry point for DID randomization inference.
use yss_sci_contract::panel::{
    DidFakeGroupEnginePayload, DidFakeGroupError, DidPlaceboFakeGroupBlock,
};

pub fn compute_fake_group_ri(
    payload: &DidFakeGroupEnginePayload,
    n_perm: usize,
    rng_seed: u64,
) -> Result<DidPlaceboFakeGroupBlock, DidFakeGroupError> {
    yss_sci::regression::panel::did::compute_fake_group_ri(payload, n_perm, rng_seed)
}
