//! Identity-neutral Panel DID fake-treatment-group randomization inference.

use ndarray::{Array1, Array2};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use yss_sci::regression::panel::fit_panel_fe_twoway;

const FAKE_GROUP_PERM_CAP: usize = 2000;
const FAKE_GROUP_PERM_MIN_VALID: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DidFakeGroupUnavailableCode {
    NoTreatedEntities,
    AllEntitiesTreated,
    InsufficientValidPermutations,
}

enum FakeGroupRiOutcome {
    Available {
        n_perm: usize,
        n_perm_valid: usize,
        n_entities: usize,
        n_treated_entities: usize,
        p_value_ri: f64,
        perm_coef_mean: f64,
        perm_coef_std: f64,
    },
    Unavailable {
        code: DidFakeGroupUnavailableCode,
        n_perm: usize,
        n_perm_valid: usize,
        n_entities: usize,
        n_treated_entities: usize,
    },
}

fn labels_after_fe_omit(
    labels: &[(String, Option<String>)],
    omitted: Option<&[usize]>,
) -> Vec<(String, Option<String>)> {
    match omitted {
        None | Some([]) => labels.to_vec(),
        Some(omitted) => (0..labels.len())
            .filter(|index| !omitted.contains(index))
            .filter_map(|index| labels.get(index).cloned())
            .collect(),
    }
}

