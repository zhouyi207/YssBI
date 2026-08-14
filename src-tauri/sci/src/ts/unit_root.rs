//! DF & ADF 单位根检验
//!
//! 参考 Stata dfuller: Δy_t = α + β*y_{t-1} + δ*t + ζ₁*Δy_{t-1} + ... + ζₖ*Δy_{t-k}
//! - noconstant: 无常数无趋势
//! - drift: 仅常数
//! - trend: 常数 + 时间趋势

use super::distributions::normal_cdf;
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::distribution::{ContinuousCDF, StudentsT};

/// 回归类型（对应 Stata dfuller 选项）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdfRegression {
    /// noconstant: 无常数无趋势
    NoConstant,
    /// drift: 仅常数
    Drift,
    /// trend: 常数 + 时间趋势
    Trend,
}

impl AdfRegression {
    pub fn from_flags(constant: bool, trend: bool) -> Self {
        match (constant, trend) {
            (false, _) => AdfRegression::NoConstant,
            (true, false) => AdfRegression::Drift,
            (true, true) => AdfRegression::Trend,
        }
    }
}

/// MacKinnon (1994) 响应面系数: c_α(T) = φ_∞ + φ_1/T + φ_2/T²
/// (phi_inf, phi_1, phi_2) for 1%, 5%, 10%
const MACKINNON_COEFFS: [[[f64; 3]; 3]; 3] = [
    // NoConstant
    [
        [-2.5658, -4.2389, -14.0],   // 1%
        [-1.9410, -2.9975, -7.2300], // 5%
        [-1.6168, -2.4985, -4.8850], // 10%
    ],
    // Drift
    [
        [-3.4304, -6.0773, -24.2350], // 1%
        [-2.8615, -3.5225, -6.6700],  // 5%
        [-2.5668, -2.6148, -4.4800],  // 10%
    ],
    // Trend
    [
        [-3.9634, -8.3534, -35.9670], // 1%
        [-3.4126, -4.3895, -10.8930], // 5%
        [-3.1279, -3.2982, -7.0000],  // 10%
    ],
];

/// MacKinnon (1994) p-value 近似：noconstant 与 trend 情形
/// 参考 statsmodels tsa.adfvalues.mackinnonp
/// polyval(coef[::-1], x) = coef[0]*x^0 + coef[1]*x^1 + ... (numpy 顺序)
fn mackinnon_pvalue(teststat: f64, reg: AdfRegression) -> f64 {
    // N=1 (ADF test), 使用 index 0
    // tau_smallp: 3 系数, polyval(coef[::-1], teststat) = c2 + c1*x + c0*x^2
    // tau_largep: 4 系数, polyval(coef[::-1], teststat) = c3 + c2*x + c1*x^2 + c0*x^3
    let (max_stat, min_stat, star_stat, smallp, largep) = match reg {
        AdfRegression::NoConstant => (
            f64::INFINITY,
            -19.04,
            -1.04,
            [0.6344, 1.2378, 0.032496],
            [0.4797, 0.93557, -0.06999, 0.033066],
        ),
        AdfRegression::Trend => (
            0.7,
            -16.18,
            -2.89,
            [3.2512, 1.6047, 0.049588],
            [2.5261, 0.61654, -0.37956, -0.060285],
        ),
        AdfRegression::Drift => return 0.0, // drift 用 t 分布，不在此调用
    };

    if teststat > max_stat {
        return 1.0;
    }
    if teststat < min_stat {
        return 0.0;
    }

    let x = if teststat <= star_stat {
        let c = &smallp;
        c[2] + c[1] * teststat + c[0] * teststat * teststat
    } else {
        let c = &largep;
        c[3] + c[2] * teststat + c[1] * teststat * teststat + c[0] * teststat * teststat * teststat
    };
    normal_cdf(x)
}

fn mackinnon_critical_value(reg: AdfRegression, n: usize, level_idx: usize) -> f64 {
    let reg_idx = match reg {
        AdfRegression::NoConstant => 0,
        AdfRegression::Drift => 1,
        AdfRegression::Trend => 2,
    };
    let [phi_inf, phi_1, phi_2] = MACKINNON_COEFFS[reg_idx][level_idx];
    let t = n as f64;
    phi_inf + phi_1 / t + phi_2 / (t * t)
}

