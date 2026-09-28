fn checked_fit(fit: &VecFit) -> Result<(), String> {
    let k = fit.var_names.len();
    let n = fit.statistics.observations;
    let m = fit.design.first().map_or(0, Vec::len);
    if k == 0
        || n == 0
        || m == 0
        || fit.lags == 0
        || fit.rank >= k
        || fit.design.len() != n
        || fit.design.iter().any(|v| v.len() != m)
        || fit.coefficients.len() != k
        || fit.coefficients.iter().any(|v| v.len() != m)
        || fit.residuals.len() != k
        || fit.residuals.iter().any(|v| v.len() != n)
        || fit.cointegration.beta.len() < k
        || fit.cointegration.beta.iter().any(|v| v.len() != fit.rank)
        || k.checked_mul(fit.lags - 1)
            .and_then(|v| v.checked_add(fit.rank))
            .is_none_or(|v| v > m)
        || fit
            .coefficients
            .iter()
            .flatten()
            .chain(fit.design.iter().flatten())
            .chain(fit.residuals.iter().flatten())
            .chain(fit.cointegration.beta.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err("VEC: invalid fitted model".into());
    }
    Ok(())
}

pub fn serial_correlation(fit: &VecFit, mlag: usize) -> Result<Vec<SerialCorrelationTest>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let n = fit.statistics.observations;
    let n_z_sr = fit.design[0].len();
    let residuals = &fit.residuals;
    let x_sr = Mat::from_fn(n, n_z_sr, |i, j| fit.design[i][j]);
    let z0 = Mat::from_fn(n, k, |i, j| {
        (0..n_z_sr)
            .map(|c| x_sr[(i, c)] * fit.coefficients[j][c])
            .sum::<f64>()
            + residuals[j][i]
    });
    // veclmar: LM 残差自相关检验（Stata veclmar，与 varlmar 相同思路）
    // LM_s = (T - d - 0.5) * ln(|Σ̂| / |Σ̃_s|)，df = K²，使用 ML 估计 Σ
    let u_mat = Mat::from_fn(n, k, |i, j| residuals[j][i]);
    let sigma_ml = (u_mat.transpose() * u_mat.as_ref()) / yss_sci_linalg::Scale(n as f64);
    let mut det_sigma_ml_copy = sigma_ml.clone();
    let det_sigma_hat = match cholesky_lower_in_place(&mut det_sigma_ml_copy) {
        Ok(()) => {
            let det_g: f64 = (0..k).map(|i| det_sigma_ml_copy[(i, i)]).product();
            (det_g * det_g).abs().max(1e-300)
        }
        Err(()) => 1e-300,
    };

    let mut veclmar = Vec::new();
    let n_z_aug_base = n_z_sr + k;
    for s in 1..=mlag {
        if s >= n {
            break;
        }
        let mut x_aug = Mat::zeros(n, n_z_aug_base);
        x_aug.submatrix_mut(0, 0, n, n_z_sr).copy_from(&x_sr);
        for j in 0..k {
            for i in 0..n {
                x_aug[(i, n_z_sr + j)] = if i >= s { residuals[j][i - s] } else { 0.0 };
            }
        }

        let x_aug_matrix = x_aug.as_ref().to_owned();
        let xt_aug = x_aug_matrix.transpose();
        let xtx_aug = xt_aug.as_ref() * x_aug_matrix.as_ref();
        let xtx_aug_inv = match xtx_aug.as_ref().checked_cholesky() {
            Ok(llt) => llt.solve(&Mat::<f64>::identity(xtx_aug.nrows(), xtx_aug.nrows())),
            Err(_) => continue,
        };

        let mut u_aug = Mat::zeros(n, k);
        for eq in 0..k {
            let y_col = z0.col(eq).to_owned();
            let y_vector = y_col.as_ref().to_owned();
            let xty = xt_aug.as_ref() * y_vector.as_ref();
            let beta_aug = xtx_aug_inv.as_ref() * xty.as_ref();
            let y_hat = x_aug_matrix.as_ref() * beta_aug.as_ref();
            let u = y_vector.as_ref() - y_hat.as_ref();
            let u_nd = u.as_ref().to_owned();
            for i in 0..n {
                u_aug[(i, eq)] = u_nd[i];
            }
        }

        let sigma_tilde = (u_aug.transpose() * u_aug.as_ref()) / yss_sci_linalg::Scale(n as f64);
        let mut det_tilde = sigma_tilde.clone();
        let det_sigma_tilde = match cholesky_lower_in_place(&mut det_tilde) {
            Ok(()) => {
                let det_g: f64 = (0..k).map(|i| det_tilde[(i, i)]).product();
                (det_g * det_g).abs().max(1e-300)
            }
            Err(()) => continue,
        };

        let lm_stat =
            (n as f64 - n_z_aug_base as f64 - 0.5) * (det_sigma_hat / det_sigma_tilde).ln();
        let lm_stat = lm_stat.max(0.0);
        let df_lm = k * k;
        let p_lm = chi_squared_sf(df_lm as f64, lm_stat);

        veclmar.push(SerialCorrelationTest {
            lag: s,
            chi2: lm_stat,
            df: df_lm,
            p_value: p_lm,
        });
    }

    Ok(veclmar)
}

