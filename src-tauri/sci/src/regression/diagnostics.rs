//! 回归诊断：BP 检验（异方差检验）等
//!
//! 采用原始 Breusch-Pagan 公式以匹配 Stata 的 estat hettest：
//! g_i = u²_i / σ̂²，σ̂² = Σu²/n，辅助回归 g 对 X，LM = (1/2)·ESS，ESS = TSS - RSS

use crate::tools::{IntoFaer, IntoFaerCol};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF};

/// Breusch-Pagan 异方差检验结果
#[derive(Debug, Clone)]
pub struct BreuschPaganResult {
    /// LM 统计量
    pub lm_stat: f64,
    /// 自由度 (k-1)
    pub df: usize,
    /// p 值 (H0: 同方差)
    pub p_value: f64,
}

/// Breusch-Pagan 检验（原始公式，与 Stata hettest 一致）
/// 要求原回归含常数项，调用方在无常数时应跳过此检验
/// - x: 设计矩阵 (n × k)，含常数项
/// - residuals: OLS 残差 (n,)
/// 步骤：g_i = u²_i/σ̂²，σ̂²=Σu²/n；辅助回归 g 对 X；LM = (1/2)·(TSS-RSS)，LM ~ χ²(k-1)
pub fn breusch_pagan(x: &Array2<f64>, residuals: &Array1<f64>) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.len() != n {
        return Err(format!(
            "breusch_pagan: residuals length {} != n {}",
            residuals.len(),
            n
        ));
    }
    if n < k + 2 {
        return Err("breusch_pagan: insufficient observations".to_string());
    }

    let u_sq: Array1<f64> = residuals.mapv(|u| u * u);
    let sigma2 = u_sq.iter().sum::<f64>() / n as f64;
    if sigma2 <= 0.0 {
        return Err("breusch_pagan: residual variance is zero".to_string());
    }
    let g: Array1<f64> = u_sq.mapv(|v| v / sigma2);

    let x_faer = x.view().into_faer().to_owned();
    let g_col = g.view().into_faer_col().to_owned();

    let xtx = x_faer.as_ref().transpose() * x_faer.as_ref();
    let xtg = x_faer.as_ref().transpose() * g_col.as_ref();

    let xtx_inv = xtx
        .llt(Side::Lower)
        .map_err(|_| "breusch_pagan: X'X singular in auxiliary regression".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
    let gamma = xtx_inv.as_ref() * xtg;
    let g_hat = x_faer.as_ref() * gamma.as_ref();

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
    let df = k.saturating_sub(1);
    if df == 0 {
        return Ok(BreuschPaganResult {
            lm_stat: 0.0,
            df: 0,
            p_value: 1.0,
        });
    }

    let chi2 = ChiSquared::new(df as f64)
        .map_err(|e| format!("breusch_pagan: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}
