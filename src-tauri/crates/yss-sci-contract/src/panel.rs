//! Neutral DID inputs and randomization-inference results.
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DidFakeGroupError {
    #[error("observed coefficient must be finite")]
    NonFiniteObservedCoefficient,
    #[error("exogenous matrix dimensions overflow")]
    ExogShapeOverflow,
    #[error("exogenous matrix has {actual} values; expected {expected}")]
    ExogShape { actual: usize, expected: usize },
    #[error("exogenous label count does not match the column count")]
    LabelCount,
    #[error("DID input vector lengths do not match the response row count")]
    LengthMismatch,
    #[error("DID numeric inputs must be finite")]
    NonFiniteInput,
    #[error("the last exogenous label must be the DID term")]
    DidLabelPosition {
        expected: String,
        actual: Option<String>,
    },
    #[error("DID entity input must not be empty")]
    EmptyEntities,
    #[error("DID fake-group TWFE fit failed")]
    FitFailed { diagnostic: String },
    #[error("DID coefficient index is out of bounds")]
    CoefficientIndex,
    #[error("DID coefficient must be finite")]
    NonFiniteCoefficient,
    #[error("DID summary statistics must be finite")]
    NonFiniteSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DidFakeGroupUnavailableCode {
    NoTreatedEntities,
    AllEntitiesTreated,
    InsufficientValidPermutations,
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
