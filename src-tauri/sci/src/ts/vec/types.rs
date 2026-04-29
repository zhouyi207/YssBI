// VEC (Vector Error-Correction) 协整模型
//
// 按 Stata vec 命令的 Johansen (1995) 方法实现。
// 支持 trend(none), trend(constant), trend(trend)。

use super::distributions::{chi_squared_sf, normal_cdf, normal_two_sided_p};
use super::vec_vecrank_cv::{max_eigen_critical_row, trace_critical_row};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{linalg::solvers::Solve, Mat, Side};
use faer::linalg::solvers::Eigen;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// 趋势设定：与 Stata trend() 对应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VecTrendSpec {
    /// trend(none): 无常数无趋势
    None,
    /// trend(constant): 无约束常数（默认）
    Constant,
    /// trend(trend): 协整方程含线性趋势，水平数据含二次趋势
    Trend,
}

/// VEC 配置
#[derive(Debug, Clone)]
pub struct VECConfig {
    pub trend_spec: VecTrendSpec,
    pub lags: usize,
    pub rank: usize,
    /// veclmar 最大滞后阶数，默认 2
    pub mlag: usize,
}

/// VEC 估计结果（Stata vec 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECResult {
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub rank: usize,
    pub lags: usize,
    pub trend_spec: String,
    /// 协整向量 β：变量行 + 可选 const 行，(K+1)×r 当 has_const 否则 K×r
    pub beta: Vec<Vec<f64>>,
    /// 短 run 系数：每方程 [Γ1, Γ2, ..., const/trend]
    pub coefficients: Vec<Vec<f64>>,
    pub std_errs: Vec<Vec<f64>>,
    pub z_values: Vec<Vec<f64>>,
    pub p_values: Vec<Vec<f64>>,
    pub ci_lower: Vec<Vec<f64>>,
    pub ci_upper: Vec<Vec<f64>>,
    pub coef_labels: Vec<Vec<String>>,
    pub equations: Vec<VECEquationStats>,
    /// 协整方程统计（Stata Cointegrating equations 表）
    pub cointegrating_equations: Vec<VECCointegratingEquationStats>,
    /// beta 表 Stata 风格：每元素 [std_err, z, p_value, ci_lower, ci_upper]，归一化/常数用 None
    pub beta_std_err: Vec<Vec<Option<f64>>>,
    pub beta_z_value: Vec<Vec<Option<f64>>>,
    pub beta_p_value: Vec<Vec<Option<f64>>>,
    pub beta_ci_lower: Vec<Vec<Option<f64>>>,
    pub beta_ci_upper: Vec<Vec<Option<f64>>>,
    /// veclmar: LM 残差自相关检验（Stata veclmar 命令）
    pub veclmar: Vec<VecLmarRow>,
    /// vecstable: 特征值平稳性检验（Stata vecstable 命令）
    pub vecstable: Vec<VecStableRow>,
}

/// Stata `vecrank` 风格输出（Johansen trace / max eigenvalue）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecRankRow {
    pub rank: usize,
    pub log_likelihood: f64,
    pub eigenvalue: Option<f64>,
    pub trace_statistic: Option<f64>,
    /// 右尾检验在 10% / 5% / 1% 显著性下的临界值（与 Stata/R 10pct·5pct·1pct 列一致）
    pub trace_crit_10pct: Option<f64>,
    pub trace_crit_5pct: Option<f64>,
    pub trace_crit_1pct: Option<f64>,
    pub max_eigenvalue_statistic: Option<f64>,
    pub max_eigen_crit_10pct: Option<f64>,
    pub max_eigen_crit_5pct: Option<f64>,
    pub max_eigen_crit_1pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecRankResult {
    pub kind: String,
    pub title: String,
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub n_lags: usize,
    pub trend_spec: String,
    pub show_max_eigen: bool,
    pub selected_rank_trace_95: usize,
    pub selected_rank_trace_99: usize,
    pub selected_rank_max_95: usize,
    pub selected_rank_max_99: usize,
    pub rows: Vec<VecRankRow>,
    pub note: String,
}

/// veclmar 单行：lag 阶的 LM 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecLmarRow {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// vecstable 单行：特征值及其模
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VecStableRow {
    pub re: f64,
    pub im: f64,
    pub modulus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECCointegratingEquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECEquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

