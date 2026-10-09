//! OLS 协方差矩阵计算
//! 支持 nonrobust, HC0, HC1, HC2, HC3, fixed scale, cluster, HAC 等

use yss_sci_linalg::{Col, ColRef, Mat};

use yss_sci_contract::regression::OlsCovariance;

/// 计算参数协方差矩阵 cov_beta
/// - x: (n × k) 设计矩阵
/// - xtx_inv: (X'X)⁻¹
/// - u: (n,) 残差向量
/// - df_residual: n - k
/// - intercept_col: configured intercept position, including after weighting
pub fn compute_cov_beta(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    df_residual: usize,
    intercept_col: Option<usize>,
    covariance: &OlsCovariance,
) -> Result<Mat<f64>, String> {
    let n = x.nrows();
    let k = x.ncols();
    if intercept_col.is_some_and(|column| column >= k) {
        return Err("Intercept column is outside the design matrix".into());
    }

    match covariance {
        OlsCovariance::NonRobust => cov_nonrobust(xtx_inv, u, df_residual),
        OlsCovariance::FixedScale { scale } => cov_fixed_scale(xtx_inv, *scale),
        OlsCovariance::Hc0 => cov_hc0(x, xtx_inv, u),
        OlsCovariance::Hc1 => cov_hc1(x, xtx_inv, u, n, df_residual),
        OlsCovariance::Hc2 => cov_hc2(x, xtx_inv, u, n, k),
        OlsCovariance::Hc3 => cov_hc3(x, xtx_inv, u, n, k),
        OlsCovariance::Cluster {
            cluster_id,
            xtreg_fe_style,
        } => cov_cluster(x, xtx_inv, u, cluster_id, *xtreg_fe_style),
        OlsCovariance::Hac { kernel, bandwidth } => {
            cov_hac(x, xtx_inv, u, intercept_col, kernel, *bandwidth)
        }
        OlsCovariance::Newey { lag } => cov_newey(x, xtx_inv, u, df_residual, *lag),
    }
}

fn cov_nonrobust(
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    df_residual: usize,
) -> Result<Mat<f64>, String> {
    let sigma2 = (u.transpose() * u.as_ref()) / df_residual as f64;
    Ok(yss_sci_linalg::Scale(sigma2) * xtx_inv)
}

/// Fixed scale: scale * (X'X)⁻¹，scale 由用户通过 Config 指定
fn cov_fixed_scale(xtx_inv: &Mat<f64>, scale: f64) -> Result<Mat<f64>, String> {
    if scale <= 0.0 {
        return Err("fixed scale: scale must be positive".to_string());
    }
    Ok(yss_sci_linalg::Scale(scale) * xtx_inv)
}

