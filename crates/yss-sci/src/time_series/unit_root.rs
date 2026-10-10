//! DF & ADF 单位根检验
//!
//! 参考 Stata dfuller: Δy_t = α + β*y_{t-1} + δ*t + ζ₁*Δy_{t-1} + ... + ζₖ*Δy_{t-k}
//! - noconstant: 无常数无趋势
//! - drift: 仅常数
//! - trend: 常数 + 时间趋势

use statrs::distribution::StudentsT;
use yss_sci_contract::time_series::forecast::Deterministic;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

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
    if lags >= n_raw - 1 {
        return Err("ADF: 滞后阶数过大，有效样本不足".to_string());
    }

    let reg = match (constant, trend) {
        (false, false) => AdfRegression::NoConstant,
        (true, false) => AdfRegression::Drift,
        (true, true) => AdfRegression::Trend,
        (false, true) => return Err("ADF: 时间趋势需要同时包含常数项".into()),
    };

    // 有效样本: 需要 y_{t-1} 和最多 lags 个 Δy_{t-j}，所以从 t = 1 + lags 开始
    let start = 1 + lags; // t=start 时，y_{t-1}=y[start-1] 存在，Δy_{t-1}..Δy_{t-lags} 都存在

    let n_obs = n_raw - start;
    let lagged_col = usize::from(constant) + usize::from(trend);
    let ncols = lagged_col + 1 + lags;
    if n_obs <= ncols {
        return Err("ADF: 回归剩余自由度不足".into());
    }
    let df_resid = n_obs - ncols;

    // dy[i] = Δy_{i+1}; the admitted lag window guarantees every index below exists.
    let dy: Vec<f64> = y.windows(2).map(|pair| pair[1] - pair[0]).collect();
    let y_col = Col::from_iter(dy[start - 1..].iter().copied());

    // 自变量: [const?, trend?, y_{t-1}, Δy_{t-1}, ..., Δy_{t-lags}]
    let x = Mat::from_fn(n_obs, ncols, |row, col| {
        if constant && col == 0 {
            1.0
        } else if col < lagged_col {
            (start + row) as f64
        } else if col == lagged_col {
            y[start - 1 + row]
        } else {
            dy[start - 1 + row - (col - lagged_col)]
        }
    });

    // OLS: β = (X'X)^{-1} X'y
    let xtx = x.transpose() * x.as_ref();
    let xty = x.transpose() * y_col.as_ref();

    let xtx_inv = xtx
        .checked_cholesky()
        .map_err(|_| "ADF: 设计矩阵秩不足".to_string())?
        .solve(&Mat::<f64>::identity(xtx.nrows(), xtx.nrows()));

    let betas = xtx_inv.as_ref() * xty.as_ref();
    let y_hat = x.as_ref() * betas.as_ref();
    let u = y_col.as_ref() - y_hat.as_ref();

    let rss: f64 = u.iter().map(|v| v * v).sum();
    let sigma2 = rss / df_resid as f64;

    let cov_beta = yss_sci_linalg::Scale(sigma2) * &xtx_inv;

    // y_{t-1} 的系数在列 lagged_col
    let coef_lagged = betas[lagged_col];
    let var_lagged = cov_beta[(lagged_col, lagged_col)];
    let std_err_lagged = var_lagged.sqrt();

    let test_statistic = coef_lagged / std_err_lagged;
    if !test_statistic.is_finite() {
        return Err("ADF: 单位根检验统计量无定义或非有限".into());
    }
    let t_dist =
        StudentsT::new(0.0, 1.0, df_resid as f64).map_err(|error| format!("ADF: {error}"))?;

    // drift 情形用 t 分布临界值和 p-value（Stata 第三情形）
    let (cv_1, cv_5, cv_10, p_value, use_t_dist) = if reg == AdfRegression::Drift {
        let critical = |tail| {
            crate::distribution::student_t::upper_quantile(tail, t_dist.freedom())
                .map(|q| -q)
                .ok_or_else(|| "ADF: Student-t critical value is undefined".to_string())
        };
        let cv_1 = critical(0.01)?;
        let cv_5 = critical(0.05)?;
        let cv_10 = critical(0.10)?;
        let p_val = crate::distribution::student_t_probability(
            &t_dist,
            test_statistic,
            yss_sci_contract::hypothesis::Alternative::Less,
        );
        (cv_1, cv_5, cv_10, p_val, true)
    } else {
        let cv_1 = mackinnon_critical_value(reg, n_obs, 0);
        let cv_5 = mackinnon_critical_value(reg, n_obs, 1);
        let cv_10 = mackinnon_critical_value(reg, n_obs, 2);
        let deterministic = match reg {
            AdfRegression::NoConstant => Deterministic::None,
            AdfRegression::Drift => Deterministic::Constant,
            AdfRegression::Trend => Deterministic::Trend,
        };
        let p_val = super::mackinnon::p_value(test_statistic, deterministic, 1);
        (cv_1, cv_5, cv_10, p_val, false)
    };

    // 构建回归表（变量顺序与设计矩阵列一致）
    let mut reg_table = Vec::with_capacity(ncols);
    let mut col_names: Vec<String> = Vec::with_capacity(ncols);
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
    let t_crit = crate::inference::intervals::critical(0.95, Some(t_dist.freedom()))
        .map_err(|error| format!("ADF t critical value: {error:?}"))?;
    for (c, name) in col_names.into_iter().enumerate() {
        let coef = betas[c];
        let se = cov_beta[(c, c)].sqrt();
        let t_val = coef / se;
        if !t_val.is_finite() {
            return Err("ADF: 辅助回归统计量无定义或非有限".into());
        }
        let p_val = crate::distribution::student_t_probability(
            &t_dist,
            t_val,
            yss_sci_contract::hypothesis::Alternative::TwoSided,
        );
        let ci_lower = coef - t_crit * se;
        let ci_upper = coef + t_crit * se;
        reg_table.push(AdfRegRow {
            variable: name,
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
