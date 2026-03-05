//! OLS 协方差矩阵计算
//! 支持 nonrobust, HC0, HC1, HC2, HC3, fixed scale, cluster 等

use ndarray::{Array1, Array2};

/// 协方差计算所需的额外参数（cluster、HAC、fixed scale 等）
#[derive(Debug, Clone)]
pub enum CovParams {
    FixedScale { scale: f64 },
    Cluster { cluster_id: Vec<usize> },
    HAC { kernel: String, bandwidth: Option<i64> },
    HacPanel { entity_id: Vec<usize>, time_id: Vec<usize> },
    HacGroupsum { group_id: Vec<usize> },
}

/// 计算参数协方差矩阵 cov_beta
/// - x: (n × k) 设计矩阵
/// - xtx_inv: (X'X)⁻¹
/// - u: (n,) 残差向量
/// - df_residual: n - k
pub fn compute_cov_beta(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    df_residual: usize,
    cov_type: &str,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let n = x.nrows();
    let k = x.ncols();

    match cov_type {
        "nonrobust" => cov_nonrobust(xtx_inv, u, df_residual),
        "fixed scale" => cov_fixed_scale(xtx_inv, cov_params),
        "HC0" => cov_hc0(x, xtx_inv, u, n, k),
        "HC1" => cov_hc1(x, xtx_inv, u, n, k, df_residual),
        "HC2" => cov_hc2(x, xtx_inv, u, n, k, df_residual),
        "HC3" => cov_hc3(x, xtx_inv, u, n, k, df_residual),
        "cluster" => cov_cluster(x, xtx_inv, u, cov_params),
        "HAC" | "hac-panel" | "hac-groupsum" => {
            Err(format!("cov_type '{}' not yet implemented", cov_type))
        }
        _ => cov_nonrobust(xtx_inv, u, df_residual),
    }
}

fn cov_nonrobust(xtx_inv: &Array2<f64>, u: &Array1<f64>, df_residual: usize) -> Result<Array2<f64>, String> {
    let sigma2 = u.dot(u) / df_residual as f64;
    Ok(sigma2 * xtx_inv)
}

/// Fixed scale: scale * (X'X)⁻¹，scale 由用户通过 Config 指定
fn cov_fixed_scale(
    xtx_inv: &Array2<f64>,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let scale = match cov_params {
        Some(CovParams::FixedScale { scale }) => *scale,
        _ => return Err("fixed scale cov_type requires CovParams::FixedScale with scale".to_string()),
    };
    if scale <= 0.0 {
        return Err("fixed scale: scale must be positive".to_string());
    }
    Ok(scale * xtx_inv)
}

/// HC0: (X'X)⁻¹ X' diag(u²) X (X'X)⁻¹
fn cov_hc0(x: &Array2<f64>, xtx_inv: &Array2<f64>, u: &Array1<f64>, n: usize, k: usize) -> Result<Array2<f64>, String> {
    let mut meat = Array2::zeros((k, k));
    for i in 0..n {
        let u2 = u[i] * u[i];
        let xi = x.row(i);
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += u2 * xi[r] * xi[c];
            }
        }
    }
    let sandwich = xtx_inv.dot(&meat).dot(xtx_inv);
    Ok(sandwich)
}

/// HC1: HC0 × n / (n - k)
fn cov_hc1(x: &Array2<f64>, xtx_inv: &Array2<f64>, u: &Array1<f64>, n: usize, k: usize, df_residual: usize) -> Result<Array2<f64>, String> {
    let hc0 = cov_hc0(x, xtx_inv, u, n, k)?;
    let scale = n as f64 / df_residual as f64;
    Ok(scale * hc0)
}

/// HC2: 权重 w_i = 1 / (1 - h_ii)
fn cov_hc2(x: &Array2<f64>, xtx_inv: &Array2<f64>, u: &Array1<f64>, n: usize, k: usize, _df_residual: usize) -> Result<Array2<f64>, String> {
    let mut meat = Array2::zeros((k, k));
    for i in 0..n {
        let h_ii = hat_diag_i(x, xtx_inv, i);
        let wi = 1.0 / (1.0 - h_ii).max(1e-10);
        let u2w = u[i] * u[i] * wi;
        let xi = x.row(i);
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += u2w * xi[r] * xi[c];
            }
        }
    }
    let sandwich = xtx_inv.dot(&meat).dot(xtx_inv);
    Ok(sandwich)
}

/// HC3: 权重 w_i = 1 / (1 - h_ii)²
fn cov_hc3(x: &Array2<f64>, xtx_inv: &Array2<f64>, u: &Array1<f64>, n: usize, k: usize, _df_residual: usize) -> Result<Array2<f64>, String> {
    let mut meat = Array2::zeros((k, k));
    for i in 0..n {
        let h_ii = hat_diag_i(x, xtx_inv, i);
        let wi = 1.0 / ((1.0 - h_ii) * (1.0 - h_ii)).max(1e-10);
        let u2w = u[i] * u[i] * wi;
        let xi = x.row(i);
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += u2w * xi[r] * xi[c];
            }
        }
    }
    let sandwich = xtx_inv.dot(&meat).dot(xtx_inv);
    Ok(sandwich)
}

fn hat_diag_i(x: &Array2<f64>, xtx_inv: &Array2<f64>, i: usize) -> f64 {
    let xi = x.row(i);
    xi.dot(xtx_inv).dot(&xi.to_owned())
}

/// Cluster: (X'X)⁻¹ [Σ_g (X_g' u_g)(X_g' u_g)'] (X'X)⁻¹
fn cov_cluster(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let cluster_id = match cov_params {
        Some(CovParams::Cluster { cluster_id }) => cluster_id,
        _ => return Err("cluster cov_type requires CovParams::Cluster".to_string()),
    };

    if cluster_id.len() != x.nrows() {
        return Err(format!(
            "cluster_id length {} does not match n={}",
            cluster_id.len(),
            x.nrows()
        ));
    }

    let k = x.ncols();
    let mut meat = Array2::zeros((k, k));

    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (i, &g) in cluster_id.iter().enumerate() {
        groups.entry(g).or_default().push(i);
    }

    for (_g, indices) in groups.iter() {
        let mut s_g = Array1::<f64>::zeros(k);
        for &i in indices {
            let ui = u[i];
            let xi = x.row(i);
            for r in 0..k {
                s_g[r] += ui * xi[r];
            }
        }
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += s_g[r] * s_g[c];
            }
        }
    }

    let g = groups.len() as f64;
    let n = x.nrows() as f64;
    let k_f = k as f64;
    let scale = if g > 1.0 && n > k_f {
        g / (g - 1.0) * (n - 1.0) / (n - k_f)
    } else {
        1.0
    };
    let meat_scaled = scale * meat;
    let sandwich = xtx_inv.dot(&meat_scaled).dot(xtx_inv);
    Ok(sandwich)
}
