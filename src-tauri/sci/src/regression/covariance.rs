//! OLS 协方差矩阵计算
//! 支持 nonrobust, HC0, HC1, HC2, HC3, fixed scale, cluster, HAC 等

use ndarray::{Array1, Array2};

/// 协方差计算所需的额外参数（cluster、HAC、fixed scale 等）
#[derive(Debug, Clone)]
pub enum CovParams {
    FixedScale {
        scale: f64,
    },
    Cluster {
        cluster_id: Vec<usize>,
        /// When true, use Stata xtreg,fe style: denom = (N-k-1) instead of (N-k).
        /// Only for FE within estimator where design matrix has slopes only (no absorbed dummies).
        /// For LSDV: use false — x already includes all dummies, (N-k) is correct.
        xtreg_fe_style: bool,
    },
    HAC {
        kernel: String,
        bandwidth: Option<i64>,
    },
    /// Stata newey: Bartlett kernel + n/(n-k) finite-sample adjustment (与 ivreg2 HAC 不同)
    Newey {
        lag: Option<i64>,
    },
    HacPanel {
        entity_id: Vec<usize>,
        time_id: Vec<usize>,
    },
    HacGroupsum {
        group_id: Vec<usize>,
    },
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
        "HAC" => cov_hac(x, xtx_inv, u, n, k, cov_params),
        "newey" => cov_newey(x, xtx_inv, u, n, k, df_residual, cov_params),
        "hac-panel" | "hac-groupsum" => Err(format!("cov_type '{}' not yet implemented", cov_type)),
        _ => cov_nonrobust(xtx_inv, u, df_residual),
    }
}

fn cov_nonrobust(
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    df_residual: usize,
) -> Result<Array2<f64>, String> {
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
        _ => {
            return Err(
                "fixed scale cov_type requires CovParams::FixedScale with scale".to_string(),
            );
        }
    };
    if scale <= 0.0 {
        return Err("fixed scale: scale must be positive".to_string());
    }
    Ok(scale * xtx_inv)
}