pub fn stability(fit: &VecFit) -> Result<Vec<StabilityRoot>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let p = fit.lags;
    let r = fit.rank;
    let coefficients = &fit.coefficients;
    let alpha_nd = Mat::from_fn(k, r, |i, j| coefficients[i][j]);
    let beta_y = Mat::from_fn(k, r, |i, j| fit.cointegration.beta[i][j]);
    // vecstable: 特征值平稳性检验（Stata vecstable）
    // VEC 隐含 VAR 水平形式: y_t = A_1 y_{t-1} + ... + A_p y_{t-p}
    // A_1 = I + Π + Γ_1, A_i = Γ_i - Γ_{i-1} (i=2..p-1), A_p = -Γ_{p-1}, Π = αβ'
    let pi = alpha_nd.as_ref() * beta_y.transpose();
    let mut gamma_mats: Vec<Mat<f64>> = Vec::with_capacity(p);
    gamma_mats.push(Mat::zeros(k, k)); // Γ_0 = 0
    for i in 1..p {
        let mut g = Mat::zeros(k, k);
        for eq in 0..k {
            for j in 0..k {
                let idx = r + (i - 1) * k + j;
                if idx < coefficients[0].len() {
                    g[(eq, j)] = coefficients[eq][idx];
                }
            }
        }
        gamma_mats.push(g);
    }

    let mut a_mats: Vec<Mat<f64>> = Vec::with_capacity(p + 1);
    a_mats.push(Mat::zeros(k, k));
    let eye = Mat::<f64>::identity(k, k);
    if p == 1 {
        a_mats.push((&eye + &pi).to_owned());
    } else {
        a_mats.push((&eye + &pi + &gamma_mats[1]).to_owned());
        for i in 2..p {
            a_mats.push((&gamma_mats[i] - &gamma_mats[i - 1]).to_owned());
        }
        a_mats.push((-&gamma_mats[p - 1]).to_owned());
    }

    let kp = k * p;
    let mut companion = Mat::<f64>::zeros(kp, kp);
    for (lag_idx, a) in a_mats.iter().skip(1).enumerate() {
        for i in 0..k {
            for j in 0..k {
                companion.as_mut()[(i, lag_idx * k + j)] = a[(i, j)];
            }
        }
    }
    for block in 0..(p - 1) {
        for i in 0..k {
            companion.as_mut()[(k + block * k + i, block * k + i)] = 1.0;
        }
    }

    let vecstable = match yss_sci_linalg::Eigen::factor(companion.as_ref()) {
        Ok(evd) => {
            let s_diag = evd.values();
            (0..kp)
                .map(|i| {
                    let ev = s_diag[i];
                    let re = ev.re;
                    let im = ev.im;
                    let modulus = (re * re + im * im).sqrt();
                    StabilityRoot { re, im, modulus }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    Ok(vecstable)
}
