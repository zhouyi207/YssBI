//! ACF (Autocorrelation) and PACF (Partial Autocorrelation)
//!
//! 参考 Stata ac / pac 命令：样本自相关与偏自相关，用于残差诊断。

/// 计算样本自相关 ACF
///
/// 返回 lag 0..=max_lag，其中 lag 0 恒为 1.0。
/// 公式: ρ̂(k) = Σ(y_t - ȳ)(y_{t-k} - ȳ) / Σ(y_t - ȳ)²
pub fn acf(x: &[f64], max_lag: usize) -> Vec<f64> {
    let n = x.len();
    if n < 2 || max_lag == 0 {
        return vec![1.0];
    }
    let mean: f64 = x.iter().sum::<f64>() / n as f64;
    let var: f64 = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
    if var <= 0.0 {
        return vec![1.0];
    }
    let max_lag = max_lag.min(n - 1);
    let mut acf_vals = Vec::with_capacity(max_lag + 1);
    acf_vals.push(1.0); // lag 0
    for k in 1..=max_lag {
        let mut sum = 0.0;
        for t in k..n {
            sum += (x[t] - mean) * (x[t - k] - mean);
        }
        acf_vals.push(sum / var);
    }
    acf_vals
}

/// 计算偏自相关 PACF（Durbin-Levinson 递推）
///
/// 返回 lag 1..=max_lag（PACF 无 lag 0）。
/// 与 Stata pac 一致，使用 Yule-Walker / Durbin-Levinson。
pub fn pacf(x: &[f64], max_lag: usize) -> Vec<f64> {
    let acf_vals = acf(x, max_lag);
    if acf_vals.len() <= 1 {
        return vec![];
    }
    let rho: Vec<f64> = acf_vals[1..].to_vec(); // ρ_1, ρ_2, ...
    let n_lag = rho.len();
    if n_lag == 0 {
        return vec![];
    }
    let mut pacf_out = Vec::with_capacity(n_lag);
    let mut phi_prev = vec![rho[0]]; // φ_{1,1} = ρ_1
    pacf_out.push(rho[0]);
    for k in 2..=n_lag {
        let mut num = rho[k - 1];
        let mut den = 1.0;
        for j in 1..k {
            num -= phi_prev[j - 1] * rho[k - 1 - j];
            den -= phi_prev[j - 1] * rho[j - 1];
        }
        let phi_kk = if den.abs() < 1e-15 { 0.0 } else { num / den };
        pacf_out.push(phi_kk);
        let mut phi_curr = vec![0.0; k];
        for j in 1..k {
            phi_curr[j - 1] = phi_prev[j - 1] - phi_kk * phi_prev[k - 1 - j];
        }
        phi_curr[k - 1] = phi_kk;
        phi_prev = phi_curr;
    }
    pacf_out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acf_white_noise() {
        let x: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();
        let a = acf(&x, 10);
        assert_eq!(a.len(), 11);
        assert!((a[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pacf_len() {
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let p = pacf(&x, 5);
        assert_eq!(p.len(), 5);
    }
}
