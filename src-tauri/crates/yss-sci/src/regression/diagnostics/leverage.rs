// ======================== Leverage (Stata predict lev, leverage) ========================
// 帽子矩阵 H = X(X'X)^{-1}X' 的对角元，即 leverage_i = H_ii

/// 计算 leverage（Stata `predict lev, leverage`）
/// x: (n × k) 设计矩阵
pub fn leverage(x: &Mat<f64>) -> Result<Vec<f64>, String> {
    let n = x.nrows();
    let k = x.ncols();
    if n == 0 || k == 0 {
        return Err("leverage: empty design matrix".to_string());
    }

    let x_matrix = x.as_ref().to_owned();
    let xtx = x_matrix.transpose() * x_matrix.as_ref();
    let xtx_inv = xtx
        .checked_cholesky()
        .map_err(|_| "leverage: X'X singular".to_string())?
        .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));

    // H = X (X'X)^{-1} X'，取对角元
    // H_ii = row_i(X) @ (X'X)^{-1} @ row_i(X)'
    let x_xtx_inv_nd = x_matrix.as_ref() * xtx_inv.as_ref(); // (n × k)
    let h_diag: Vec<f64> = (0..n)
        .map(|i| (0..k).map(|j| x_xtx_inv_nd[(i, j)] * x[(i, j)]).sum())
        .collect();
    Ok(h_diag)
}
