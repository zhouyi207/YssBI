//! Panel DID 结果页按需调用：虚构处理组置换检验

use crate::graph::register::catalog::dataframe::{
    ComputeDidFakeGroupRequest, DidPlaceboFakeGroupBlock, compute_fake_group_ri,
};

#[tauri::command]
pub fn compute_panel_did_fake_group_ri(
    req: ComputeDidFakeGroupRequest,
) -> Result<DidPlaceboFakeGroupBlock, String> {
    compute_fake_group_ri(&req.payload, req.n_perm, req.rng_seed)
}
