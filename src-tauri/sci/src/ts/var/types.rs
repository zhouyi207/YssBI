// VAR (Vector Autoregression) 模型与 IRF (Impulse Response Function)
//
// 实现与 Stata varbasic 一致：VAR(p) 估计（每方程 OLS）、正交化 IRF、FEVD。
// 参考 Lutkepohl (2005) New Introduction to Multiple Time Series Analysis.

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use super::distributions::{chi_squared_sf, normal_two_sided_p};

/// Cholesky 分解 L 使得 A = L L'，L 为下三角，原地覆盖 A 的下三角部分
fn cholesky_lower_in_place(a: &mut Array2<f64>) -> Result<(), ()> {
    let n = a.nrows();
    if a.ncols() != n {
        return Err(());
    }
    for j in 0..n {
        let mut s = 0.0;
        for k in 0..j {
            s += a[[j, k]].powi(2);
        }
        let d = a[[j, j]] - s;
        if d <= 0.0 {
            return Err(());
        }
        let ljj = d.sqrt();
        a[[j, j]] = ljj;
        for i in (j + 1)..n {
            let mut s = 0.0;
            for k in 0..j {
                s += a[[i, k]] * a[[j, k]];
            }
            a[[i, j]] = (a[[i, j]] - s) / ljj;
        }
        for i in 0..j {
            a[[i, j]] = 0.0;
        }
    }
    Ok(())
}

use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// VAR 配置
#[derive(Debug, Clone)]
pub struct VARConfig {
    /// 是否包含常数项
    pub constant: bool,
    /// 滞后阶数列表，如 [1, 2] 表示 L1 和 L2
    pub lags: Vec<usize>,
    /// IRF 步数（预测 horizon），默认 8
    pub step: usize,
    /// 小样本自由度调整：dfk 时用 1/(T-m) 估计 Σ
    pub dfk: bool,
    /// varlmar 最大滞后阶数，默认 2
    pub mlag: usize,
    /// 与 Stata `varsoc` 一致：回归从全局时刻 `sample_start` 开始（0-based 行下标），`n_obs = T - sample_start`。
    /// `None` 时等于 `max(lags)`（原行为）。
    pub sample_start_offset: Option<usize>,
    /// 仅似然与信息准则，跳过 IRF/FEVD/诊断（`varsoc` 用）
    pub skip_extras: bool,
}

impl Default for VARConfig {
    fn default() -> Self {
        Self {
            constant: true,
            lags: vec![1, 2],
            step: 8,
            dfk: false,
            mlag: 2,
            sample_start_offset: None,
            skip_extras: false,
        }
    }
}

/// VAR 模型
pub struct VAR {
    /// 内生变量 Y (T × K)，每列一个变量
    pub y: Array2<f64>,
    /// 外生变量 X (T × M)，可选；缺失处可为 NaN（仅非回归行）
    pub exog: Option<Array2<f64>>,
    pub config: VARConfig,
    /// 变量名，用于系数标签
    pub var_names: Option<Vec<String>>,
    /// 外生变量名，用于系数标签
    pub exog_names: Option<Vec<String>>,
    /// 与 Stata `var …, exog()` 一致：`None` 时用连续样本 `t = sample_start … T−1`；
    /// `Some` 时仅在列出的全局行下标 `t` 上估计（要求该期 `exog[t]` 及所需 `y` 均有限）。
    pub regression_times: Option<Vec<usize>>,
}

/// 单方程估计结果（Stata 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAREquationStats {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

/// VAR 估计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARResult {
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub fpe: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub equations: Vec<VAREquationStats>,
    /// 系数：按方程分组，每方程 [L1.var1, L1.var2, ..., Lp.varK, const]
    pub coefficients: Vec<Vec<f64>>,
    pub std_errs: Vec<Vec<f64>>,
    pub z_values: Vec<Vec<f64>>,
    pub p_values: Vec<Vec<f64>>,
    pub ci_lower: Vec<Vec<f64>>,
    pub ci_upper: Vec<Vec<f64>>,
    /// 系数标签：如 ["dln_inv:L1.dln_inv", "dln_inv:L1.dln_inc", ...]
    pub coef_labels: Vec<Vec<String>>,
    /// 残差协方差矩阵 Σ (K×K)
    pub sigma: Vec<Vec<f64>>,
    /// 正交化 IRF: step+1 个 K×K 矩阵，[step][response][impulse]
    pub oirf: Vec<Vec<Vec<f64>>>,
    /// FEVD: step+1 个 K×K 矩阵，[step][response][impulse]
    pub fevd: Vec<Vec<Vec<f64>>>,
    /// varwle: Wald lag-exclusion 检验（Stata varwle 命令）
    pub varwle: Vec<VARWleRow>,
    /// varlmar: LM 残差自相关检验（Stata varlmar 命令）
    pub varlmar: Vec<VARLmarRow>,
    /// varstable: 特征值平稳性检验（Stata varstable 命令）
    pub varstable: Vec<VARStableRow>,
    /// vargranger: 格兰杰因果 Wald 检验（Stata vargranger 命令）
    pub vargranger: Vec<VARGrangerRow>,
}

/// varstable 单行：特征值及其模
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARStableRow {
    /// 特征值实部
    pub re: f64,
    /// 特征值虚部
    pub im: f64,
    /// 模 |λ|
    pub modulus: f64,
}

/// varlmar 单行：lag 阶的 LM 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARLmarRow {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// vargranger 单行：格兰杰因果 Wald 检验（Stata vargranger 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARGrangerRow {
    /// 方程（因变量）
    pub eq_name: String,
    /// 被排除的变量（或 "ALL"）
    pub excluded: String,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varwle 单行：某方程（或 All）在 lag 上的 Wald 检验
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARWleRow {
    pub eq_name: String,
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// `varsoc` 表一行（Stata `varsoc` 输出对应列）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARSocRow {
    pub lag: usize,
    pub log_likelihood: f64,
    /// 相对 VAR(p−1) 的 LR；lag 1 为 `None`
    pub lr: Option<f64>,
    pub lr_df: Option<usize>,
    pub lr_p: Option<f64>,
    pub fpe: f64,
    pub aic: f64,
    pub hqic: f64,
    pub sbic: f64,
}

/// `varsoc` 完整结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARSocResult {
    pub title: String,
    pub var_names: Vec<String>,
    pub maxlag: usize,
    pub num_observation: usize,
    pub rows: Vec<VARSocRow>,
}

