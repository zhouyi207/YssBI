/// 构建 White 检验的完整 Z 矩阵：X + 所有平方项 + 所有交叉项
/// Z = [X, x1², x2², ..., xp², x1*x2, x1*x3, ..., x_{p-1}*xp]
/// 其中 p = k-1 为非常数变量数（X 列 0 为常数）
fn build_white_z_full(x: &Mat<f64>, k: usize, p: usize, n: usize) -> Vec<Vec<f64>> {
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

/// 从列向量列表构建设计矩阵
fn build_z_matrix(z_cols: &[Vec<f64>], n: usize) -> Option<faer::Mat<f64>> {
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
    let z_arr = faer::MatRef::from_row_major_slice(&(z_raw), n, n_cols).to_owned();
    Some(z_arr.as_ref().to_owned())
}

/// White 异方差检验（estat imtest, white）
/// 对应 Stata: `estat imtest, white`
/// 辅助回归：û² 对 Z = [X, 平方项, 交叉项]；LM = n×R²，LM ~ χ²(df)
/// df = rank(Z) - 1，由 SVD 计算 rank，自动剔除共线变量（如哑变量 D²=D、D1*D2=0）
pub fn white_test(x: &Mat<f64>, residuals: &Col<f64>) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.nrows() != n {
        return Err(format!(
            "white_test: residuals length {} != n {}",
            residuals.nrows(),
            n
        ));
    }
    if k < 2 {
        return Err(
            "white_test: need at least 2 columns in X (constant + 1 regressor)".to_string(),
        );
    }

    let u_sq: Col<f64> = residuals.map(|&u| u * u);
    let p = k - 1;

    let z_cols = build_white_z_full(x, k, p, n);
    let n_cols = z_cols.len();
    if n <= n_cols {
        return Err(format!(
            "white_test: insufficient observations (n={}, need > {})",
            n, n_cols
        ));
    }

    let z_matrix = build_z_matrix(&z_cols, n)
        .ok_or_else(|| "white_test: failed to build Z matrix".to_string())?;

    let s_sv = z_matrix
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
    let svd = yss_linalg::Svd::factor(z_matrix
        .as_ref())
        .map_err(|e| format!("white_test: SVD failed: {:?}", e))?;
    let u = svd.left_vectors();
    let v = svd.right_vectors();
    let size = Ord::min(z_matrix.nrows(), z_matrix.ncols());
    let b_col = u_sq.as_ref().to_owned();
    // tmp = U' * b
    let tmp = u.submatrix(0, 0, u.nrows(), size).transpose() * b_col.as_ref();
    // tmp2[i] = tmp[i] / s[i] when s[i] > tol, else 0（避免除以零）
    let mut tmp2 = Mat::zeros(size, 1);
    let tmp_nd = tmp.as_ref().to_owned();
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp_nd[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    // gamma = V * tmp2
    let gamma = v.submatrix(0, 0, v.nrows(), size).as_ref() * tmp2.as_ref();
    let y_hat = z_matrix.as_ref() * gamma.as_ref();

    let y_hat_mat = y_hat.as_ref().to_owned();
    let y_hat_nd: Col<f64> = y_hat_mat.col(0).to_owned();
    let y_mean = u_sq.iter().sum::<f64>() / n as f64;
    let tss: f64 = u_sq.iter().map(|v| (v - y_mean).powi(2)).sum();
    let rss: f64 = u_sq
        .iter()
        .zip(y_hat_nd.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let lm_stat = n as f64 * r2;

    let chi2 = ChiSquared::new(df as f64).map_err(|e| format!("white_test: ChiSquared: {}", e))?;
    let p_value = 1.0 - chi2.cdf(lm_stat);

    Ok(BreuschPaganResult {
        lm_stat,
        df,
        p_value,
    })
}

/// White 异方差检验（加权版，WLS / aweight）
/// 对应 Stata: regress [aweight] 后 estat imtest, white
/// 辅助回归：w_i·û²_i = α₀ + Z_i·γ + v_i，其中 Z = [X, X², X_i·X_j]
/// 因变量 (√w_i·u_i)² = w_i·u_i²，与 OLS 的 û² 对应
/// LM = n×R²，df = rank(Z) - 1
pub fn white_test_weighted(
    x: &Mat<f64>,
    residuals: &Col<f64>,
    weights: &Col<f64>,
) -> Result<BreuschPaganResult, String> {
    let n = x.nrows();
    let k = x.ncols();
    if residuals.nrows() != n || weights.nrows() != n {
        return Err("white_test_weighted: length mismatch".to_string());
    }
    if k < 2 {
        return Err(
            "white_test_weighted: need at least 2 columns in X (constant + 1 regressor)"
                .to_string(),
        );
    }
    if weights.iter().any(|&w| w < 0.0) {
        return Err("white_test_weighted: weights must be non-negative".to_string());
    }

    // y = w_i·u_i²，辅助回归：y 对 Z（OLS）
    let y: Col<f64> = Col::from_fn(n, |i| weights[i] * residuals[i] * residuals[i]);
    let p = k - 1;

    let z_cols = build_white_z_full(x, k, p, n);
    let n_cols = z_cols.len();
    if n <= n_cols {
        return Err(format!(
            "white_test_weighted: insufficient observations (n={}, need > {})",
            n, n_cols
        ));
    }

    let z_matrix = build_z_matrix(&z_cols, n)
        .ok_or_else(|| "white_test_weighted: failed to build Z matrix".to_string())?;

    let s_sv = z_matrix
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

    let svd = yss_linalg::Svd::factor(z_matrix
        .as_ref())
        .map_err(|e| format!("white_test_weighted: SVD failed: {:?}", e))?;
    let u = svd.left_vectors();
    let v = svd.right_vectors();
    let size = Ord::min(z_matrix.nrows(), z_matrix.ncols());
    let b_col = y.as_ref().to_owned();
    let tmp = u.submatrix(0, 0, u.nrows(), size).transpose() * b_col.as_ref();
    let mut tmp2 = Mat::zeros(size, 1);
    let tmp_nd = tmp.as_ref().to_owned();
    for i in 0..size {
        let si = s_sv[i];
        let ti = tmp_nd[i];
        tmp2.as_mut()[(i, 0)] = if si > tol { ti / si } else { 0.0 };
    }
    let gamma = v.submatrix(0, 0, v.nrows(), size).as_ref() * tmp2.as_ref();
    let y_hat = z_matrix.as_ref() * gamma.as_ref();

    let y_hat_mat = y_hat.as_ref().to_owned();
    let y_hat_nd: Col<f64> = y_hat_mat.col(0).to_owned();

    let y_mean = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - y_mean).powi(2)).sum();
    let rss: f64 = y
        .iter()
        .zip(y_hat_nd.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

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
