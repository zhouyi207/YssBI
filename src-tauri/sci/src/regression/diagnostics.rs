//! 回归诊断：BP 检验（异方差检验）等
//!
//! 实现 Stata `estat hettest` 的四种变体：
//! - `estat hettest`：z = 拟合值，原始 BP 公式
//! - `estat hettest, rhs`：z = RHS 变量，原始 BP 公式
//! - `estat hettest, iid`：z = 拟合值，Koenker 形式 (LM = n×R²)
//! - `estat hettest, rhs iid`：z = RHS 变量，Koenker 形式

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF};

/// Breusch-Pagan 异方差检验结果
#[derive(Debug, Clone)]
pub struct BreuschPaganResult {
    /// LM 统计量
    pub lm_stat: f64,
    /// 自由度
    pub df: usize,
    /// p 值 (H0: 同方差)
    pub p_value: f64,
}

/// 辅助函数：给定 z 矩阵，计算原始 BP 统计量 LM = (1/2)·ESS
fn bp_stat_stata(g: &Array1<f64>, z_faer: &faer::Mat<f64>) -> Result<(f64, usize), String> {
    let m = z_faer.ncols();
    let g_col = g.view().into_faer_col().to_owned();

    let ztz = z_faer.as_ref().transpose() * z_faer.as_ref();
    let ztg = z_faer.as_ref().transpose() * g_col.as_ref();

    let ztz_inv = ztz
        .llt(Side::Lower)
        .map_err(|_| "BP: Z'Z singular in auxiliary regression".to_string())?
        .solve(Mat::identity(ztz.nrows(), ztz.ncols()));
    let gamma = ztz_inv.as_ref() * ztg;
    let g_hat = z_faer.as_ref() * gamma.as_ref();

    let g_mean = 1.0;
    let tss: f64 = g_col.as_ref().iter().map(|v| (v - g_mean).powi(2)).sum();
    let rss: f64 = g_col
        .as_ref()
        .iter()
        .zip(g_hat.as_ref().iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    let ess = tss - rss;

    let lm_stat = 0.5 * ess;
    let df = m.saturating_sub(1);
    Ok((lm_stat, df))
}

/// 辅助函数：给定 z 矩阵，计算 Koenker 统计量 LM = n×R²（假定 g 均值为 1）
fn bp_stat_koenker(g: &Array1<f64>, z_faer: &faer::Mat<f64>) -> Result<(f64, usize), String> {
    let n = g.len();
    let m = z_faer.ncols();
    let g_col = g.view().into_faer_col().to_owned();

    let ztz = z_faer.as_ref().transpose() * z_faer.as_ref();
    let ztg = z_faer.as_ref().transpose() * g_col.as_ref();

    let ztz_inv = ztz
        .llt(Side::Lower)
        .map_err(|_| "BP: Z'Z singular in auxiliary regression".to_string())?
        .solve(Mat::identity(ztz.nrows(), ztz.ncols()));
    let gamma = ztz_inv.as_ref() * ztg;
    let g_hat = z_faer.as_ref() * gamma.as_ref();

    let g_mean = 1.0;
    let tss: f64 = g_col.as_ref().iter().map(|v| (v - g_mean).powi(2)).sum();
    let rss: f64 = g_col
        .as_ref()
        .iter()
        .zip(g_hat.as_ref().iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let lm_stat = n as f64 * r2;
    let df = m.saturating_sub(1);
    Ok((lm_stat, df))
}

/// Breusch-Pagan 检验（Stata 原始公式，z = RHS 变量）
/// 对应 Stata: `estat hettest, rhs`
/// 辅助回归：g_i = u²_i/σ̂² 对 X；LM = (1/2)·ESS，LM ~ χ²(k-1)
pub fn breusch_pagan_stata_rhs(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n {
        return Err(format!(
            "breusch_pagan_stata_rhs: residuals length {} != n {}",
            residuals.len(),
            n
        ));
    }
    if n < k + 2 {
        return Err("breusch_pagan_stata_rhs: insufficient observations".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let sigma2 = u_sq.iter().sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_stata_rhs: residual variance is zero".to_string());
    }
    let g: Array1<f64> = u_sq.mapv(|v| v / sigma2);

    let z_faer = x.view().into_faer().to_owned();
    let (lm_stat, df) = bp_stat_stata(&g, &z_faer)?;

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_stata_rhs: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 检验（Koenker 形式，z = RHS 变量）
/// 对应 Stata: `estat hettest, rhs iid`
/// 辅助回归：g_i = u²_i/σ̂² 对 X；LM = n×R²，LM ~ χ²(k-1)，不假定正态性
pub fn breusch_pagan_koenker_rhs(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n {
        return Err(format!(
            "breusch_pagan_koenker_rhs: residuals length {} != n {}",
            residuals.len(),
            n
        ));
    }
    if n < k + 2 {
        return Err("breusch_pagan_koenker_rhs: insufficient observations".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let sigma2 = u_sq.iter().sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_koenker_rhs: residual variance is zero".to_string());
    }
    let g: Array1<f64> = u_sq.mapv(|v| v / sigma2);

    let z_faer = x.view().into_faer().to_owned();
    let (lm_stat, df) = bp_stat_koenker(&g, &z_faer)?;

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_koenker_rhs: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// 构建 z = [1, fitted_values] 矩阵
fn build_z_fitted(n: usize, fitted: &Array1<f64>) -> faer::Mat<f64> {
    let mut z_raw = Vec::with_capacity(n * 2);
    for i in 0..n {
        z_raw.push(1.0);
        z_raw.push(fitted[i]);
    }
    let z_arr = Array2::from_shape_vec((n, 2), z_raw).expect("build_z_fitted: shape");
    z_arr.view().into_faer().to_owned()
}

/// Breusch-Pagan 检验（Stata 原始公式，z = 拟合值）
/// 对应 Stata: `estat hettest`
/// 辅助回归：g_i = u²_i/σ̂² 对 [1, ŷ]；LM = (1/2)·ESS，LM ~ χ²(1)
pub fn breusch_pagan_stata(
    residuals: &Array1<f64>,
    fitted_values: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = residuals.len();
    if fitted_values.len() != n {
        return Err(format!(
            "breusch_pagan_stata: fitted_values length {} != n {}",
            fitted_values.len(),
            n
        ));
    }
    if n < 4 {
        return Err("breusch_pagan_stata: insufficient observations".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let sigma2 = u_sq.iter().sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_stata: residual variance is zero".to_string());
    }
    let g: Array1<f64> = u_sq.mapv(|v| v / sigma2);

    let z_faer = build_z_fitted(n, fitted_values);
    let (lm_stat, df) = bp_stat_stata(&g, &z_faer)?;

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_stata: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 检验（Koenker 形式，z = 拟合值）
/// 对应 Stata: `estat hettest, iid`
/// 辅助回归：g_i = u²_i/σ̂² 对 [1, ŷ]；LM = n×R²，LM ~ χ²(1)，不假定正态性
pub fn breusch_pagan_koenker(
    residuals: &Array1<f64>,
    fitted_values: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = residuals.len();
    if fitted_values.len() != n {
        return Err(format!(
            "breusch_pagan_koenker: fitted_values length {} != n {}",
            fitted_values.len(),
            n
        ));
    }
    if n < 4 {
        return Err("breusch_pagan_koenker: insufficient observations".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let sigma2 = u_sq.iter().sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_koenker: residual variance is zero".to_string());
    }
    let g: Array1<f64> = u_sq.mapv(|v| v / sigma2);

    let z_faer = build_z_fitted(n, fitted_values);
    let (lm_stat, df) = bp_stat_koenker(&g, &z_faer)?;

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_koenker: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// 构建 White 检验的完整 Z 矩阵：X + 所有平方项 + 所有交叉项
/// Z = [X, x1², x2², ..., xp², x1*x2, x1*x3, ..., x_{p-1}*xp]
/// 其中 p = k-1 为非常数变量数（X 列 0 为常数）
fn build_white_z_full(x: &Array2<f64>, k: usize, p: usize, n: usize) -> Vec<Vec<f64>> {
    let mut z_cols: Vec<Vec<f64>> = Vec::new();
    for c in 0..k {
        z_cols.push((0..n).map(|i| x[(i, c)]).collect());
    }
    for j in 1..=p {
        z_cols.push((0..n).map(|i| x[(i, j)] * x[(i, j)]).collect());
    }
    for j in 1..=p {
        for m in (j + 1)..=p {
            z_cols.push((0..n).map(|i| x[(i, j)] * x[(i, m)]).collect());
        }
    }
    z_cols
}

/// 从列向量列表构建 faer 矩阵
fn build_z_faer(z_cols: &[Vec<f64>], n: usize) -> Option<faer::Mat<f64>> {
    let n_cols = z_cols.len();
    if n_cols == 0 {
        return None;
    }
    let mut z_raw = Vec::with_capacity(n * n_cols);
    for i in 0..n {
        for col in z_cols {
            z_raw.push(col[i]);
        }
    }
    let z_arr = Array2::from_shape_vec((n, n_cols), z_raw).ok()?;
    Some(z_arr.view().into_faer().to_owned())
}

/// White 异方差检验（estat imtest, white）
/// 对应 Stata: `estat imtest, white`
/// 辅助回归：û² 对 Z = [X, 平方项, 交叉项]；LM = n×R²，LM ~ χ²(df)
/// df = rank(Z) - 1，由 SVD 计算 rank，自动剔除共线变量（如哑变量 D²=D、D1*D2=0）
pub fn white_test(x: &Array2<f64>, residuals: &Array1<f64>) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n {
        return Err(format!(
            "white_test: residuals length {} != n {}",
            residuals.len(),
            n
        ));
    }
    if k < 2 {
        return Err("white_test: need at least 2 columns in X (constant + 1 regressor)".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let p = k - 1;

    let z_cols = build_white_z_full(x, k, p, n);
    let n_cols = z_cols.len();
    if n <= n_cols {
        return Err(format!(
            "white_test: insufficient observations (n={}, need > {})",
            n,
            n_cols
        ));
    }

    let z_faer = build_z_faer(&z_cols, n)
        .ok_or_else(|| "white_test: failed to build Z matrix".to_string())?;

    let s_sv = z_faer
        .as_ref()
        .singular_values()
        .map_err(|e| format!("white_test: SVD failed: {:?}", e))?;
    let max_sv = s_sv.first().copied().unwrap_or(0.0);
    let tol = 1e-10_f64 * max_sv.max(1e-10);
    let rank = s_sv.iter().filter(|v| **v > tol).count();
    let df = rank.saturating_sub(1);

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    // 秩不足时 QR 会除以近零对角元产生 NaN，改用 SVD 并截断小奇异值
    let svd = z_faer
        .as_ref()
        .svd()
        .map_err(|e| format!("white_test: SVD failed: {:?}", e))?;
    let u = svd.U();
    let v = svd.V();
    let size = Ord::min(z_faer.nrows(), z_faer.ncols());
    let b_col = u_sq.view().into_faer_col().to_owned();
    // tmp = U' * b
    let tmp = u.get(.., ..size).transpose() * b_col.as_ref();
    // tmp2[i] = tmp[i] / s[i] when s[i] > tol, else 0（避免除以零）
    let mut tmp2 = Mat::zeros(size, 1);
    let tmp_nd = tmp.as_ref().into_ndarray().to_owned();
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp_nd[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    // gamma = V * tmp2
    let gamma = v.get(.., ..size) * tmp2.as_ref();
    let y_hat = z_faer.as_ref() * gamma;

    let y_hat_mat = y_hat.as_ref().into_ndarray().to_owned();
    let y_hat_nd: Array1<f64> = y_hat_mat.column(0).into_owned();
    let y_mean = u_sq.iter().sum::<f64>() / n as f64;
    let tss: f64 = u_sq.iter().map(|v| (v - y_mean).powi(2)).sum();
    let rss: f64 = u_sq
        .iter()
        .zip(y_hat_nd.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let lm_stat = n as f64 * r2;

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("white_test: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 检验（向后兼容别名）
/// 等价于 `breusch_pagan_stata_rhs`，对应 Stata `estat hettest, rhs`
#[inline(always)]
pub fn breusch_pagan(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    breusch_pagan_stata_rhs(x, residuals)
}

/// White 异方差检验（加权版，WLS / aweight）
/// 对应 Stata: regress [aweight] 后 estat imtest, white
/// 辅助回归：w_i·û²_i = α₀ + Z_i·γ + v_i，其中 Z = [X, X², X_i·X_j]
/// 因变量 (√w_i·u_i)² = w_i·u_i²，与 OLS 的 û² 对应
/// LM = n×R²，df = rank(Z) - 1
pub fn white_test_weighted(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n || weights.len() != n {
        return Err("white_test_weighted: length mismatch".to_string());
    }
    if k < 2 {
        return Err("white_test_weighted: need at least 2 columns in X (constant + 1 regressor)".to_string());
    }
    if weights.iter().any(|&w| w < 0.0) {
        return Err("white_test_weighted: weights must be non-negative".to_string());
    }

    // y = w_i·u_i²，辅助回归：y 对 Z（OLS）
    let y: Array1<f64> = Array1::from_shape_fn(n, |i| weights[i] * residuals[i] * residuals[i]);
    let p = k - 1;

    let z_cols = build_white_z_full(x, k, p, n);
    let n_cols = z_cols.len();
    if n <= n_cols {
        return Err(format!(
            "white_test_weighted: insufficient observations (n={}, need > {})",
            n,
            n_cols
        ));
    }

    let z_faer = build_z_faer(&z_cols, n)
        .ok_or_else(|| "white_test_weighted: failed to build Z matrix".to_string())?;

    let s_sv = z_faer
        .as_ref()
        .singular_values()
        .map_err(|e| format!("white_test_weighted: SVD failed: {:?}", e))?;
    let max_sv = s_sv.first().copied().unwrap_or(0.0);
    let tol = 1e-10_f64 * max_sv.max(1e-10);
    let rank = s_sv.iter().filter(|v| **v > tol).count();
    let df = rank.saturating_sub(1);

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let svd = z_faer
        .as_ref()
        .svd()
        .map_err(|e| format!("white_test_weighted: SVD failed: {:?}", e))?;
    let u = svd.U();
    let v = svd.V();
    let size = Ord::min(z_faer.nrows(), z_faer.ncols());
    let b_col = y.view().into_faer_col().to_owned();
    let tmp = u.get(.., ..size).transpose() * b_col.as_ref();
    let mut tmp2 = Mat::zeros(size, 1);
    let tmp_nd = tmp.as_ref().into_ndarray().to_owned();
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp_nd[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    let gamma = v.get(.., ..size) * tmp2.as_ref();
    let y_hat = z_faer.as_ref() * gamma;

    let y_hat_mat = y_hat.as_ref().into_ndarray().to_owned();
    let y_hat_nd: Array1<f64> = y_hat_mat.column(0).into_owned();

    let y_mean = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - y_mean).powi(2)).sum();
    let rss: f64 = y.iter().zip(y_hat_nd.iter()).map(|(a, b)| (a - b).powi(2)).sum();

    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let lm_stat = n as f64 * r2;

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("white_test_weighted: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// χ² 检验结果（chi2, df, p_value），用于 IM-test 各分量
#[derive(Debug, Clone)]
pub struct Chi2TestResult {
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// Cameron & Trivedi (1990) IM-test 分解：Heteroskedasticity + Skewness + Kurtosis
/// 对应 Stata: `estat imtest`
#[derive(Debug, Clone)]
pub struct ImTestResult {
    pub heteroskedasticity: Chi2TestResult,
    pub skewness: Chi2TestResult,
    pub kurtosis: Chi2TestResult,
    pub total: Chi2TestResult,
}

/// 辅助回归 y 对 Z，LM = n×(1 - RSS/USS)，df = rank(Z) - 1
fn lm_chi2_aux(y: &Array1<f64>, z_faer: &faer::Mat<f64>) -> Result<(f64, usize), String> {
    let n = y.len();
    let uss: f64 = y.iter().map(|v| v * v).sum();
    if uss <= 0.0 {
        return Ok((0.0, 0));
    }
    let s_sv = z_faer
        .as_ref()
        .singular_values()
        .map_err(|e| format!("lm_chi2_aux: SVD failed: {:?}", e))?;
    let max_sv = s_sv.first().copied().unwrap_or(0.0);
    let tol = 1e-10_f64 * max_sv.max(1e-10);
    let rank = s_sv.iter().filter(|v| **v > tol).count();
    let df = rank.saturating_sub(1);
    if df == 0 {
        return Ok((0.0, 0));
    }
    let svd = z_faer
        .as_ref()
        .svd()
        .map_err(|e| format!("lm_chi2_aux: SVD failed: {:?}", e))?;
    let u = svd.U();
    let v = svd.V();
    let size = Ord::min(z_faer.nrows(), z_faer.ncols());
    let b_col = y.view().into_faer_col().to_owned();
    let tmp = u.get(.., ..size).transpose() * b_col.as_ref();
    let mut tmp2 = Mat::zeros(size, 1);
    let tmp_nd = tmp.as_ref().into_ndarray().to_owned();
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp_nd[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    let gamma = v.get(.., ..size) * tmp2.as_ref();
    let y_hat = z_faer.as_ref() * gamma;
    let y_hat_mat = y_hat.as_ref().into_ndarray().to_owned();
    let y_hat_nd: Array1<f64> = y_hat_mat.column(0).into_owned();
    let rss: f64 = y.iter().zip(y_hat_nd.iter()).map(|(a, b)| (a - b).powi(2)).sum();
    let chi2 = n as f64 * (1.0 - rss / uss);
    Ok((chi2, df))
}

/// Cameron & Trivedi (1990) 信息矩阵检验分解
/// 对应 Stata: `estat imtest`
pub fn im_test(x: &Array2<f64>, residuals: &Array1<f64>) -> Result<ImTestResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n {
        return Err(format!("im_test: residuals length {} != n {}", residuals.len(), n));
    }
    if k < 2 {
        return Err("im_test: need at least 2 columns in X (constant + 1 regressor)".to_string());
    }

    let s2 = residuals.iter().map(|u| u * u).sum::<f64>() / n as f64;

    // Heteroskedasticity: White 检验
    let hetero = white_test(x, residuals)?;

    // Skewness: y_s = u³ - 3·σ²·u，对 X 回归
    let y_s: Array1<f64> = Array1::from_shape_fn(n, |i| {
        let u = residuals[i];
        u * u * u - 3.0 * s2 * u
    });
    let x_faer = x.view().into_faer().to_owned();
    let (chi2_s, df_s) = lm_chi2_aux(&y_s, &x_faer)?;
    let chi2_skew = ChiSquared::new(df_s as f64)
        .map_err(|e| format!("im_test skewness: ChiSquared: {}", e))?;
    let p_s = if df_s > 0 {
        1.0 - chi2_skew.cdf(chi2_s)
    } else {
        1.0
    };

    // Kurtosis: y_k = u⁴ - 6·σ²·u² + 3·σ⁴，对常数回归，df=1（Cameron-Trivedi 固定）
    let y_k: Array1<f64> = Array1::from_shape_fn(n, |i| {
        let u = residuals[i];
        let u2 = u * u;
        u2 * u2 - 6.0 * s2 * u2 + 3.0 * s2 * s2
    });
    let uss_k: f64 = y_k.iter().map(|v| v * v).sum();
    let y_k_mean = y_k.iter().sum::<f64>() / n as f64;
    let rss_k: f64 = y_k.iter().map(|v| (v - y_k_mean).powi(2)).sum();
    let chi2_k = if uss_k > 0.0 {
        n as f64 * (1.0 - rss_k / uss_k)
    } else {
        0.0
    };
    let df_k = 1;
    let chi2_kurt = ChiSquared::new(df_k as f64)
        .map_err(|e| format!("im_test kurtosis: ChiSquared: {}", e))?;
    let p_k = 1.0 - chi2_kurt.cdf(chi2_k);

    let total_chi2 = hetero.lm_stat + chi2_s + chi2_k;
    let total_df = hetero.df + df_s + df_k;
    let chi2_total = ChiSquared::new(total_df as f64)
        .map_err(|e| format!("im_test total: ChiSquared: {}", e))?;
    let total_p_value = 1.0 - chi2_total.cdf(total_chi2);

    Ok(ImTestResult {
        heteroskedasticity: Chi2TestResult {
            chi2: hetero.lm_stat,
            df: hetero.df,
            p_value: hetero.p_value,
        },
        skewness: Chi2TestResult {
            chi2: chi2_s,
            df: df_s,
            p_value: p_s,
        },
        kurtosis: Chi2TestResult {
            chi2: chi2_k,
            df: df_k,
            p_value: p_k,
        },
        total: Chi2TestResult {
            chi2: total_chi2,
            df: total_df,
            p_value: total_p_value,
        },
    })
}

/// Cameron & Trivedi (1990) IM-test 分解（WLS 加权版）
/// 异方差用 white_test_weighted，偏度与峰度同 OLS（无标准加权形式）
pub fn im_test_weighted(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<ImTestResult, String> {
    let hetero = white_test_weighted(x, residuals, weights)?;
    let mut base = im_test(x, residuals)?;
    base.heteroskedasticity = Chi2TestResult {
        chi2: hetero.lm_stat,
        df: hetero.df,
        p_value: hetero.p_value,
    };
    let total_chi2 = base.heteroskedasticity.chi2 + base.skewness.chi2 + base.kurtosis.chi2;
    let total_df = base.heteroskedasticity.df + base.skewness.df + base.kurtosis.df;
    let chi2_total = ChiSquared::new(total_df as f64)
        .map_err(|e| format!("im_test_weighted total: ChiSquared: {}", e))?;
    base.total = Chi2TestResult {
        chi2: total_chi2,
        df: total_df,
        p_value: 1.0 - chi2_total.cdf(total_chi2),
    };
    Ok(base)
}

// ========== 残差正态性检验（Omnibus / Jarque-Bera，statsmodels 风格）==========
// 基于样本偏度 S 和峰度 K 的矩检验，与 ImTest（回归式 LM 检验）互补

/// 残差正态性检验结果（Omnibus + Jarque-Bera）
#[derive(Debug, Clone)]
pub struct NormalityTestResult {
    pub skewness: f64,
    pub kurtosis: f64,
    pub omnibus_stat: f64,
    pub omnibus_p_value: f64,
    pub jarque_bera_stat: f64,
    pub jarque_bera_p_value: f64,
}

/// 计算样本偏度 g1 和峰度（raw K，正态时 K=3）
fn sample_skewness_kurtosis(x: &Array1<f64>) -> (f64, f64) {
    let n = x.len() as f64;
    if n < 4.0 {
        return (0.0, 3.0);
    }
    let mean = x.iter().sum::<f64>() / n;
    let m2 = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    if m2 <= 0.0 {
        return (0.0, 3.0);
    }
    let m3 = x.iter().map(|v| (v - mean).powi(3)).sum::<f64>() / n;
    let m4 = x.iter().map(|v| (v - mean).powi(4)).sum::<f64>() / n;
    let g1 = m3 / m2.powf(1.5);
    let kurtosis_raw = m4 / (m2 * m2);
    (g1, kurtosis_raw)
}

/// scipy.stats.skewtest 的 Z 统计量（与 statsmodels/scipy 完全一致）
fn scipy_skewtest_z(g1: f64, n: f64) -> f64 {
    if n < 8.0 {
        return 0.0;
    }
    let mu2 = 6.0 * (n - 2.0) / ((n + 1.0) * (n + 3.0));
    let y = g1 * (((n + 1.0) * (n + 3.0)) / (6.0 * (n - 2.0))).sqrt();
    let y = if y == 0.0 { 1.0 } else { y };
    let beta2 = (3.0 * (n * n + 27.0 * n - 70.0) * (n + 1.0) * (n + 3.0))
        / ((n - 2.0) * (n + 5.0) * (n + 7.0) * (n + 9.0));
    if beta2 <= 1.0 {
        return g1 / mu2.sqrt();
    }
    let w2 = -1.0 + (2.0 * (beta2 - 1.0)).sqrt();
    if w2 <= 1.0 {
        return g1 / mu2.sqrt();
    }
    let delta = 1.0 / (0.5 * w2.ln()).sqrt();
    let alpha = (2.0 / (w2 - 1.0)).sqrt();
    let ya = y / alpha;
    delta * (ya + (ya * ya + 1.0).sqrt()).ln()
}

/// scipy.stats.kurtosistest 的 Z 统计量（Anscombe-Glynn，与 statsmodels/scipy 完全一致）
/// 输入为 raw kurtosis b2 = m4/m2²（Pearson 定义，正态时 b2≈3）
fn scipy_kurtosistest_z(b2: f64, n: f64) -> f64 {
    if n < 5.0 {
        return 0.0;
    }
    let e = 3.0 * (n - 1.0) / (n + 1.0);
    let varb2 = 24.0 * n * (n - 2.0) * (n - 3.0)
        / ((n + 1.0).powi(2) * (n + 3.0) * (n + 5.0));
    let x = (b2 - e) / varb2.sqrt();
    let sqrtbeta1 = 6.0 * (n * n - 5.0 * n + 2.0) / ((n + 7.0) * (n + 9.0))
        * (6.0 * (n + 3.0) * (n + 5.0) / (n * (n - 2.0) * (n - 3.0))).sqrt();
    if sqrtbeta1.abs() < 1e-14 {
        return x;
    }
    let a = 6.0
        + 8.0 / sqrtbeta1
            * (2.0 / sqrtbeta1 + (1.0 + 4.0 / (sqrtbeta1 * sqrtbeta1)).sqrt());
    if a <= 4.0 {
        return x;
    }
    let term1 = 1.0 - 2.0 / (9.0 * a);
    let denom = 1.0 + x * (2.0 / (a - 4.0)).sqrt();
    if denom.abs() < 1e-300 {
        return x;
    }
    let ratio = (1.0 - 2.0 / a) / denom.abs();
    let term2 = denom.signum() * ratio.powf(1.0 / 3.0);
    (term1 - term2) / (2.0 / (9.0 * a)).sqrt()
}

/// 残差正态性检验：Omnibus (D'Agostino-Pearson) + Jarque-Bera
/// 与 statsmodels OLS summary 的 Omnibus / JB 一致
pub fn normality_tests(residuals: &Array1<f64>) -> Result<NormalityTestResult, String> {
    let n = residuals.len();
    if n < 8 {
        return Err("normality_tests: need at least 8 observations".to_string());
    }
    let n_f = n as f64;
    let (g1, kurtosis_raw) = sample_skewness_kurtosis(residuals);

    // Jarque-Bera: JB = n/6 * (S² + (K-3)²/4), K 为 raw kurtosis
    let jb = n_f / 6.0 * (g1 * g1 + (kurtosis_raw - 3.0) * (kurtosis_raw - 3.0) / 4.0);
    let chi2_jb = ChiSquared::new(2.0).map_err(|e| format!("normality_tests JB: {}", e))?;
    let jb_p = 1.0 - chi2_jb.cdf(jb);

    // Omnibus: scipy normaltest = skewtest² + kurtosistest²（与 statsmodels 完全一致）
    let z_skew = scipy_skewtest_z(g1, n_f);
    let z_kurt = scipy_kurtosistest_z(kurtosis_raw, n_f);
    let omnibus_stat = z_skew * z_skew + z_kurt * z_kurt;
    let chi2_om = ChiSquared::new(2.0).map_err(|e| format!("normality_tests Omnibus: {}", e))?;
    let omnibus_p = 1.0 - chi2_om.cdf(omnibus_stat);

    Ok(NormalityTestResult {
        skewness: g1,
        kurtosis: kurtosis_raw,
        omnibus_stat,
        omnibus_p_value: omnibus_p,
        jarque_bera_stat: jb,
        jarque_bera_p_value: jb_p,
    })
}

// ========== 加权版本（WLS / aweight，与 R lmtest::bptest、Stata 一致）==========
// R bptest: sigma2 = sum(w*e²)/n, f = e²/sigma2 - 1, 加权回归 f 对 Z, LM = 0.5*sum(w*f_hat²)
// Stata aweight: 先归一化 w = (N/sum(v))*v

/// 加权辅助回归的 R²（用于 Koenker：LM = n×R²）
fn weighted_aux_r2(z: &faer::Mat<f64>, y: &Array1<f64>, w: &Array1<f64>) -> Result<f64, String> {
    let fitted = weighted_aux_regression(z, y, w)?;
    let n = y.len();
    let sum_w: f64 = w.iter().sum();
    let y_mean_w = if sum_w > 0.0 {
        (0..n).map(|i| w[i] * y[i]).sum::<f64>() / sum_w
    } else {
        0.0
    };
    let tss: f64 = (0..n).map(|i| w[i] * (y[i] - y_mean_w).powi(2)).sum();
    let rss: f64 = (0..n).map(|i| w[i] * (y[i] - fitted[i]).powi(2)).sum();
    Ok(if tss > 0.0 { 1.0 - rss / tss } else { 0.0 })
}

/// 加权辅助回归：min Σ w_i*(y_i - Z_i*γ)²，返回 fitted
fn weighted_aux_regression(
    z: &faer::Mat<f64>,
    y: &Array1<f64>,
    w: &Array1<f64>,
) -> Result<Array1<f64>, String> {
    let n = y.len();
    let m = z.ncols();

    // Z'WZ, Z'Wy (ztwy as m×1 matrix for solve)
    let mut ztwz: faer::Mat<f64> = faer::Mat::zeros(m, m);
    let mut ztwy: faer::Mat<f64> = faer::Mat::zeros(m, 1);
    for i in 0..n {
        let wi = w[i];
        for c in 0..m {
            ztwy[(c, 0)] += wi * z[(i, c)] * y[i];
            for r in 0..m {
                ztwz[(r, c)] += wi * z[(i, r)] * z[(i, c)];
            }
        }
    }

    let ztwz_inv = ztwz
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "BP weighted: Z'WZ singular".to_string())?
        .solve(Mat::identity(m, m));
    let gamma = ztwz_inv.as_ref() * ztwy.as_ref();
    let mut fitted = Array1::zeros(n);
    for i in 0..n {
        fitted[i] = (0..m).map(|c| z[(i, c)] * gamma[(c, 0)]).sum();
    }
    Ok(fitted)
}

/// Breusch-Pagan 加权版（原始公式，z = RHS）
/// 对应 R bptest(, studentize=FALSE), Stata regress [aweight] 后 estat hettest, rhs
pub fn breusch_pagan_stata_rhs_weighted(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n || weights.len() != n {
        return Err("breusch_pagan_stata_rhs_weighted: length mismatch".to_string());
    }
    if n < k + 2 {
        return Err("breusch_pagan_stata_rhs_weighted: insufficient observations".to_string());
    }

    let sigma2 = (0..n).map(|i| weights[i] * residuals[i] * residuals[i]).sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_stata_rhs_weighted: residual variance is zero".to_string());
    }
    let f: Array1<f64> = Array1::from_shape_fn(n, |i| residuals[i] * residuals[i] / sigma2 - 1.0);

    let z_faer = x.view().into_faer().to_owned();
    let f_hat = weighted_aux_regression(&z_faer, &f, weights)?;

    let lm_stat = 0.5 * (0..n).map(|i| weights[i] * f_hat[i] * f_hat[i]).sum::<f64>();
    let df = k.saturating_sub(1);

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_stata_rhs_weighted: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 加权版（Koenker 形式，z = RHS）
/// Stata estat hettest, rhs iid：g = e²/σ̂²，加权回归 g 对 Z，LM = n×R²
pub fn breusch_pagan_koenker_rhs_weighted(
    x: &Array2<f64>,
    residuals: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n || weights.len() != n {
        return Err("breusch_pagan_koenker_rhs_weighted: length mismatch".to_string());
    }
    if n < k + 2 {
        return Err("breusch_pagan_koenker_rhs_weighted: insufficient observations".to_string());
    }

    let sigma2 = (0..n).map(|i| weights[i] * residuals[i] * residuals[i]).sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_koenker_rhs_weighted: residual variance is zero".to_string());
    }
    let g: Array1<f64> = Array1::from_shape_fn(n, |i| residuals[i] * residuals[i] / sigma2);

    let z_faer = x.view().into_faer().to_owned();
    let r2 = weighted_aux_r2(&z_faer, &g, weights)?;
    let lm_stat = n as f64 * r2;
    let df = k.saturating_sub(1);

    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_koenker_rhs_weighted: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 加权版（原始公式，z = 拟合值）
pub fn breusch_pagan_stata_weighted(
    residuals: &Array1<f64>,
    fitted_values: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = residuals.len();
    if fitted_values.len() != n || weights.len() != n {
        return Err("breusch_pagan_stata_weighted: length mismatch".to_string());
    }
    if n < 4 {
        return Err("breusch_pagan_stata_weighted: insufficient observations".to_string());
    }

    let sigma2 = (0..n).map(|i| weights[i] * residuals[i] * residuals[i]).sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_stata_weighted: residual variance is zero".to_string());
    }
    let f: Array1<f64> = Array1::from_shape_fn(n, |i| residuals[i] * residuals[i] / sigma2 - 1.0);

    let z_faer = build_z_fitted(n, fitted_values);
    let f_hat = weighted_aux_regression(&z_faer, &f, weights)?;

    let lm_stat = 0.5 * (0..n).map(|i| weights[i] * f_hat[i] * f_hat[i]).sum::<f64>();
    let df = 1;

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_stata_weighted: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// Breusch-Pagan 加权版（Koenker 形式，z = 拟合值）
/// Stata estat hettest, iid：g = e²/σ̂²，加权回归 g 对 [1, ŷ]，LM = n×R²
pub fn breusch_pagan_koenker_weighted(
    residuals: &Array1<f64>,
    fitted_values: &Array1<f64>,
    weights: &Array1<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = residuals.len();
    if fitted_values.len() != n || weights.len() != n {
        return Err("breusch_pagan_koenker_weighted: length mismatch".to_string());
    }
    if n < 4 {
        return Err("breusch_pagan_koenker_weighted: insufficient observations".to_string());
    }

    let sigma2 = (0..n).map(|i| weights[i] * residuals[i] * residuals[i]).sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan_koenker_weighted: residual variance is zero".to_string());
    }
    let g: Array1<f64> = Array1::from_shape_fn(n, |i| residuals[i] * residuals[i] / sigma2);

    let z_faer = build_z_fitted(n, fitted_values);
    let r2 = weighted_aux_r2(&z_faer, &g, weights)?;
    let lm_stat = n as f64 * r2;
    let df = 1;

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan_koenker_weighted: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}
