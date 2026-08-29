// ======================== VIF 多重共线性检验 ========================
// 对应 Stata estat vif
// VIF_c(x_j) = 1/(1-R²_j)，R²_j 为 x_j 对其他解释变量回归的 R²

/// 单变量 VIF 结果
#[derive(Debug, Clone)]
pub struct VifEntry {
    pub vif: f64,
    pub tolerance: f64, // 1/VIF
}

/// 计算 centered VIF（Stata 默认）
/// x: (n × k) 设计矩阵，列顺序与变量对应
/// has_constant: 若 true，第 0 列为常数项，不计算其 VIF
/// 完美共线时用 1e99 代替 INFINITY，避免 serde_json 序列化失败
pub fn vif_centered(x: &Array2<f64>, has_constant: bool) -> Result<Vec<VifEntry>, String> {
    let n = x.nrows();
    let k = x.ncols();
    if n < k + 2 {
        return Err("VIF: insufficient observations".to_string());
    }

    let mut result = Vec::with_capacity(k);
    for j in 0..k {
        if has_constant && j == 0 {
            result.push(VifEntry {
                vif: f64::NAN,
                tolerance: f64::NAN,
            });
            continue;
        }

        let mut x_other = Vec::with_capacity(n * (k - 1));
        for i in 0..n {
            for jj in 0..k {
                if jj != j {
                    x_other.push(x[[i, jj]]);
                }
            }
        }
        let x_other = Array2::from_shape_vec((n, k - 1), x_other)
            .map_err(|e| format!("VIF: x_other shape: {}", e))?;

        let y_j: Array1<f64> = (0..n).map(|i| x[[i, j]]).collect();

        let r2 = r2_centered_aux(&y_j, &x_other)?;
        let (vif, tol) = if r2 >= 1.0 - 1e-10 {
            (1e99, 0.0)
        } else {
            let v = 1.0 / (1.0 - r2);
            (v, 1.0 / v)
        };
        result.push(VifEntry { vif, tolerance: tol });
    }
    Ok(result)
}

/// 辅助回归 R²（Stata centered）：y 对 X 的 OLS，R² = 1 - RSS/TSS
/// X 含常数项时不做中心化，否则常数列会变为 0 导致 X'X 奇异
fn r2_centered_aux(y: &Array1<f64>, x: &Array2<f64>) -> Result<f64, String> {
    let n = x.nrows();
    let y_mean = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - y_mean).powi(2)).sum();
    if tss < 1e-300 {
        return Ok(0.0);
    }

    let y_col = y.view().into_faer_col().to_owned();
    let x_faer = x.view().into_faer().to_owned();
    let xtx = x_faer.as_ref().transpose() * x_faer.as_ref();
    let xty = x_faer.as_ref().transpose() * y_col.as_ref();
    let xtx_inv = xtx
        .llt(Side::Lower)
        .map_err(|_| "VIF: X'X singular in auxiliary regression".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
    let beta = xtx_inv.as_ref() * xty.as_ref();
    let y_hat = x_faer.as_ref() * beta.as_ref();

    let rss: f64 = y_col
        .as_ref()
        .iter()
        .zip(y_hat.as_ref().iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    let r2 = 1.0 - rss / tss;
    Ok(r2)
}