/// HC0: (X'X)⁻¹ X' diag(u²) X (X'X)⁻¹
fn cov_hc0(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
) -> Result<Array2<f64>, String> {
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
fn cov_hc1(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    df_residual: usize,
) -> Result<Array2<f64>, String> {
    let hc0 = cov_hc0(x, xtx_inv, u, n, k)?;
    let scale = n as f64 / df_residual as f64;
    Ok(scale * hc0)
}

/// HC2: 权重 w_i = 1 / (1 - h_ii)
fn cov_hc2(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    _df_residual: usize,
) -> Result<Array2<f64>, String> {
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
fn cov_hc3(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    _df_residual: usize,
) -> Result<Array2<f64>, String> {
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

/// HAC kernel weight at lag j (ivreg2 / Andrews 1991 style).
/// bandwidth b: max lag = b-1, x = j/b. Bartlett: w = 1 - j/b.
fn hac_kernel_weight(j: usize, bandwidth: usize, kernel: &str) -> f64 {
    if bandwidth == 0 || j >= bandwidth {
        return 0.0;
    }
    let x = j as f64 / bandwidth as f64;
    match kernel.to_lowercase().as_str() {
        "bartlett" => {
            // Andrews/ivreg2: w = 1 - j/bandwidth
            1.0 - x
        }
        "parzen" => {
            // Andrews (1991) Parzen kernel
            if x <= 0.5 {
                1.0 - 6.0 * x * x + 6.0 * x * x * x
            } else if x < 1.0 {
                let t = 1.0 - x;
                2.0 * t * t * t
            } else {
                0.0
            }
        }
        "quadratic spectral" | "quadratic spectral kernel" => {
            // Andrews (1991) Quadratic Spectral: k(z) = 3/x^2 * (sin(x)/x - cos(x)), x = 6πz/5
            if j == 0 {
                1.0
            } else {
                let z = x;
                let arg = 6.0 * std::f64::consts::PI * z / 5.0;
                if arg.abs() < 1e-15 {
                    1.0
                } else {
                    3.0 / (arg * arg) * (arg.sin() / arg - arg.cos())
                }
            }
        }
        _ => 1.0 - x, // default Bartlett
    }
}

/// Newey-West (1994) automatic bandwidth selection (ivreg2 bw(auto) / abw).
/// Returns bandwidth = optlag + 1. Per NW(1994) p.639, mstar = trunc(20*(T/100)^expo).
/// f = (u .* X) * h with h=1 for exog cols, h=0 for constant (last col).
fn newey_west_1994_bandwidth(
    x: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    kernel: &str,
) -> usize {
    let t = n as f64;
    let one_t = 1.0 / t;
    let (expo, q, cgamma) = match kernel.to_lowercase().as_str() {
        "parzen" => (4.0 / 25.0, 2, 2.6614),
        "quadratic spectral" | "quadratic spectral kernel" => (2.0 / 25.0, 2, 1.3221),
        _ => (2.0 / 9.0, 1, 1.4117), // Bartlett default
    };
    let mstar = (20.0 * (t / 100.0).powf(expo)).trunc() as usize;
    if mstar == 0 {
        return 1;
    }
    let h: Vec<f64> = if k <= 1 {
        vec![1.0; k]
    } else {
        (0..k).map(|c| if c == k - 1 { 0.0 } else { 1.0 }).collect()
    };
    let f: Vec<f64> = (0..n)
        .map(|i| {
            let mut s = 0.0;
            for c in 0..k {
                s += u[i] * x[[i, c]] * h[c];
            }
            s
        })
        .collect();
    let mut sigmahat = vec![one_t; mstar + 1];
    for j in 0..=mstar {
        let mut sum_val = 0.0;
        for i in j..n {
            sum_val += f[i] * f[i - j];
        }
        sigmahat[j] += sum_val * one_t;
    }
    let mut shatq = 0.0;
    let mut shat0 = sigmahat[0];
    for j in 1..=mstar {
        let jf = j as f64;
        shatq += 2.0 * sigmahat[j] * jf.powi(q as i32);
        shat0 += 2.0 * sigmahat[j];
    }
    let expon = 1.0 / (2.0 * q as f64 + 1.0);
    let gammahat = cgamma * (shatq / shat0).powf(2.0).powf(expon);
    let m = gammahat * t.powf(expon);
    let optlag = match kernel.to_lowercase().as_str() {
        "quadratic spectral" | "quadratic spectral kernel" => (m.min(mstar as f64)) as usize,
        _ => (m.trunc() as usize).min(mstar),
    };
    optlag.saturating_add(1)
}

/// HAC: (X'X)⁻¹ S (X'X)⁻¹（sandwich，无 n/(n-k)）
/// S = Σ_t e_t² x_t x_t' + Σ_{j=1}^{L} w_j Σ_{t=j+1}^{n} e_t e_{t-j} (x_t x_{t-j}' + x_{t-j} x_t')
/// ivreg2 bw(b): max lag = b-1, weight = 1 - j/b
fn cov_hac(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let (kernel, bandwidth) = match cov_params {
        Some(CovParams::HAC { kernel, bandwidth }) => (kernel.as_str(), *bandwidth),
        _ => {
            return Err(
                "HAC cov_type requires CovParams::HAC with kernel and bandwidth".to_string(),
            );
        }
    };

    // ivreg2 bw(b): max lag = b-1, weight = 1 - j/b.
    // bw(auto): full Newey-West (1994) procedure (mstar=20*(T/100)^expo, data-dependent optlag).
    let bw = match bandwidth {
        Some(q) if q > 0 => q as usize,
        Some(0) | Some(1) => 1usize, // bandwidth 0 or 1 => max_lag 0
        Some(_) => return Err("HAC bandwidth must be non-negative".to_string()),
        None => newey_west_1994_bandwidth(x, u, n, k, kernel),
    };
    let max_lag = bw.saturating_sub(1);

    let mut meat: Array2<f64> = Array2::zeros((k, k));

    // j=0: Σ_t e_t² x_t x_t'
    for t in 0..n {
        let e2 = u[t] * u[t];
        let xt = x.row(t);
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += e2 * xt[r] * xt[c];
            }
        }
    }

    // j=1..max_lag: w_j * Σ_{t=j+1}^{n} e_t e_{t-j} (x_t x_{t-j}' + x_{t-j} x_t')
    for j in 1..=max_lag.min(n.saturating_sub(1)) {
        let w = hac_kernel_weight(j, bw, kernel);
        for t in j..n {
            let e_e = u[t] * u[t - j] * w;
            let xt = x.row(t);
            let xtj = x.row(t - j);
            for r in 0..k {
                for c in 0..k {
                    meat[[r, c]] += e_e * (xt[r] * xtj[c] + xtj[r] * xt[c]);
                }
            }
        }
    }

    let sandwich = xtx_inv.dot(&meat).dot(xtx_inv);
    Ok(sandwich)
}

/// Newey: Stata newey 风格 — Bartlett kernel + n/(n-k) 有限样本调整
/// lag(0) = regress vce(robust) = HC1
fn cov_newey(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    n: usize,
    k: usize,
    df_residual: usize,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let lag = match cov_params {
        Some(CovParams::Newey { lag }) => *lag,
        _ => return Err("newey cov_type requires CovParams::Newey with lag".to_string()),
    };

    let l = match lag {
        Some(q) if q >= 0 => q as usize,
        Some(_) => return Err("Newey lag must be non-negative".to_string()),
        None => (4.0 * (n as f64 / 100.0).powf(2.0 / 9.0)).floor() as usize,
    };

    let mut meat = Array2::zeros((k, k));
    for t in 0..n {
        let e2 = u[t] * u[t];
        let xt = x.row(t);
        for r in 0..k {
            for c in 0..k {
                meat[[r, c]] += e2 * xt[r] * xt[c];
            }
        }
    }
    // Stata newey: weight = 1 - j/(l+1), so bandwidth = l+1 for Bartlett
    for j in 1..=l.min(n.saturating_sub(1)) {
        let w = hac_kernel_weight(j, l + 1, "bartlett");
        for t in j..n {
            let e_e = u[t] * u[t - j] * w;
            let xt = x.row(t);
            let xtj = x.row(t - j);
            for r in 0..k {
                for c in 0..k {
                    meat[[r, c]] += e_e * (xt[r] * xtj[c] + xtj[r] * xt[c]);
                }
            }
        }
    }

    let scale = if df_residual > 0 {
        n as f64 / df_residual as f64
    } else {
        1.0
    };
    let meat_scaled = scale * meat;
    let sandwich = xtx_inv.dot(&meat_scaled).dot(xtx_inv);
    Ok(sandwich)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regression::linear_model::{OLS, OLSConfig};

    #[test]
    fn test_hac_bartlett_bw1_equals_hc0() {
        // HAC bw(1) 无 lag 项，meat = HC0 meat，sandwich 无 n/(n-k) => 等于 HC0
        let n = 30;
        let k = 3;
        let mut exog_data = Vec::with_capacity(n * k);
        for i in 0..n {
            exog_data.push(1.0);
            exog_data.push((i as f64 * 0.1).sin());
            exog_data.push((i as f64 * 0.2).cos());
        }
        let exog = Array2::from_shape_vec((n, k), exog_data).unwrap();
        let endog = Array1::from_shape_fn(n, |i| (i as f64 * 0.15).sin() + (i as f64 * 0.08).cos());

        let ols_hc0 = OLS {
            endog: endog.clone(),
            exog: exog.clone(),
            config: OLSConfig {
                constant: true,
                cov_type: "HC0".to_string(),
                cov_params: None,
            },
        };
        let ols_hac = OLS {
            endog,
            exog,
            config: OLSConfig {
                constant: true,
                cov_type: "HAC".to_string(),
                cov_params: Some(CovParams::HAC {
                    kernel: "Bartlett".to_string(),
                    bandwidth: Some(1),
                }),
            },
        };

        let r_hc0 = ols_hc0.fit().unwrap();
        let r_hac = ols_hac.fit().unwrap();

        for i in 0..k {
            for j in 0..k {
                assert!(
                    (r_hac.cov_beta[[i, j]] - r_hc0.cov_beta[[i, j]]).abs() < 1e-9,
                    "HAC Bartlett bw(1) should equal HC0 at ({},{}): {} vs {}",
                    i,
                    j,
                    r_hac.cov_beta[[i, j]],
                    r_hc0.cov_beta[[i, j]]
                );
            }
        }
    }

    #[test]
    fn test_newey_lag0_equals_hc1() {
        // Stata newey lag(0) = regress vce(robust) = HC1
        let n = 30;
        let k = 3;
        let mut exog_data = Vec::with_capacity(n * k);
        for i in 0..n {
            exog_data.push(1.0);
            exog_data.push((i as f64 * 0.1).sin());
            exog_data.push((i as f64 * 0.2).cos());
        }
        let exog = Array2::from_shape_vec((n, k), exog_data).unwrap();
        let endog = Array1::from_shape_fn(n, |i| (i as f64 * 0.15).sin() + (i as f64 * 0.08).cos());

        let ols_hc1 = OLS {
            endog: endog.clone(),
            exog: exog.clone(),
            config: OLSConfig {
                constant: true,
                cov_type: "HC1".to_string(),
                cov_params: None,
            },
        };
        let ols_newey = OLS {
            endog,
            exog,
            config: OLSConfig {
                constant: true,
                cov_type: "newey".to_string(),
                cov_params: Some(CovParams::Newey { lag: Some(0) }),
            },
        };

        let r_hc1 = ols_hc1.fit().unwrap();
        let r_newey = ols_newey.fit().unwrap();

        for i in 0..k {
            for j in 0..k {
                assert!(
                    (r_newey.cov_beta[[i, j]] - r_hc1.cov_beta[[i, j]]).abs() < 1e-9,
                    "Newey lag=0 should equal HC1 (Stata newey lag(0)) at ({},{}): {} vs {}",
                    i,
                    j,
                    r_newey.cov_beta[[i, j]],
                    r_hc1.cov_beta[[i, j]]
                );
            }
        }
    }

    #[test]
    fn test_hac_bartlett_with_lag() {
        let n = 50;
        let k = 2;
        let mut exog_data = Vec::with_capacity(n * k);
        for i in 0..n {
            exog_data.push(1.0);
            exog_data.push(i as f64 / n as f64);
        }
        let exog = Array2::from_shape_vec((n, k), exog_data).unwrap();
        let endog = Array1::from_shape_fn(n, |i| {
            (i as f64 * 0.2).sin() * 2.0 + (i as f64 * 0.05).cos()
        });

        let ols = OLS {
            endog,
            exog,
            config: OLSConfig {
                constant: true,
                cov_type: "HAC".to_string(),
                cov_params: Some(CovParams::HAC {
                    kernel: "Bartlett".to_string(),
                    bandwidth: Some(5),
                }),
            },
        };

        let r = ols.fit().unwrap();
        for i in 0..k {
            assert!(r.cov_beta[[i, i]] > 0.0, "variance should be positive");
            for j in 0..k {
                assert!(
                    (r.cov_beta[[i, j]] - r.cov_beta[[j, i]]).abs() < 1e-10,
                    "covariance should be symmetric"
                );
            }
        }
    }
}

/// Cluster: (X'X)⁻¹ [Σ_g (X_g' u_g)(X_g' u_g)'] (X'X)⁻¹
fn cov_cluster(
    x: &Array2<f64>,
    xtx_inv: &Array2<f64>,
    u: &Array1<f64>,
    cov_params: Option<&CovParams>,
) -> Result<Array2<f64>, String> {
    let (cluster_id, xtreg_fe_style) = match cov_params {
        Some(CovParams::Cluster {
            cluster_id,
            xtreg_fe_style,
        }) => (cluster_id, *xtreg_fe_style),
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
        let denom = if xtreg_fe_style {
            (n - k_f - 1.0).max(1.0)
        } else {
            (n - k_f).max(1.0)
        };
        g / (g - 1.0) * (n - 1.0) / denom
    } else {
        1.0
    };
    let meat_scaled = scale * meat;
    let sandwich = xtx_inv.dot(&meat_scaled).dot(xtx_inv);
    Ok(sandwich)
}
