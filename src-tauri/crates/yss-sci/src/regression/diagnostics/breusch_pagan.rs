// 回归诊断：BP 检验（异方差检验）等
//
// 实现 Stata `estat hettest` 的四种变体：
// - `estat hettest`：z = 拟合值，原始 BP 公式
// - `estat hettest, rhs`：z = RHS 变量，原始 BP 公式
// - `estat hettest, iid`：z = 拟合值，Koenker 形式 (LM = n×R²)
// - `estat hettest, rhs iid`：z = RHS 变量，Koenker 形式

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};

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
    let mut z = faer::Mat::zeros(n, 2);
    for i in 0..n {
        z[(i, 0)] = 1.0;
        z[(i, 1)] = fitted[i];
    }
    z
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

