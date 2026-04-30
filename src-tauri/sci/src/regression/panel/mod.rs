//! Panel data regression: FE (Within), LSDV, FD, RE

mod fd;
mod fe;
mod lsdv;
mod re;

pub use fd::fit_panel_fd;
pub use fe::{fit_panel_fe, fit_panel_fe_time, fit_panel_fe_twoway};
pub use lsdv::{fit_panel_lsdv, fit_panel_lsdv_time, fit_panel_lsdv_twoway};
pub use re::{
    fit_panel_re, fit_panel_re_be, fit_panel_re_be_time, fit_panel_re_fgls, fit_panel_re_fgls_time,
    fit_panel_re_fgls_twoway, fit_panel_re_mle, fit_panel_re_mle_time, fit_panel_re_mle_twoway,
};

use ndarray::{Array1, Array2};

/// R² Within/Between/Overall. None for MLE (does not report these).
#[derive(Debug, Clone)]
pub struct PanelR2Stats {
    pub r2_within: f64,
    pub r2_between: f64,
    pub r2_overall: f64,
}

/// Observations per group (min/avg/max across entities or time periods).
#[derive(Debug, Clone)]
pub struct ObsPerGroupStats {
    pub min: usize,
    pub avg: f64,
    pub max: usize,
}

/// Variance decomposition: σ_u, σ_e, ρ = σ²_u / (σ²_u + σ²_e).
#[derive(Debug, Clone)]
pub struct SigmaStats {
    pub sigma_u: f64,
    pub sigma_e: f64,
    pub rho: f64,
}

/// RE quasi-demeaning parameter θ = 1 - sqrt(σ²_e / (T_i·σ²_u + σ²_e)).
/// For balanced panels all three are equal; for unbalanced they differ.
#[derive(Debug, Clone)]
pub struct ThetaStats {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}

/// FE-specific stats (Stata xtreg, fe style)
#[derive(Debug, Clone)]
pub struct PanelFEStats {
    pub r2: Option<PanelR2Stats>,
    pub obs_per_group: ObsPerGroupStats,
    pub sigma: SigmaStats,
    pub corr_u_i_xb: f64,
    pub theta: Option<ThetaStats>,
}

/// Common result structure for panel OLS-style estimators (FE, FD, RE)
#[derive(Debug, Clone)]
pub struct PanelOLSResult {
    /// Recovered constant _cons = ȳ - β'x̄ (FE only, Stata-style). None for FD.
    pub const_coef: Option<f64>,
    /// Standard error of recovered constant (FE only)
    pub const_std_err: Option<f64>,
    /// FE-specific stats (Stata xtreg, fe). None for FD/RE.
    pub fe_stats: Option<PanelFEStats>,
    pub num_observation: usize,
    pub num_entities: usize,
    pub num_time_periods: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
    pub r2: f64,
    pub r2_adjusted: f64,
    pub r2_within: Option<f64>,
    pub fvalue: f64,
    pub f_p_value: f64,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    /// Nonrobust VCE: σ² (X'X)⁻¹ for Hausman test
    pub cov_beta_nonrobust: Option<Array2<f64>>,
    pub cond_no: f64,
    /// Column indices omitted due to collinearity (LSDV/LSDV-time full matrix). Used by caller to build omit_info.
    pub omitted_indices: Option<Vec<usize>>,
    /// Wald chi2 for RE FGLS (Stata xtreg, re). None for FE/BE/MLE.
    pub wald_chi2: Option<f64>,
    /// Prob > chi2 for Wald test. None for FE/BE/MLE.
    pub prob_wald_chi2: Option<f64>,
    /// Log likelihood (MLE only). None for FGLS/FE/BE.
    pub log_likelihood: Option<f64>,
    /// LR chi2 for MLE (Stata xtreg, mle). None for FGLS/FE/BE.
    pub lr_chi2: Option<f64>,
    /// Prob > chi2 for LR test. None for FGLS/FE/BE.
    pub prob_lr_chi2: Option<f64>,
    /// chibar2(01) for sigma_u=0 (MLE only). None for FGLS/FE/BE.
    pub chibar2: Option<f64>,
    /// Prob >= chibar2 for sigma_u=0 test. None for FGLS/FE/BE.
    pub prob_chibar2: Option<f64>,
    /// MLE constant-only model iterations (Stata "Fitting constant-only model"). None for non-MLE.
    pub mle_iter_log_lik_const: Option<Vec<f64>>,
    /// MLE full model iterations (Stata "Fitting full model"). None for non-MLE.
    pub mle_iter_log_lik: Option<Vec<f64>>,
}
