use super::{
    breusch_pagan::BreuschPaganResult,
    white::{white_test, white_test_weighted},
};
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_linalg::{Col, Mat};

/// χ² 检验结果（chi2, df, p_value），用于 IM-test 各分量
pub use yss_sci_contract::diagnostics::residual::Chi2TestResult;

/// Cameron & Trivedi (1990) IM-test 分解：Heteroskedasticity + Skewness + Kurtosis
/// 对应 Stata: `estat imtest`
pub use yss_sci_contract::diagnostics::residual::ImTestResult;

/// 辅助回归 y 对 Z，LM = n×(1 - RSS/USS)，df = rank(Z) - 1
fn lm_chi2_aux(y: &Col<f64>, z_matrix: &yss_sci_linalg::Mat<f64>) -> Result<(f64, usize), String> {
    let n = y.nrows();
    let uss: f64 = y.iter().map(|v| v * v).sum();
    if uss <= 0.0 {
        return Ok((0.0, 0));
    }
    let svd = yss_sci_linalg::Svd::factor(z_matrix.as_ref())
        .map_err(|e| format!("lm_chi2_aux: SVD failed: {:?}", e))?;
    let s_sv = svd.values();
    let max_sv = s_sv.iter().next().copied().unwrap_or(0.0);
    let tol = 1e-10_f64 * max_sv.max(1e-10);
    let rank = s_sv.iter().filter(|v| **v > tol).count();
    let df = rank.saturating_sub(1);
    if df == 0 {
        return Ok((0.0, 0));
    }
    let u = svd.left_vectors();
    let v = svd.right_vectors();
    let size = Ord::min(z_matrix.nrows(), z_matrix.ncols());
    let tmp = u.submatrix(0, 0, u.nrows(), size).transpose() * y.as_ref();
    let mut tmp2 = Mat::zeros(size, 1);
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    let gamma = v.submatrix(0, 0, v.nrows(), size).as_ref() * tmp2.as_ref();
    let y_hat = z_matrix.as_ref() * gamma.as_ref();
    let y_hat_nd = y_hat.col(0);
    let rss: f64 = y
        .iter()
        .zip(y_hat_nd.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    let chi2 = n as f64 * (1.0 - rss / uss);
    Ok((chi2, df))
}

/// Cameron & Trivedi (1990) 信息矩阵检验分解
/// 对应 Stata: `estat imtest`
pub fn im_test(x: &Mat<f64>, residuals: &Col<f64>) -> Result<ImTestResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.nrows() != n {
        return Err(format!(
            "im_test: residuals length {} != n {}",
            residuals.nrows(),
            n
        ));
    }
    if k < 2 {
        return Err("im_test: need at least 2 columns in X (constant + 1 regressor)".to_string());
    }

    // Heteroskedasticity: White 检验
    let hetero = white_test(x, residuals)?;
    im_test_from_white(x, residuals, hetero)
}

fn im_test_from_white(
    x: &Mat<f64>,
    residuals: &Col<f64>,
    hetero: BreuschPaganResult,
) -> Result<ImTestResult, String> {
    let n = x.nrows();
    let s2 = residuals.iter().map(|u| u * u).sum::<f64>() / n as f64;

    // Skewness: y_s = u³ - 3·σ²·u，对 X 回归
    let y_s: Col<f64> = Col::from_fn(n, |i| {
        let u = residuals[i];
        u * u * u - 3.0 * s2 * u
    });
    let (chi2_s, df_s) = lm_chi2_aux(&y_s, x)?;
    let chi2_skew =
        ChiSquared::new(df_s as f64).map_err(|e| format!("im_test skewness: ChiSquared: {}", e))?;
    let p_s = if df_s > 0 { chi2_skew.sf(chi2_s) } else { 1.0 };

    // Kurtosis: y_k = u⁴ - 6·σ²·u² + 3·σ⁴，对常数回归，df=1（Cameron-Trivedi 固定）
    let y_k: Col<f64> = Col::from_fn(n, |i| {
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
    let chi2_kurt =
        ChiSquared::new(df_k as f64).map_err(|e| format!("im_test kurtosis: ChiSquared: {}", e))?;
    let p_k = chi2_kurt.sf(chi2_k);

    let total_chi2 = hetero.lm_stat + chi2_s + chi2_k;
    let total_df = hetero.df + df_s + df_k;
    let chi2_total = ChiSquared::new(total_df as f64)
        .map_err(|e| format!("im_test total: ChiSquared: {}", e))?;
    let total_p_value = chi2_total.sf(total_chi2);

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
    x: &Mat<f64>,
    residuals: &Col<f64>,
    weights: &Col<f64>,
) -> Result<ImTestResult, String> {
    let hetero = white_test_weighted(x, residuals, weights)?;
    im_test_from_white(x, residuals, hetero)
}
