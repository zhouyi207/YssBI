//! 序列相关检验：Breusch-Godfrey, Ljung-Box Q, Durbin-Watson
//!
//! 参考 Stata: estat bgodfrey, wntestq, estat dwatson

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Side, linalg::solvers::Solve};
use ndarray::Array1;
use statrs::distribution::{ChiSquared, ContinuousCDF};

/// Durbin-Watson 统计量（Stata estat dwatson）
/// d = Σ(e_t - e_{t-1})² / Σ(e_t)²
pub fn durbin_watson(residuals: &[f64]) -> f64 {
    let n = residuals.len();
    if n < 2 {
        return 2.0;
    }
    let sum_sq_diff: f64 = (1..n)
        .map(|t| (residuals[t] - residuals[t - 1]).powi(2))
        .sum();
    let sum_sq: f64 = residuals.iter().map(|e| e.powi(2)).sum();
    if sum_sq <= 0.0 {
        return 2.0;
    }
    sum_sq_diff / sum_sq
}

/// Ljung-Box Q 统计量（Stata wntestq, lags(p)）
/// Q = n(n+2) Σ_{k=1}^h ρ̂_k² / (n-k) ~ χ²(h)
pub fn ljung_box_q(residuals: &[f64], lags: usize) -> Option<(f64, f64)> {
    let n = residuals.len();
    if n < 4 || lags < 1 {
        return None;
    }
    let mean: f64 = residuals.iter().sum::<f64>() / n as f64;
    let var: f64 = residuals.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    if var <= 0.0 {
        return None;
    }
    let h = lags.min(n - 1);
    let mut q = 0.0;
    for k in 1..=h {
        let mut sum = 0.0;
        for t in k..n {
            sum += (residuals[t] - mean) * (residuals[t - k] - mean);
        }
        let rho_k = sum / var;
        q += rho_k.powi(2) / (n - k) as f64;
    }
    let q_stat = n as f64 * (n as f64 + 2.0) * q;
    let dist = ChiSquared::new(h as f64).ok()?;
    let p_value = 1.0 - dist.cdf(q_stat);
    Some((q_stat, p_value))
}

/// Breusch-Godfrey LM 检验（Stata estat bgodfrey, lags(p)）
/// 辅助回归: u_t = X_t·γ + ρ_1·u_{t-1} + ... + ρ_p·u_{t-p}
/// TR² ~ χ²(p)
///
/// * `nomiss0`: true = 缺失的滞后残差用 0 填充，保留 n 个观测（Stata nomiss0）
///              false = 去掉前 p 个观测，仅用 n-p 个观测做辅助回归
pub fn breusch_godfrey(
    residuals: &[f64],
    exog: &[Vec<f64>],
    lags: usize,
    nomiss0: bool,
) -> Option<(f64, f64)> {
    let n = residuals.len();
    let k = exog.get(0).map(|r| r.len()).unwrap_or(0);
    if n < 4 || k == 0 || lags < 1 || exog.len() != n {
        return None;
    }
    let p = lags.min(n - 1).max(1);

    let (n_aux, z_data, y_data) = if nomiss0 {
        // nomiss0: 保留 n 个观测，u_{t-j}=0 当 t<j
        let ncols = p + k;
        let mut z_data = vec![0.0; n * ncols];
        for t in 0..n {
            let row_start = t * ncols;
            for j in 1..=p {
                let lag_val = if t >= j { residuals[t - j] } else { 0.0 };
                z_data[row_start + j - 1] = lag_val;
            }
            for (c, &v) in exog[t].iter().enumerate() {
                z_data[row_start + p + c] = v;
            }
        }
        let y_data: Vec<f64> = residuals.to_vec();
        (n, z_data, y_data)
    } else {
        // 去掉前 p 个观测，仅用 t=p..n-1
        let n_aux = n - p;
        if n_aux <= k + p {
            return None;
        }
        let ncols = p + k;
        let mut z_data = vec![0.0; n_aux * ncols];
        let mut y_data = Vec::with_capacity(n_aux);
        for (i, t) in (p..n).enumerate() {
            y_data.push(residuals[t]);
            let row_start = i * ncols;
            for j in 1..=p {
                z_data[row_start + j - 1] = residuals[t - j];
            }
            for (c, &v) in exog[t].iter().enumerate() {
                z_data[row_start + p + c] = v;
            }
        }
        (n_aux, z_data, y_data)
    };

    let z_arr = ndarray::Array2::from_shape_vec((n_aux, p + k), z_data).ok()?;
    let y_arr = Array1::from_vec(y_data);

    let z_faer = z_arr.view().into_faer().to_owned();
    let y_col = y_arr.view().into_faer_col().to_owned();

    let ztz = z_faer.as_ref().transpose() * z_faer.as_ref();
    let zty = z_faer.as_ref().transpose() * y_col.as_ref();

    let ztz_llt = ztz.llt(Side::Lower).ok()?;
    let gamma = ztz_llt.solve(zty.as_ref());
    let y_hat = z_faer.as_ref() * gamma.as_ref();
    let y_hat_nd = y_hat.as_ref().into_ndarray().to_owned();
    let y_hat_vec: Vec<f64> = y_hat_nd.iter().copied().collect();

    let rss: f64 = y_arr
        .iter()
        .zip(y_hat_vec.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>();
    let tss: f64 = y_arr.iter().map(|v| v.powi(2)).sum();
    let r2 = if tss > 1e-20 { 1.0 - rss / tss } else { 0.0 };
    let lm_stat = n_aux as f64 * r2;

    let dist = ChiSquared::new(p as f64).ok()?;
    let p_value = 1.0 - dist.cdf(lm_stat);

    Some((lm_stat, p_value))
}
