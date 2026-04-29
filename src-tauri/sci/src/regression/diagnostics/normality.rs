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

