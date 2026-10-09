// VEC (Vector Error-Correction) 协整模型
//
// 按 Stata vec 命令的 Johansen (1995) 方法实现。
// 支持 trend(none), trend(constant), trend(trend)。

use super::distributions::{chi_squared_sf, normal_two_sided_p};
use super::vec_vecrank_cv::{max_eigen_critical_row, trace_critical_row};

use yss_sci_linalg::{MatrixExt, Solve};

use serde::{Deserialize, Serialize};
use yss_sci_linalg::{Col, Mat};

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

use crate::regression::design::covariance_rows;
use yss_sci_contract::regression::fit::RegressionCoefficientStatistics;
use yss_sci_contract::time_series::fit::{
    EquationStatistics, MultivariateStatistics, SerialCorrelationTest, StabilityRoot,
};
pub use yss_sci_contract::time_series::vec::VecFit;
use yss_sci_contract::time_series::vec::{
    CointegratingEquationStatistics, CointegrationStatistics,
};
