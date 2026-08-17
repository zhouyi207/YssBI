//! Panel DID 结果页按需调用：虚构处理组置换检验

use crate::error::CommandError;
use crate::sci::models::panel_did::{
    ComputeDidFakeGroupRequest, DidPlaceboFakeGroupBlock, compute_fake_group_ri,
};

fn compute_panel_did_fake_group_ri_request(
    req: ComputeDidFakeGroupRequest,
) -> Result<DidPlaceboFakeGroupBlock, CommandError> {
    compute_fake_group_ri(&req.payload, req.n_perm, req.rng_seed).map_err(CommandError::internal)
}

#[tauri::command]
pub fn compute_panel_did_fake_group_ri(
    req: ComputeDidFakeGroupRequest,
) -> Result<DidPlaceboFakeGroupBlock, CommandError> {
    compute_panel_did_fake_group_ri_request(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sci::models::panel_did::{DidFakeGroupEnginePayload, ExogLabelEntry};

    fn malformed_request() -> ComputeDidFakeGroupRequest {
        ComputeDidFakeGroupRequest {
            payload: DidFakeGroupEnginePayload {
                endog: vec![1.0],
                exog_row_major: vec![],
                ncols: 1,
                all_labels: vec![ExogLabelEntry {
                    variable: "Treat×Post".into(),
                    category: None,
                }],
                entity_id: vec![0],
                time_id: vec![0],
                post: vec![1.0],
                treat: vec![1.0],
                did_label: "Treat×Post".into(),
                observed_coef: 1.0,
                constant: true,
                cov_type: "cluster".into(),
            },
            n_perm: 10,
            rng_seed: 7,
        }
    }

    #[test]
    fn pure_command_boundary_maps_engine_errors_to_command_error() {
        let error = compute_panel_did_fake_group_ri_request(malformed_request()).unwrap_err();

        assert_eq!(error.code(), "internal_error");
        assert!(error.details().is_none());
        assert!(error.incident_id().is_some());

        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire["code"], "internal_error");
        assert!(wire["incidentId"].is_string());
        assert!(wire.get("message").is_none());
    }
}