/// 回归表单行
#[derive(Debug, Clone)]
pub struct AdfRegRow {
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub t: f64,
    pub p_value: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

/// ADF/DF 检验结果
#[derive(Debug, Clone)]
pub struct AdfResult {
    /// 检验统计量 (t-statistic on y_{t-1})
    pub test_statistic: f64,
    /// 1% 临界值
    pub critical_value_1pct: f64,
    /// 5% 临界值
    pub critical_value_5pct: f64,
    /// 10% 临界值
    pub critical_value_10pct: f64,
    /// p-value (drift 用 t 分布，其他用 MacKinnon 近似时可为 None)
    pub p_value: f64,
    /// 是否使用 t 分布临界值（drift 情形）
    pub use_t_distribution: bool,
    /// 有效观测数（回归用）
    pub num_obs: usize,
    /// 滞后阶数 (0=DF, >0=ADF)
    pub lags: usize,
    /// 回归类型
    pub regression: AdfRegression,
    /// 回归系数（含 y_{t-1} 的系数）
    pub coef_lagged: f64,
    /// 系数标准误
    pub std_err_lagged: f64,
    /// 完整回归表
    pub regression_table: Vec<AdfRegRow>,
}

/// Augmented Dickey-Fuller / Dickey-Fuller 单位根检验
///
/// 回归: Δy_t = α + β*y_{t-1} + δ*t + ζ₁*Δy_{t-1} + ... + ζₖ*Δy_{t-k}
///
/// * `y` - 原始序列
/// * `lags` - 滞后阶数，0 为 DF，>0 为 ADF
/// * `constant` - 是否含常数 (drift)
/// * `trend` - 是否含时间趋势
pub fn adf_test(y: &[f64], lags: usize, constant: bool, trend: bool) -> Result<AdfResult, String> {
    let n_raw = y.len();
    if n_raw < 4 {
        return Err("ADF: 至少需要 4 个观测值".to_string());
    }

    let reg = AdfRegression::from_flags(constant, trend);

    // Δy_t = y_t - y_{t-1}
    let dy: Vec<f64> = (1..n_raw).map(|i| y[i] - y[i - 1]).collect();
    let n_dy = dy.len();

    // 有效样本: 需要 y_{t-1} 和最多 lags 个 Δy_{t-j}，所以从 t = 1 + lags 开始
    let start = 1 + lags; // t=start 时，y_{t-1}=y[start-1] 存在，Δy_{t-1}..Δy_{t-lags} 都存在
    if start >= n_raw {
        return Err("ADF: 滞后阶数过大，有效样本不足".to_string());
    }

    let n = n_raw - start; // 有效观测数
    if n < 2 {
        return Err("ADF: 有效观测数不足".to_string());
    }

    // 因变量: Δy_t, t = start .. n_raw-1
    // dy 的索引: dy[i] = y[i+1]-y[i]，所以 dy[start-1] = y[start]-y[start-1] 对应 t=start
    let y_endog: Vec<f64> = (start - 1..n_dy.min(n_raw - 1)).map(|i| dy[i]).collect();
    let n_obs = y_endog.len();

    // 自变量: [const?, trend?, y_{t-1}, Δy_{t-1}, ..., Δy_{t-lags}]
    let ncols = {
        let mut k = 1; // y_{t-1}
        if constant {
            k += 1;
        }
        if trend {
            k += 1;
        }
        k += lags;
        k
    };

    // 按行填充（row-major）：每行 [const?, trend?, y_{t-1}, Δy_{t-1}, ..., Δy_{t-lags}]
    let mut exog_data = Vec::with_capacity(n_obs * ncols);

    for i in 0..n_obs {
        if constant {
            exog_data.push(1.0);
        }
        if trend {
            exog_data.push((start + i) as f64);
        }
        exog_data.push(y[start - 1 + i]);
        for j in 1..=lags {
            let idx = start - 1 + i - j;
            exog_data.push(if idx < n_dy { dy[idx] } else { 0.0 });
        }
    }
    let lagged_col = (if constant { 1 } else { 0 }) + (if trend { 1 } else { 0 });

    let exog = Array2::from_shape_vec((n_obs, ncols), exog_data)
        .map_err(|e| format!("ADF: 构建设计矩阵失败: {}", e))?;
    let endog = Array1::from_vec(y_endog);

    // OLS: β = (X'X)^{-1} X'y
    let x = exog.view().into_faer().to_owned();
    let y_col = endog.view().into_faer_col().to_owned();

    let xtx = x.transpose() * x.as_ref();
    let xty = x.transpose() * y_col.as_ref();

    let xtx_inv = xtx
        .llt(Side::Lower)
        .map_err(|_| "ADF: 设计矩阵秩不足".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));

