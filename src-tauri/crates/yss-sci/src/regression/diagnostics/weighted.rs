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

