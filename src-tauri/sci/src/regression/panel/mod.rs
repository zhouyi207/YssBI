//! Panel data regression: FE (Within), LSDV, FD, RE

mod fe;
mod fd;
mod lsdv;
mod re;

pub use fe::{fit_panel_fe, fit_panel_fe_time, fit_panel_fe_twoway};
pub use fd::fit_panel_fd;
pub use lsdv::{fit_panel_lsdv, fit_panel_lsdv_time};
pub use re::fit_panel_re;

use ndarray::{Array1, Array2};

/// FE-specific stats (Stata xtreg, fe style)
#[derive(Debug, Clone)]
pub struct PanelFEStats {
    pub r2_within: f64,
    pub r2_between: f64,
    pub r2_overall: f64,
    pub obs_per_group_min: usize,
    pub obs_per_group_avg: f64,
    pub obs_per_group_max: usize,
    pub sigma_u: f64,
    pub sigma_e: f64,
    pub rho: f64,
    pub corr_u_i_xb: f64,
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
    pub cond_no: f64,
}