    let betas = xtx_inv.as_ref() * xty.as_ref();
    let y_hat = x.as_ref() * betas.as_ref();
    let u = y_col.as_ref() - y_hat.as_ref();

    let rss: f64 = u.iter().map(|v| v * v).sum();
    let df_resid = (n_obs - ncols).max(1);
    let sigma2 = rss / df_resid as f64;

    let xtx_inv_nd = xtx_inv.as_ref().into_ndarray().to_owned();
    let cov_beta = sigma2 * &xtx_inv_nd;

    // y_{t-1} 的系数在列 lagged_col
    let betas_nd = betas.as_ref().into_ndarray().to_owned();
    let coef_lagged = betas_nd[lagged_col];
    let var_lagged = cov_beta[[lagged_col, lagged_col]];
    let std_err_lagged = if var_lagged > 0.0 {
        var_lagged.sqrt()
    } else {
        0.0
    };

    let test_statistic = if std_err_lagged > 1e-15 {
        coef_lagged / std_err_lagged
    } else {
        f64::NAN
    };

    // drift 情形用 t 分布临界值和 p-value（Stata 第三情形）
    let (cv_1, cv_5, cv_10, p_value, use_t_dist) = if reg == AdfRegression::Drift {
        let df = df_resid as f64;
        let t_dist =
            StudentsT::new(0.0, 1.0, df).unwrap_or_else(|_| StudentsT::new(0.0, 1.0, 1.0).unwrap());
        let cv_1 = t_dist.inverse_cdf(0.01);
        let cv_5 = t_dist.inverse_cdf(0.05);
        let cv_10 = t_dist.inverse_cdf(0.10);
        let p_val = t_dist.cdf(test_statistic);
        (cv_1, cv_5, cv_10, p_val, true)
    } else {
        let cv_1 = mackinnon_critical_value(reg, n_obs, 0);
        let cv_5 = mackinnon_critical_value(reg, n_obs, 1);
        let cv_10 = mackinnon_critical_value(reg, n_obs, 2);
        let p_val = mackinnon_pvalue(test_statistic, reg);
        (cv_1, cv_5, cv_10, p_val, false)
    };

    // 构建回归表（变量顺序与设计矩阵列一致）
    let mut reg_table = Vec::new();
    let mut col_names: Vec<String> = Vec::new();
    if constant {
        col_names.push("const".to_string());
    }
    if trend {
        col_names.push("trend".to_string());
    }
    col_names.push("L1.".to_string()); // y_{t-1}
    for j in 1..=lags {
        col_names.push(format!("L{}D.", j));
    }
    for (c, name) in col_names.iter().enumerate() {
        if c >= betas_nd.len() {
            break;
        }
        let coef = betas_nd[c];
        let se = cov_beta[[c, c]].sqrt().max(1e-15);
        let t_val = coef / se;
        let dist = StudentsT::new(0.0, 1.0, df_resid as f64)
            .unwrap_or_else(|_| StudentsT::new(0.0, 1.0, 1.0).unwrap());
        let p_val = 2.0 * (1.0 - dist.cdf(t_val.abs()));
        let t_crit = dist.inverse_cdf(0.975);
        let ci_lower = coef - t_crit * se;
        let ci_upper = coef + t_crit * se;
        reg_table.push(AdfRegRow {
            variable: name.clone(),
            coef,
            std_err: se,
            t: t_val,
            p_value: p_val,
            ci_lower,
            ci_upper,
        });
    }

    Ok(AdfResult {
        test_statistic,
        critical_value_1pct: cv_1,
        critical_value_5pct: cv_5,
        critical_value_10pct: cv_10,
        p_value,
        use_t_distribution: use_t_dist,
        num_obs: n_obs,
        lags,
        regression: reg,
        coef_lagged,
        std_err_lagged,
        regression_table: reg_table,
    })
}