/// HC0: (X'X)⁻¹ X' diag(u²) X (X'X)⁻¹
fn cov_hc0(x: &Mat<f64>, xtx_inv: &Mat<f64>, u: ColRef<'_, f64>) -> Result<Mat<f64>, String> {
    let meat = lagged_score_meat(x, |row| u[row], "bartlett", 1);
    let sandwich = (xtx_inv.as_ref() * meat.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

/// HC1: HC0 × n / (n - k)
fn cov_hc1(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    n: usize,
    df_residual: usize,
) -> Result<Mat<f64>, String> {
    let hc0 = cov_hc0(x, xtx_inv, u)?;
    let scale = n as f64 / df_residual as f64;
    Ok(yss_sci_linalg::Scale(scale) * hc0)
}

/// HC2: 权重 w_i = 1 / (1 - h_ii)
fn cov_hc2(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    n: usize,
    k: usize,
) -> Result<Mat<f64>, String> {
    let mut meat = Mat::<f64>::zeros(k, k);
    for i in 0..n {
        let h_ii = hat_diag_i(x, xtx_inv, i);
        let wi = 1.0 / (1.0 - h_ii).max(1e-10);
        let u2w = u[i] * u[i] * wi;
        let xi = x.row(i);
        for r in 0..k {
            for c in 0..k {
                meat[(r, c)] += u2w * xi[r] * xi[c];
            }
        }
    }
    let sandwich = (xtx_inv.as_ref() * meat.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

/// HC3: 权重 w_i = 1 / (1 - h_ii)²
fn cov_hc3(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    n: usize,
    k: usize,
) -> Result<Mat<f64>, String> {
    let mut meat = Mat::<f64>::zeros(k, k);
    for i in 0..n {
        let h_ii = hat_diag_i(x, xtx_inv, i);
        let wi = 1.0 / ((1.0 - h_ii) * (1.0 - h_ii)).max(1e-10);
        let u2w = u[i] * u[i] * wi;
        let xi = x.row(i);
        for r in 0..k {
            for c in 0..k {
                meat[(r, c)] += u2w * xi[r] * xi[c];
            }
        }
    }
    let sandwich = (xtx_inv.as_ref() * meat.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

fn hat_diag_i(x: &Mat<f64>, xtx_inv: &Mat<f64>, i: usize) -> f64 {
    let xi = x.row(i);
    xi * xtx_inv * xi.transpose()
}

/// HAC kernel weight at lag j (ivreg2 / Andrews 1991 style).
/// bandwidth b: x = j/b. Bartlett/Parzen support ends at b; QS uses every lag.
fn hac_kernel_weight(j: usize, bandwidth: usize, kernel: &str) -> f64 {
    let kernel = kernel.to_lowercase();
    if bandwidth == 0
        || (j >= bandwidth
            && !matches!(
                kernel.as_str(),
                "quadratic spectral" | "quadratic spectral kernel"
            ))
    {
        return 0.0;
    }
    let x = j as f64 / bandwidth as f64;
    match kernel.as_str() {
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

/// Newey-West (1994) direct-score automatic selection, without prewhitening.
/// The pilot estimates score moments; it does not cap the final integer bandwidth.
/// Returns optlag + 1 under the existing kernel bandwidth convention.
fn newey_west_1994_bandwidth(
    x: &Mat<f64>,
    row_weight: impl Fn(usize) -> f64,
    intercept_col: Option<usize>,
    kernel: &str,
) -> Result<usize, String> {
    let n = x.nrows();
    let k = x.ncols();
    let t = n as f64;
    let (expo, q, cgamma) = match kernel.to_lowercase().as_str() {
        "parzen" => (4.0 / 25.0, 2, 2.6614),
        "quadratic spectral" | "quadratic spectral kernel" => (2.0 / 25.0, 2, 1.3221),
        _ => (2.0 / 9.0, 1, 1.1447), // NW(1994), Table I, p.640
    };
    if n == 0 {
        return Err("Automatic HAC bandwidth requires observations".into());
    }
    let pilot_lags = ((20.0 * (t / 100.0).powf(expo)).trunc() as usize).min(n - 1);
    let mut f: Vec<f64> = (0..n)
        .map(|i| {
            let weight = row_weight(i);
            let mut score = 0.0;
            for c in 0..k {
                if k <= 1 || Some(c) != intercept_col {
                    score += weight * x[(i, c)];
                }
            }
            score
        })
        .collect();
    if f.iter().any(|value| !value.is_finite()) {
        return Err("Automatic HAC bandwidth requires finite scores".into());
    }
    let scale = f
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    if scale == 0.0 {
        return Ok(1);
    }
    // Every moment has the same quadratic scale and 1/n factor. Cancel them
    // before products to preserve the selector under response-unit changes.
    for value in &mut f {
        *value /= scale;
    }
    let mut shatq = 0.0;
    let mut shat0 = f.iter().map(|value| value * value).sum::<f64>();
    for j in 1..=pilot_lags {
        let sigma = (j..n).map(|i| f[i] * f[i - j]).sum::<f64>();
        shatq += 2.0 * sigma * (j as f64).powi(q);
        shat0 += 2.0 * sigma;
    }
    if shat0 == 0.0 {
        return Err("Automatic HAC bandwidth has undefined pilot covariance".into());
    }
    let expon = 1.0 / (2.0 * q as f64 + 1.0);
    // Taking the power before dividing avoids squaring an overflowing ratio.
    let gammahat = cgamma * shatq.abs().powf(2.0 * expon) / shat0.abs().powf(2.0 * expon);
    let m = gammahat * t.powf(expon);
    if !m.is_finite() || m >= usize::MAX as f64 {
        return Err("Automatic HAC bandwidth is out of range".into());
    }
    (m.trunc() as usize)
        .checked_add(1)
        .ok_or_else(|| "Automatic HAC bandwidth is out of range".into())
}

/// HAC: (X'X)⁻¹ S (X'X)⁻¹（sandwich，无 n/(n-k)）
/// S = Σ_t e_t² x_t x_t' + Σ_{j=1}^{L} w_j Σ_{t=j+1}^{n} e_t e_{t-j} (x_t x_{t-j}' + x_{t-j} x_t')
/// Bartlett/Parzen have at most b-1 lags; QS has n-1 lags with scale b.
fn cov_hac(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    intercept_col: Option<usize>,
    kernel: &str,
    bandwidth: Option<i64>,
) -> Result<Mat<f64>, String> {
    let bw = hac_bandwidth(x, |row| u[row], intercept_col, kernel, bandwidth)?;
    let meat = lagged_score_meat(x, |row| u[row], kernel, bw);

    let sandwich = (xtx_inv.as_ref() * meat.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

/// Newey: Stata newey 风格 — Bartlett kernel + n/(n-k) 有限样本调整
/// lag(0) = regress vce(robust) = HC1
fn cov_newey(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    df_residual: usize,
    lag: Option<i64>,
) -> Result<Mat<f64>, String> {
    let n = x.nrows();
    let bw = newey_bandwidth(n, lag)?;
    let meat = lagged_score_meat(x, |row| u[row], "bartlett", bw);

    let scale = if df_residual > 0 {
        n as f64 / df_residual as f64
    } else {
        1.0
    };
    let meat_scaled = yss_sci_linalg::Scale(scale) * meat;
    let sandwich = (xtx_inv.as_ref() * meat_scaled.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

/// Cluster: (X'X)⁻¹ [Σ_g (X_g' u_g)(X_g' u_g)'] (X'X)⁻¹
fn cov_cluster(
    x: &Mat<f64>,
    xtx_inv: &Mat<f64>,
    u: ColRef<'_, f64>,
    cluster_id: &[usize],
    xtreg_fe_style: bool,
) -> Result<Mat<f64>, String> {
    let (meat, groups) = clustered_score_meat(x, |row| u[row], cluster_id)?;

    let g = groups as f64;
    let n = x.nrows() as f64;
    let k_f = x.ncols() as f64;
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
    let meat_scaled = yss_sci_linalg::Scale(scale) * meat;
    let sandwich = (xtx_inv.as_ref() * meat_scaled.as_ref()).as_ref() * xtx_inv.as_ref();
    Ok(sandwich)
}

fn lagged_score_meat(
    x: &Mat<f64>,
    row_weight: impl Fn(usize) -> f64,
    kernel: &str,
    bandwidth: usize,
) -> Mat<f64> {
    let n = x.nrows();
    let k = x.ncols();
    let bw = bandwidth;
    let max_lag = match kernel.to_lowercase().as_str() {
        "quadratic spectral" | "quadratic spectral kernel" => n.saturating_sub(1),
        _ => bw.saturating_sub(1).min(n.saturating_sub(1)),
    };
    let mut meat: Mat<f64> = Mat::zeros(k, k);

    // j=0: Σ_t e_t² x_t x_t'
    for t in 0..n {
        let e2 = row_weight(t) * row_weight(t);
        let xt = x.row(t);
        for r in 0..k {
            for c in 0..k {
                meat[(r, c)] += e2 * xt[r] * xt[c];
            }
        }
    }

    // j=1..max_lag: w_j * Σ_{t=j+1}^{n} e_t e_{t-j} (x_t x_{t-j}' + x_{t-j} x_t')
    for j in 1..=max_lag {
        let w = hac_kernel_weight(j, bw, kernel);
        for t in j..n {
            let e_e = row_weight(t) * row_weight(t - j) * w;
            let xt = x.row(t);
            let xtj = x.row(t - j);
            for r in 0..k {
                for c in 0..k {
                    meat[(r, c)] += e_e * (xt[r] * xtj[c] + xtj[r] * xt[c]);
                }
            }
        }
    }

    meat
}

fn hac_bandwidth(
    x: &Mat<f64>,
    row_weight: impl Fn(usize) -> f64,
    intercept_col: Option<usize>,
    kernel: &str,
    bandwidth: Option<i64>,
) -> Result<usize, String> {
    match bandwidth {
        Some(q) if q > 0 => Ok(q as usize),
        Some(0) => Ok(1),
        Some(_) => Err("HAC bandwidth must be non-negative".into()),
        None => newey_west_1994_bandwidth(x, row_weight, intercept_col, kernel),
    }
}

fn newey_bandwidth(observations: usize, lag: Option<i64>) -> Result<usize, String> {
    let lag = match lag {
        Some(q) if q >= 0 => q as usize,
        Some(_) => return Err("Newey lag must be non-negative".into()),
        None => (4.0 * (observations as f64 / 100.0).powf(2.0 / 9.0)).floor() as usize,
    };
    Ok(lag + 1)
}

fn clustered_score_meat(
    x: &Mat<f64>,
    row_weight: impl Fn(usize) -> f64,
    cluster_id: &[usize],
) -> Result<(Mat<f64>, usize), String> {
    if cluster_id.len() != x.nrows() {
        return Err(format!(
            "cluster_id length {} does not match n={}",
            cluster_id.len(),
            x.nrows()
        ));
    }

    let k = x.ncols();
    let mut meat = Mat::<f64>::zeros(k, k);

    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (i, &g) in cluster_id.iter().enumerate() {
        groups.entry(g).or_default().push(i);
    }

    for (_g, indices) in groups.iter() {
        let mut s_g = Col::<f64>::zeros(k);
        for &i in indices {
            let ui = row_weight(i);
            let xi = x.row(i);
            for r in 0..k {
                s_g[r] += ui * xi[r];
            }
        }
        for r in 0..k {
            for c in 0..k {
                meat[(r, c)] += s_g[r] * s_g[c];
            }
        }
    }

    Ok((meat, groups.len()))
}

/// Covariance of the sum of score rows, without coefficient small-sample corrections.
pub(crate) fn score_covariance(
    scores: &Mat<f64>,
    covariance: &OlsCovariance,
) -> Result<Mat<f64>, String> {
    match covariance {
        OlsCovariance::Hc0 | OlsCovariance::Hc1 | OlsCovariance::Hc2 | OlsCovariance::Hc3 => {
            Ok(lagged_score_meat(scores, |_| 1.0, "bartlett", 1))
        }
        OlsCovariance::Cluster { cluster_id, .. } => {
            let (meat, groups) = clustered_score_meat(scores, |_| 1.0, cluster_id)?;
            if groups <= 1 {
                return Err("Score covariance requires independent clusters".into());
            }
            Ok(meat)
        }
        OlsCovariance::Hac { kernel, bandwidth } => {
            let bw = hac_bandwidth(scores, |_| 1.0, None, kernel, *bandwidth)?;
            Ok(lagged_score_meat(scores, |_| 1.0, kernel, bw))
        }
        OlsCovariance::Newey { lag } => {
            let bw = newey_bandwidth(scores.nrows(), *lag)?;
            Ok(lagged_score_meat(scores, |_| 1.0, "bartlett", bw))
        }
        _ => Err("Score covariance requires a robust covariance specification".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regression::linear::OLS;
    use yss_sci_contract::regression::{CovParams, OlsOptions};

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
        let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog_data), n, k).to_owned();
        let endog = Col::from_fn(n, |i| (i as f64 * 0.15).sin() + (i as f64 * 0.08).cos());

        let ols_hc0 = OLS {
            endog: endog.clone(),
            exog: exog.clone(),
            config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                true,
                "HC0",
                (None).as_ref(),
            )
            .expect("valid OLS covariance options"),
        };
        let ols_hac = OLS {
            endog,
            exog,
            config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                true,
                "HAC",
                (Some(CovParams::HAC {
                    kernel: "Bartlett".to_string(),
                    bandwidth: Some(1),
                }))
                .as_ref(),
            )
            .expect("valid OLS covariance options"),
        };

        let r_hc0 = ols_hc0.fit().unwrap();
        let r_hac = ols_hac.fit().unwrap();

        for i in 0..k {
            for j in 0..k {
                assert!(
                    (r_hac.cov_beta[(i, j)] - r_hc0.cov_beta[(i, j)]).abs() < 1e-9,
                    "HAC Bartlett bw(1) should equal HC0 at ({},{}): {} vs {}",
                    i,
                    j,
                    r_hac.cov_beta[(i, j)],
                    r_hc0.cov_beta[(i, j)]
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
        let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog_data), n, k).to_owned();
        let endog = Col::from_fn(n, |i| (i as f64 * 0.15).sin() + (i as f64 * 0.08).cos());

        let ols_hc1 = OLS {
            endog: endog.clone(),
            exog: exog.clone(),
            config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                true,
                "HC1",
                (None).as_ref(),
            )
            .expect("valid OLS covariance options"),
        };
        let ols_newey = OLS {
            endog,
            exog,
            config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                true,
                "newey",
                (Some(CovParams::Newey { lag: Some(0) })).as_ref(),
            )
            .expect("valid OLS covariance options"),
        };

        let r_hc1 = ols_hc1.fit().unwrap();
        let r_newey = ols_newey.fit().unwrap();

        for i in 0..k {
            for j in 0..k {
                assert!(
                    (r_newey.cov_beta[(i, j)] - r_hc1.cov_beta[(i, j)]).abs() < 1e-9,
                    "Newey lag=0 should equal HC1 (Stata newey lag(0)) at ({},{}): {} vs {}",
                    i,
                    j,
                    r_newey.cov_beta[(i, j)],
                    r_hc1.cov_beta[(i, j)]
                );
            }
        }
    }

    #[test]
    fn test_hac_bartlett_with_lag() {
        use yss_sci_linalg::{MatrixExt, Solve};
        let n = 50;
        let k = 2;
        let mut exog_data = Vec::with_capacity(n * k);
        for i in 0..n {
            exog_data.push(1.0);
            exog_data.push(i as f64 / n as f64);
        }
        let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog_data), n, k).to_owned();
        let endog = Col::from_fn(n, |i| {
            (i as f64 * 0.2).sin() * 2.0 + (i as f64 * 0.05).cos()
        });

        let ols = OLS {
            endog,
            exog,
            config: yss_sci_contract::regression::OlsOptions::from_covariance_parts(
                true,
                "HAC",
                (Some(CovParams::HAC {
                    kernel: "Bartlett".to_string(),
                    bandwidth: Some(5),
                }))
                .as_ref(),
            )
            .expect("valid OLS covariance options"),
        };

        let r = ols.fit().unwrap();
        for i in 0..k {
            assert!(r.cov_beta[(i, i)] > 0.0, "variance should be positive");
            for j in 0..k {
                assert!(
                    (r.cov_beta[(i, j)] - r.cov_beta[(j, i)]).abs() < 1e-10,
                    "covariance should be symmetric"
                );
            }
        }

        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/diagnostics_reference.json"
        ))
        .unwrap();
        let response: Vec<f64> = serde_json::from_value(data["y"].clone()).unwrap();
        let predictors: Vec<Vec<f64>> = serde_json::from_value(data["x"].clone()).unwrap();
        let n = response.len();
        for constant in [true, false] {
            let start = usize::from(constant);
            let k = 2 + start;
            let design = Mat::from_fn(n, k, |i, j| {
                if constant && j == 0 {
                    1.0
                } else {
                    predictors[j - start][i]
                }
            });
            for bandwidth in [None, Some(5)] {
                let fit = |order: &[usize]| {
                    OLS {
                        endog: Col::from_iter(response.iter().copied()),
                        exog: Mat::from_fn(n, k, |i, j| design[(i, order[j])]),
                        config: OlsOptions {
                            constant,
                            covariance: OlsCovariance::Hac {
                                kernel: "bartlett".into(),
                                bandwidth,
                            },
                        },
                    }
                    .fit()
                    .unwrap()
                };
                let mut order = (0..k).collect::<Vec<_>>();
                let expected = fit(&order);
                order.swap(start, start + 1);
                let reordered = fit(&order);
                for i in 0..k {
                    for j in 0..k {
                        assert!(
                            (reordered.cov_beta[(i, j)] - expected.cov_beta[(order[i], order[j])])
                                .abs()
                                < 1e-10,
                            "HAC covariance must follow predictor order: constant={constant}, bandwidth={bandwidth:?}, ({i}, {j})"
                        );
                    }
                }
                if constant {
                    let order = [1, 2, 0];
                    let inverse = (design.transpose() * design.as_ref())
                        .checked_cholesky()
                        .unwrap()
                        .solve(&Mat::identity(k, k));
                    let at_end = compute_cov_beta(
                        &Mat::from_fn(n, k, |i, j| design[(i, order[j])]),
                        &Mat::from_fn(k, k, |i, j| inverse[(order[i], order[j])]),
                        expected.residuals.as_ref(),
                        expected.df_residual,
                        Some(k - 1),
                        &OlsCovariance::Hac {
                            kernel: "bartlett".into(),
                            bandwidth,
                        },
                    )
                    .unwrap();
                    for i in 0..k {
                        for j in 0..k {
                            assert!(
                                (at_end[(i, j)] - expected.cov_beta[(order[i], order[j])]).abs()
                                    < 1e-10
                            );
                        }
                    }
                }
            }
        }

        let single = Mat::from_fn(n, 1, |_, _| 1.0);
        let residuals = Col::from_iter(response);
        assert_eq!(
            newey_west_1994_bandwidth(&single, |row| residuals[row], Some(0), "bartlett"),
            newey_west_1994_bandwidth(&single, |row| residuals[row], None, "bartlett")
        );
    }
}