fn run_placebo_fake_treatment_ri(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    labels: &[(String, Option<String>)],
    entity_id: &[usize],
    time_id: &[usize],
    post: &[f64],
    treat: &[f64],
    constant: bool,
    cov_type: &str,
    did_label: &str,
    observed_coef: f64,
    n_perm: usize,
    rng_seed: u64,
) -> Result<FakeGroupRiOutcome, String> {
    let n = endog.len();
    if exog.nrows() != n
        || entity_id.len() != n
        || time_id.len() != n
        || post.len() != n
        || treat.len() != n
    {
        return Err("DID fake-group placebo: length mismatch".to_string());
    }
    let ncols = exog.ncols();
    if labels.len() != ncols {
        return Err("DID fake-group placebo: label count != exog columns".to_string());
    }
    let last_column = ncols.saturating_sub(1);
    if labels
        .get(last_column)
        .map(|(name, _)| name.as_str() != did_label)
        .unwrap_or(true)
    {
        return Err(format!(
            "DID fake-group placebo: last exog column should be '{}', got {:?}",
            did_label,
            labels.get(last_column).map(|(name, _)| name.as_str())
        ));
    }

    let n_entities = entity_id
        .iter()
        .copied()
        .max()
        .map(|maximum| maximum + 1)
        .ok_or_else(|| "DID fake-group placebo: empty entity_id".to_string())?;
    let mut ever_treated = vec![false; n_entities];
    for index in 0..n {
        if treat[index] > 0.5 {
            ever_treated[entity_id[index]] = true;
        }
    }
    let n_treated_entities = ever_treated.iter().filter(|&&treated| treated).count();
    let n_rep = n_perm.max(1).min(FAKE_GROUP_PERM_CAP);
    if n_treated_entities == 0 {
        return Ok(FakeGroupRiOutcome::Unavailable {
            code: DidFakeGroupUnavailableCode::NoTreatedEntities,
            n_perm: n_rep,
            n_perm_valid: 0,
            n_entities,
            n_treated_entities,
        });
    }
    if n_treated_entities >= n_entities {
        return Ok(FakeGroupRiOutcome::Unavailable {
            code: DidFakeGroupUnavailableCode::AllEntitiesTreated,
            n_perm: n_rep,
            n_perm_valid: 0,
            n_entities,
            n_treated_entities,
        });
    }

    let mut rng = StdRng::seed_from_u64(rng_seed);
    let mut pool: Vec<usize> = (0..n_entities).collect();
    let mut permuted_exog = exog.clone();
    let mut coefficients = Vec::with_capacity(n_rep);

    for _ in 0..n_rep {
        pool.shuffle(&mut rng);
        let fake_treated: HashSet<usize> = pool.iter().take(n_treated_entities).copied().collect();
        for index in 0..n {
            let treatment = if fake_treated.contains(&entity_id[index]) {
                1.0
            } else {
                0.0
            };
            permuted_exog[[index, last_column]] = treatment * post[index];
        }

        let result = fit_panel_fe_twoway(
            endog,
            &permuted_exog,
            entity_id,
            time_id,
            constant,
            cov_type,
            None,
        )
        .map_err(|error| format!("DID fake-group placebo: TWFE fit failed: {error}"))?;
        let kept_labels = labels_after_fe_omit(labels, result.omitted_indices.as_deref());
        if let Some(index) = kept_labels.iter().position(|(name, _)| name == did_label) {
            let coefficient = result.betas.get(index).ok_or_else(|| {
                "DID fake-group placebo: TWFE coefficient index is out of bounds".to_string()
            })?;
            if !coefficient.is_finite() {
                return Err(
                    "DID fake-group placebo: TWFE returned a non-finite coefficient".to_string(),
                );
            }
            coefficients.push(*coefficient);
        }
    }

    let n_valid = coefficients.len();
    if n_valid < FAKE_GROUP_PERM_MIN_VALID {
        return Ok(FakeGroupRiOutcome::Unavailable {
            code: DidFakeGroupUnavailableCode::InsufficientValidPermutations,
            n_perm: n_rep,
            n_perm_valid: n_valid,
            n_entities,
            n_treated_entities,
        });
    }

    let count_at_least_observed = coefficients
        .iter()
        .filter(|coefficient| coefficient.abs() >= observed_coef.abs())
        .count();
    let p_value = (count_at_least_observed as f64 + 1.0) / (n_valid as f64 + 1.0);
    let mean = coefficients.iter().sum::<f64>() / n_valid as f64;
    let variance = coefficients
        .iter()
        .map(|coefficient| (coefficient - mean).powi(2))
        .sum::<f64>()
        / n_valid as f64;
    let standard_deviation = variance.max(0.0).sqrt();
    if !p_value.is_finite() || !mean.is_finite() || !standard_deviation.is_finite() {
        return Err("DID fake-group placebo: non-finite summary statistics".to_string());
    }

    Ok(FakeGroupRiOutcome::Available {
        n_perm: n_rep,
        n_perm_valid: n_valid,
        n_entities,
        n_treated_entities,
        p_value_ri: p_value,
        perm_coef_mean: mean,
        perm_coef_std: standard_deviation,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExogLabelEntry {
    pub variable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DidPlaceboFakeGroupBlock {
    pub available: bool,
    #[serde(rename = "unavailableCode", skip_serializing_if = "Option::is_none")]
    pub unavailable_code: Option<DidFakeGroupUnavailableCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_coef: Option<f64>,
    pub n_perm: usize,
    pub n_perm_valid: usize,
    pub min_valid_permutations: usize,
    pub n_entities: usize,
    pub n_treated_entities: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value_ri: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perm_coef_mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perm_coef_std: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDidFakeGroupRequest {
    #[serde(flatten)]
    pub payload: DidFakeGroupEnginePayload,
    pub n_perm: usize,
    pub rng_seed: u64,
}

pub fn compute_fake_group_ri(
    payload: &DidFakeGroupEnginePayload,
    n_perm: usize,
    rng_seed: u64,
) -> Result<DidPlaceboFakeGroupBlock, String> {
    let n = payload.endog.len();
    if !payload.observed_coef.is_finite() {
        return Err("observed_coef must be finite".to_string());
    }
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
        .map_err(|error| format!("exog reshape: {:?}", error))?;
    let endog = Array1::from_vec(payload.endog.clone());
    let labels: Vec<(String, Option<String>)> = payload
        .all_labels
        .iter()
        .map(|entry| (entry.variable.clone(), entry.category.clone()))
        .collect();
    let n_perm_requested = n_perm.max(1).min(FAKE_GROUP_PERM_CAP);

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
        n_perm_requested,
        rng_seed,
    )? {
        FakeGroupRiOutcome::Available {
            n_perm,
            n_perm_valid,
            n_entities,
            n_treated_entities,
            p_value_ri,
            perm_coef_mean,
            perm_coef_std,
        } => Ok(DidPlaceboFakeGroupBlock {
            available: true,
            unavailable_code: None,
            observed_coef: Some(payload.observed_coef),
            n_perm,
            n_perm_valid,
            min_valid_permutations: FAKE_GROUP_PERM_MIN_VALID,
            n_entities,
            n_treated_entities,
            p_value_ri: Some(p_value_ri),
            perm_coef_mean: Some(perm_coef_mean),
            perm_coef_std: Some(perm_coef_std),
        }),
        FakeGroupRiOutcome::Unavailable {
            code,
            n_perm,
            n_perm_valid,
            n_entities,
            n_treated_entities,
        } => Ok(DidPlaceboFakeGroupBlock {
            available: false,
            unavailable_code: Some(code),
            observed_coef: None,
            n_perm,
            n_perm_valid,
            min_valid_permutations: FAKE_GROUP_PERM_MIN_VALID,
            n_entities,
            n_treated_entities,
            p_value_ri: None,
            perm_coef_mean: None,
            perm_coef_std: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_payload() -> DidFakeGroupEnginePayload {
        let entities = 8;
        let periods = 6;
        let mut endog = Vec::with_capacity(entities * periods);
        let mut exog_row_major = Vec::with_capacity(entities * periods * 2);
        let mut entity_id = Vec::with_capacity(entities * periods);
        let mut time_id = Vec::with_capacity(entities * periods);
        let mut post = Vec::with_capacity(entities * periods);
        let mut treat = Vec::with_capacity(entities * periods);
        for entity in 0..entities {
            for time in 0..periods {
                let is_post = f64::from(time >= 3);
                let is_treated = f64::from(entity < 3);
                let did = is_post * is_treated;
                entity_id.push(entity);
                time_id.push(time);
                post.push(is_post);
                treat.push(is_treated);
                exog_row_major.extend([1.0, did]);
                endog.push(entity as f64 * 0.4 + time as f64 * 0.3 + did * 1.75);
            }
        }
        DidFakeGroupEnginePayload {
            endog,
            exog_row_major,
            ncols: 2,
            all_labels: vec![
                ExogLabelEntry {
                    variable: "const".into(),
                    category: None,
                },
                ExogLabelEntry {
                    variable: "Treat×Post".into(),
                    category: None,
                },
            ],
            entity_id,
            time_id,
            post,
            treat,
            did_label: "Treat×Post".into(),
            observed_coef: 1.75,
            constant: true,
            cov_type: "cluster".into(),
        }
    }

    fn unavailable(payload: &DidFakeGroupEnginePayload, n_perm: usize) -> DidPlaceboFakeGroupBlock {
        compute_fake_group_ri(payload, n_perm, 17).unwrap()
    }

    #[test]
    fn same_seed_is_exactly_deterministic_and_success_reports_structured_statistics() {
        let payload = valid_payload();
        let first = compute_fake_group_ri(&payload, 20, 42).unwrap();
        let second = compute_fake_group_ri(&payload, 20, 42).unwrap();

        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        assert!(first.available);
        assert_eq!(first.unavailable_code, None);
        assert_eq!(first.observed_coef, Some(1.75));
        assert_eq!(first.n_entities, 8);
        assert_eq!(first.n_treated_entities, 3);
        assert_eq!(first.n_perm, 20);
        assert_eq!(first.n_perm_valid, 20);
        assert_eq!(first.min_valid_permutations, 10);
        assert!(
            first
                .p_value_ri
                .is_some_and(|value| (0.0..=1.0).contains(&value))
        );
        assert!(first.perm_coef_mean.is_some_and(f64::is_finite));
        assert!(
            first
                .perm_coef_std
                .is_some_and(|value| value.is_finite() && value >= 0.0)
        );

        let wire = serde_json::to_value(first).unwrap();
        assert!(wire.get("method_note").is_none());
        assert!(wire.get("unavailableCode").is_none());
    }

    #[test]
    fn permutation_count_is_clamped_to_one_and_two_thousand() {
        let mut no_treated = valid_payload();
        no_treated.treat.fill(0.0);

        assert_eq!(unavailable(&no_treated, 0).n_perm, 1);
        assert_eq!(unavailable(&no_treated, 2001).n_perm, 2000);
    }

    #[test]
    fn fewer_than_ten_valid_permutations_has_a_stable_unavailable_code() {
        let result = unavailable(&valid_payload(), 9);

        assert!(!result.available);
        assert_eq!(
            result.unavailable_code,
            Some(DidFakeGroupUnavailableCode::InsufficientValidPermutations)
        );
        assert_eq!(result.n_entities, 8);
        assert_eq!(result.n_treated_entities, 3);
        assert_eq!(result.n_perm, 9);
        assert_eq!(result.n_perm_valid, 9);
        assert_eq!(result.min_valid_permutations, 10);
        assert_eq!(result.observed_coef, None);
        assert_eq!(result.p_value_ri, None);
        assert_eq!(result.perm_coef_mean, None);
        assert_eq!(result.perm_coef_std, None);
        assert_eq!(
            serde_json::to_value(result).unwrap()["unavailableCode"],
            "insufficient_valid_permutations"
        );
    }

    #[test]
    fn treatment_pool_limits_have_stable_unavailable_codes_and_counts() {
        let mut no_treated = valid_payload();
        no_treated.treat.fill(0.0);
        let no_treated_result = unavailable(&no_treated, 10);
        assert_eq!(
            no_treated_result.unavailable_code,
            Some(DidFakeGroupUnavailableCode::NoTreatedEntities)
        );
        assert_eq!(no_treated_result.n_entities, 8);
        assert_eq!(no_treated_result.n_treated_entities, 0);
        assert_eq!(no_treated_result.n_perm_valid, 0);

        let mut all_treated = valid_payload();
        all_treated.treat.fill(1.0);
        let all_treated_result = unavailable(&all_treated, 10);
        assert_eq!(
            all_treated_result.unavailable_code,
            Some(DidFakeGroupUnavailableCode::AllEntitiesTreated)
        );
        assert_eq!(all_treated_result.n_entities, 8);
        assert_eq!(all_treated_result.n_treated_entities, 8);
        assert_eq!(all_treated_result.n_perm_valid, 0);
    }

    #[test]
    fn malformed_structure_returns_diagnostic_errors() {
        let mut bad_shape = valid_payload();
        bad_shape.exog_row_major.pop();
        assert_eq!(
            compute_fake_group_ri(&bad_shape, 10, 1).unwrap_err(),
            "exog_row_major len 95 != n*ncols (48*2)"
        );

        let mut bad_labels = valid_payload();
        bad_labels.all_labels.clear();
        assert_eq!(
            compute_fake_group_ri(&bad_labels, 10, 1).unwrap_err(),
            "all_labels len != ncols"
        );

        let mut length = valid_payload();
        length.time_id.pop();
        assert_eq!(
            compute_fake_group_ri(&length, 10, 1).unwrap_err(),
            "DID fake-group placebo: length mismatch"
        );

        let mut wrong_last = valid_payload();
        wrong_last.all_labels[1].variable = "other".into();
        assert_eq!(
            compute_fake_group_ri(&wrong_last, 10, 1).unwrap_err(),
            "DID fake-group placebo: last exog column should be 'Treat×Post', got Some(\"other\")"
        );

        let mut empty = valid_payload();
        empty.endog.clear();
        empty.exog_row_major.clear();
        empty.entity_id.clear();
        empty.time_id.clear();
        empty.post.clear();
        empty.treat.clear();
        assert_eq!(
            compute_fake_group_ri(&empty, 10, 1).unwrap_err(),
            "DID fake-group placebo: empty entity_id"
        );
    }

    #[test]
    fn regression_engine_errors_are_not_reported_as_statistical_unavailability() {
        let mut payload = valid_payload();
        payload.cov_type = "hac-panel".into();

        let error = compute_fake_group_ri(&payload, 10, 1).unwrap_err();

        assert!(error.contains("cov_type 'hac-panel' not yet implemented"));
    }

    #[test]
    fn deserialization_rejects_legacy_method_note_shapes() {
        let mut success =
            serde_json::to_value(compute_fake_group_ri(&valid_payload(), 10, 1).unwrap()).unwrap();
        success.as_object_mut().unwrap().insert(
            "method_note".into(),
            serde_json::json!("legacy backend prose"),
        );
        assert!(serde_json::from_value::<DidPlaceboFakeGroupBlock>(success).is_err());

        let legacy_unavailable = serde_json::json!({
            "available": false,
            "observed_coef": 1.75,
            "n_perm": 9,
            "n_perm_valid": 0,
            "method_note": "legacy failure prose"
        });
        assert!(serde_json::from_value::<DidPlaceboFakeGroupBlock>(legacy_unavailable).is_err());
    }
}
