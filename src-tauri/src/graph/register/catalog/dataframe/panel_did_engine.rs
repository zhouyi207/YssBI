//! Panel DID: on-demand fake-treatment-group RI (payload for Tauri + JSON in result window).

use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

use super::panel_did_auxiliary::run_placebo_fake_treatment_ri;

/// Serialized exog column label (matches `info_nodes` coefficient naming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExogLabelEntry {
    pub variable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Enough data to re-run fake-group permutation TWFE from the info window (no graph re-exec).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidFakeGroupEnginePayload {
    pub endog: Vec<f64>,
    pub exog_row_major: Vec<f64>,
    pub ncols: usize,
    pub all_labels: Vec<ExogLabelEntry>,
    pub entity_id: Vec<usize>,
    pub time_id: Vec<usize>,
    pub post: Vec<f64>,
    pub treat: Vec<f64>,
    pub did_label: String,
    pub observed_coef: f64,
    pub constant: bool,
    pub cov_type: String,
}

/// Result of fake-group placebo / RI (same JSON shape as before when computed server-side).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidPlaceboFakeGroupBlock {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_coef: Option<f64>,
    pub n_perm: usize,
    pub n_perm_valid: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value_ri: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perm_coef_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perm_coef_std: Option<f64>,
    pub method_note: String,
}

/// Request body: flattened payload + run parameters (Tauri invoke).
#[derive(Debug, Deserialize)]
pub struct ComputeDidFakeGroupRequest {
    #[serde(flatten)]
    pub payload: DidFakeGroupEnginePayload,
    pub n_perm: usize,
    pub rng_seed: u64,
}

pub fn compute_fake_group_ri(
    payload: &DidFakeGroupEnginePayload,
    n_perm: usize,
    seed: u64,
) -> Result<DidPlaceboFakeGroupBlock, String> {
    let n = payload.endog.len();
    if payload.exog_row_major.len() != n.saturating_mul(payload.ncols) {
        return Err(format!(
            "exog_row_major len {} != n*ncols ({}*{})",
            payload.exog_row_major.len(),
            n,
            payload.ncols
        ));
    }
    if payload.all_labels.len() != payload.ncols {
        return Err("all_labels len != ncols".to_string());
    }
    let exog = Array2::from_shape_vec((n, payload.ncols), payload.exog_row_major.clone())
        .map_err(|e| format!("exog reshape: {:?}", e))?;
    let endog = Array1::from_vec(payload.endog.clone());
    let labels: Vec<(String, Option<String>)> = payload
        .all_labels
        .iter()
        .map(|e| (e.variable.clone(), e.category.clone()))
        .collect();
    let n_perm_req = n_perm.max(1).min(2000);

    match run_placebo_fake_treatment_ri(
        &endog,
        &exog,
        &labels,
        &payload.entity_id,
        &payload.time_id,
        &payload.post,
        &payload.treat,
        payload.constant,
        &payload.cov_type,
        &payload.did_label,
        payload.observed_coef,
        n_perm_req,
        seed,
    ) {
        Ok((n_rep, n_valid, p_ri, mean_c, std_c, note)) => Ok(DidPlaceboFakeGroupBlock {
            available: true,
            observed_coef: Some(payload.observed_coef),
            n_perm: n_rep,
            n_perm_valid: n_valid,
            p_value_ri: Some(p_ri),
            perm_coef_mean: Some(mean_c),
            perm_coef_std: Some(std_c),
            method_note: note,
        }),
        Err(msg) => Ok(DidPlaceboFakeGroupBlock {
            available: false,
            observed_coef: Some(payload.observed_coef),
            n_perm: n_perm_req,
            n_perm_valid: 0,
            p_value_ri: None,
            perm_coef_mean: None,
            perm_coef_std: None,
            method_note: msg,
        }),
    }
}
