// ======================== Ramsey RESET 检验 ========================
// 对应 Stata estat ovtest 和 estat ovtest, rhs
// Ramsey (1969) regression specification-error test for omitted variables

/// RESET 检验结果（F 检验）
#[derive(Debug, Clone)]
pub struct ResetTestResult {
    pub f_stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

/// 将变量归一化到 [0,1]：x_norm = (x - min) / (max - min)，若 max==min 则置 0
fn normalize_min_max(v: &[f64]) -> Vec<f64> {
    let (min, max) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
            (a.min(x), b.max(x))
        });
    let range = max - min;
    if range <= 0.0 {
        return vec![0.0; v.len()];
    }
    v.iter().map(|&x| (x - min) / range).collect()
}

/// 辅助：带权重的 F 检验（受限 vs 非受限回归）
/// y = [X | Z] * [β; γ]，检验 γ = 0
/// weights: None = OLS，Some(w) = WLS
fn f_test_restricted_unrestricted(
    y: &Array1<f64>,
    x_restricted: &Array2<f64>,
    z_augment: &Array2<f64>,
    weights: Option<&Array1<f64>>,
) -> Result<ResetTestResult, String> {
    let n = y.len();
    let k = x_restricted.ncols();
    let q = z_augment.ncols();
    if n < k + q + 1 {
        return Err("RESET: insufficient observations for auxiliary regression".to_string());
    }

    let (rss_r, rss_u, df_resid_u) = if let Some(w) = weights {
        let sqrt_w: Array1<f64> = w.mapv(|v| v.sqrt());
        let y_w: Array1<f64> = y.iter().zip(sqrt_w.iter()).map(|(a, b)| a * b).collect();
        let x_r_w: Array2<f64> = Array2::from_shape_fn((n, k), |(i, j)| x_restricted[[i, j]] * sqrt_w[i]);
        let mut x_u_w = x_r_w.clone();
        for i in 0..n {
            for j in 0..q {
                x_u_w[[i, k + j]] = z_augment[[i, j]] * sqrt_w[i];
            }
        }
        let rss_r = ols_rss(&y_w, &x_r_w)?;
        let rss_u = ols_rss(&y_w, &x_u_w)?;
        let df_resid_u = n - k - q;
        (rss_r, rss_u, df_resid_u)
    } else {
        let rss_r = ols_rss(y, x_restricted)?;
        let mut x_u = Array2::zeros((n, k + q));
        x_u.slice_mut(ndarray::s![.., ..k]).assign(x_restricted);
        x_u.slice_mut(ndarray::s![.., k..]).assign(z_augment);
        let rss_u = ols_rss(y, &x_u)?;
        let df_resid_u = n - k - q;
        (rss_r, rss_u, df_resid_u)
    };

    let f_stat = if df_resid_u > 0 && rss_u > 1e-300 {
        ((rss_r - rss_u) / q as f64) / (rss_u / df_resid_u as f64)
    } else {
        0.0
    };

    let dist = FisherSnedecor::new(q as f64, df_resid_u as f64)
        .map_err(|e| format!("RESET: FisherSnedecor: {}", e))?;
    let p_value = 1.0 - dist.cdf(f_stat);

    Ok(ResetTestResult {
        f_stat,
        df1: q,
        df2: df_resid_u,
        p_value,
    })
}

fn ols_rss(y: &Array1<f64>, x: &Array2<f64>) -> Result<f64, String> {
    let y_col = y.view().into_faer_col().to_owned();
    let x_faer = x.view().into_faer().to_owned();
    let xtx = x_faer.as_ref().transpose() * x_faer.as_ref();
    let xty = x_faer.as_ref().transpose() * y_col.as_ref();
    let xtx_inv = xtx
        .llt(Side::Lower)
        .map_err(|_| "RESET: X'X singular in auxiliary regression".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
    let beta = xtx_inv.as_ref() * xty.as_ref();
    let y_hat = x_faer.as_ref() * beta.as_ref();
    let rss: f64 = y_col
        .as_ref()
        .iter()
        .zip(y_hat.as_ref().iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    Ok(rss)
}

/// RESET 检验（默认：基于拟合值幂）
/// 对应 Stata: estat ovtest
/// 辅助回归：y = Xβ + γ₂ŷ² + γ₃ŷ³ + γ₄ŷ⁴，检验 γ=0
/// ŷ 归一化到 [0,1] 后计算幂
pub fn reset_test(
    y: &Array1<f64>,
    x: &Array2<f64>,
    fitted: &Array1<f64>,
    weights: Option<&Array1<f64>>,
) -> Result<ResetTestResult, String> {
    let n = y.len();
    let fitted_vec: Vec<f64> = fitted.iter().cloned().collect();
    let fitted_norm = normalize_min_max(&fitted_vec);
    let mut z = Array2::zeros((n, 3));
    for i in 0..n {
        let f = fitted_norm[i];
        z[[i, 0]] = f * f;
        z[[i, 1]] = f * f * f;
        z[[i, 2]] = f * f * f * f;
    }
    f_test_restricted_unrestricted(y, x, &z, weights)
}

/// RESET 检验（rhs：基于 RHS 变量幂）
/// 对应 Stata: estat ovtest, rhs
/// 辅助回归：y = Xβ + powers(2..4) of RHS vars，检验新增系数=0
/// 排除二值变量（min=0, max=1 的列）
pub fn reset_test_rhs(
    y: &Array1<f64>,
    x: &Array2<f64>,
    weights: Option<&Array1<f64>>,
) -> Result<ResetTestResult, String> {
    let n = x.nrows();
    let mut z_cols: Vec<Vec<f64>> = Vec::new();
    for j in 0..x.ncols() {
        let col: Vec<f64> = (0..n).map(|i| x[[i, j]]).collect();
        let (min, max) = col.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
        if (max - min) < 1e-10 {
            continue;
        }
        let is_binary = col.iter().all(|&v| v.abs() < 1e-10 || (v - 1.0).abs() < 1e-10);
        if is_binary {
            continue;
        }
        let norm = normalize_min_max(&col);
        for p in 2..=4 {
            let pow_col: Vec<f64> = norm.iter().map(|v| v.powi(p)).collect();
            z_cols.push(pow_col);
        }
    }
    if z_cols.is_empty() {
        return Err("RESET rhs: no non-dummy RHS variables for powers".to_string());
    }
    let q = z_cols.len();
    let mut z = Array2::zeros((n, q));
    for (j, col) in z_cols.iter().enumerate() {
        for i in 0..n {
            z[[i, j]] = col[i];
        }
    }
    f_test_restricted_unrestricted(y, x, &z, weights)
}

