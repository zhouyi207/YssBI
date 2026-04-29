use serde::{Deserialize, Serialize};

/// varlmar 单行（LM 残差自相关检验）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARLmarDisplay {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varwle 单行（Wald lag-exclusion）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARWleDisplay {
    pub eq_name: String,
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varstable 单行（特征值平稳性检验，Stata varstable 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARStableDisplay {
    pub re: f64,
    pub im: f64,
    pub modulus: f64,
}

/// vargranger 单行（格兰杰因果 Wald 检验，Stata vargranger 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARGrangerDisplay {
    pub eq_name: String,
    pub excluded: String,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// VAR Summary 窗口展示用（与 OLS Summary 形式类似）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARSummaryResult {
    pub title: String,
    pub var_names: Vec<String>,
    /// 对齐行数 T：无 exog 时为 listwise 后的压缩行数；有 exog 时为与 DataFrame 对齐的完整时间轴长度（Stata 式）
    pub complete_sample_rows: usize,
    /// 模型最大滞后阶 p（与 Lags 引脚一致）；估计样本 n = T − p，见 `num_observation`
    pub var_max_lag: usize,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub fpe: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub equations: Vec<VAREquationDisplay>,
    pub coefficients: Vec<VARCoefDisplay>,
    pub sigma: Vec<Vec<f64>>,
    pub oirf: Vec<Vec<Vec<f64>>>,
    pub fevd: Vec<Vec<Vec<f64>>>,
    pub varwle: Vec<VARWleDisplay>,
    pub varlmar: Vec<VARLmarDisplay>,
    pub varstable: Vec<VARStableDisplay>,
    pub vargranger: Vec<VARGrangerDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAREquationDisplay {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARCoefDisplay {
    pub eq_name: String,
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub z_value: f64,
    pub p_value: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}
