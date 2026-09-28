use serde::{Deserialize, Serialize};

/// Observations per group (min/avg/max)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsPerGroupInfo {
    pub min: usize,
    pub avg: f64,
    pub max: usize,
}

/// Variance decomposition: σ_u, σ_e, ρ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmaInfo {
    pub sigma_u: f64,
    pub sigma_e: f64,
    pub rho: f64,
}

/// RE quasi-demeaning parameter θ (min/avg/max across groups)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThetaInfo {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

/// Panel FE-specific stats (Stata xtreg, fe)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct PanelFEInfo {
    /// R² Within/Between/Overall. None for MLE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_within: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_between: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r2_overall: Option<f64>,
    pub num_groups: usize,
    pub obs_per_group: ObsPerGroupInfo,
    pub sigma: SigmaInfo,
    pub corr_u_i_Xb: f64,
    /// RE quasi-demeaning parameter θ (min/avg/max across groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<ThetaInfo>,
    /// MLE: chibar2(01) for sigma_u=0 (Stata xtreg, mle)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chibar2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_chibar2: Option<f64>,
}
