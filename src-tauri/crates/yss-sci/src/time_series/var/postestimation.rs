fn checked_fit(fit: &VarFit) -> Result<(), String> {
    let k = fit.var_names.len();
    let n = fit.statistics.observations;
    let m = fit.design.first().map_or(0, Vec::len);
    if k == 0
        || n == 0
        || m == 0
        || fit.design.len() != n
        || fit.design.iter().any(|v| v.len() != m)
        || fit.coefficients.len() != k
        || fit.coefficients.iter().any(|v| v.len() != m)
        || fit.residuals.len() != k
        || fit.residuals.iter().any(|v| v.len() != n)
        || fit.sigma.len() != k
        || fit.sigma.iter().any(|v| v.len() != k)
        || fit.statistics.coefficients.len() != k
        || fit
            .statistics
            .coefficients
            .iter()
            .any(|v| v.covariance.len() != m || v.covariance.iter().any(|r| r.len() != m))
        || fit.lags.contains(&0)
        || k.checked_mul(fit.lags.len()).is_none_or(|v| v > m)
        || fit
            .coefficients
            .iter()
            .flatten()
            .chain(fit.design.iter().flatten())
            .chain(fit.residuals.iter().flatten())
            .chain(fit.sigma.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err("VAR: invalid fitted model".into());
    }
    Ok(())
}

fn lag_matrices(fit: &VarFit) -> Result<Vec<Mat<f64>>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let lags = &fit.lags;
    let coefficients = &fit.coefficients;
    let p_model = lags.iter().copied().max().unwrap_or(0);
    let mut a_mats: Vec<Mat<f64>> = Vec::with_capacity(p_model + 1);
    for _ in 0..=p_model {
        a_mats.push(Mat::zeros(k, k));
    }
    let n_lags = lags.len();
    for (lag_idx, &lag) in lags.iter().enumerate() {
        for i in 0..k {
            for j in 0..k {
                let coef_idx = j * n_lags + lag_idx;
                a_mats[lag][(i, j)] = coefficients[i][coef_idx];
            }
        }
    }

    Ok(a_mats)
}

pub fn impulse_responses(fit: &VarFit, step: usize) -> Result<Vec<Vec<Vec<f64>>>, String> {
    let a_mats = lag_matrices(fit)?;
    let k = fit.var_names.len();
    let lags = &fit.lags;
    let p_model = a_mats.len() - 1;
    let sigma = Mat::from_fn(k, k, |i, j| fit.sigma[i][j]);
    // IRF: Φ_0 = I, Φ_s = Σ A_i Φ_{s-i}
    let mut phi: Vec<Mat<f64>> = vec![Mat::identity(k, k)];
    for s in 1..=step {
        let mut phi_s = Mat::zeros(k, k);
        for i in 1..=s.min(p_model) {
            if lags.contains(&i) {
                phi_s += a_mats[i].as_ref() * phi[s - i].as_ref();
            }
        }
        phi.push(phi_s);
    }

    // Cholesky: Σ = GG', G lower triangular (in-place on sigma copy)
    let mut g_nd = sigma.clone();
    cholesky_lower_in_place(&mut g_nd)
        .map_err(|_| "VAR: Sigma not positive definite for Cholesky".to_string())?;

    // OIRF: Θ_s = Φ_s G
    let mut oirf: Vec<Vec<Vec<f64>>> = Vec::with_capacity(step + 1);
    for phi_s in phi.iter().take(step + 1) {
        let theta_s = phi_s.as_ref() * g_nd.as_ref();
        oirf.push(
            (0..k)
                .map(|i| (0..k).map(|j| theta_s[(i, j)]).collect())
                .collect(),
        );
    }

    Ok(oirf)
}

pub fn variance_decomposition(fit: &VarFit, step: usize) -> Result<Vec<Vec<Vec<f64>>>, String> {
    let responses = impulse_responses(fit, step)?;
    let k = fit.var_names.len();
    let theta: Vec<_> = responses
        .iter()
        .map(|v| Mat::from_fn(k, k, |i, j| v[i][j]))
        .collect();
    // FEVD: MSE(h) = Σ_{s=0}^{h-1} Θ_s Θ_s', FEVD_ij(h) = Σ_{s=0}^{h-1} Θ_s[i,j]^2 / MSE_ii(h)
    let mut fevd = Vec::with_capacity(step + 1);
    let mut mse: Mat<f64> = Mat::zeros(k, k);
    for s in 0..=step {
        let theta_s = &theta[s];
        mse += theta_s.as_ref() * theta_s.transpose();
        let mut fevd_s = vec![vec![0.0; k]; k];
        for i in 0..k {
            let mse_ii = mse[(i, i)];
            if mse_ii > 1e-300 {
                for j in 0..k {
                    let mut sum = 0.0;
                    for theta_m in theta.iter().take(s + 1) {
                        sum += theta_m[(i, j)].powi(2);
                    }
                    fevd_s[i][j] = sum / mse_ii;
                }
            } else {
                for (j, value) in fevd_s[i].iter_mut().enumerate() {
                    *value = if i == j { 1.0 } else { 0.0 };
                }
            }
        }
        fevd.push(fevd_s);
    }

    Ok(fevd)
}

pub fn lag_exclusion(fit: &VarFit) -> Result<Vec<VARWleRow>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let lags = &fit.lags;
    let n_lags = lags.len();
    let coefficients = &fit.coefficients;
    let var_names = &fit.var_names;
    let m = fit.design[0].len();
    let cov_beta: Vec<_> = fit
        .statistics
        .coefficients
        .iter()
        .map(|s| Mat::from_fn(m, m, |i, j| s.covariance[i][j]))
        .collect();
    // varwle: Wald lag-exclusion 检验（Stata varwle 命令）
    // 对每个 lag，检验该 lag 的 k 个系数是否联合为零
    // 系数顺序：先 y 再 lag → L1.y1, L2.y1; L1.y2, L2.y2; ... 故 lag_idx 对应索引 j*n_lags+lag_idx
    let mut varwle = Vec::new();
    for (lag_idx, &lag) in lags.iter().enumerate() {
        let lag_indices: Vec<usize> = (0..k).map(|j| j * n_lags + lag_idx).collect();

        // 每个方程
        let mut chi2_all = 0.0;
        for eq in 0..k {
            let beta_lag: Col<f64> =
                Col::from_iter(lag_indices.iter().map(|&idx| coefficients[eq][idx]));
            let v_block = Mat::from_fn(k, k, |r, c| cov_beta[eq][(lag_indices[r], lag_indices[c])]);
            let v_matrix = v_block.as_ref();
            let beta_vector = beta_lag.as_ref().to_owned();
            let x = v_matrix
                .checked_cholesky()
                .map_err(|_| "VAR varwle: lag block V not positive definite".to_string())?
                .solve(&beta_vector.as_ref());
            let x_nd = x.as_ref().to_owned();
            let wald_eq: f64 = beta_lag.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
            chi2_all += wald_eq;

            let p_eq = chi_squared_sf(k as f64, wald_eq);
            let eq_name = var_names
                .get(eq)
                .cloned()
                .unwrap_or_else(|| format!("eq{}", eq));
            varwle.push(VARWleRow {
                eq_name,
                lag,
                chi2: wald_eq,
                df: k,
                p_value: p_eq,
            });
        }

        // All equations jointly
        let p_all = chi_squared_sf((k * k) as f64, chi2_all);
        varwle.push(VARWleRow {
            eq_name: "All".to_string(),
            lag,
            chi2: chi2_all,
            df: k * k,
            p_value: p_all,
        });
    }

    Ok(varwle)
}

pub fn granger(fit: &VarFit) -> Result<Vec<VARGrangerRow>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let lags = &fit.lags;
    let n_lags = lags.len();
    let coefficients = &fit.coefficients;
    let var_names = &fit.var_names;
    let m = fit.design[0].len();
    let cov_beta: Vec<_> = fit
        .statistics
        .coefficients
        .iter()
        .map(|s| Mat::from_fn(m, m, |i, j| s.covariance[i][j]))
        .collect();
    // vargranger: 格兰杰因果 Wald 检验（Stata vargranger 命令）
    // 对每个方程 i，检验排除变量 j（j≠i）的滞后项是否显著；Excluded "ALL" 为排除所有其他变量
    // 系数顺序：变量优先 → L1.y0, L2.y0, ...; L1.y1, L2.y1, ...；变量 j 的索引为 j*n_lags .. j*n_lags+n_lags-1
    let mut vargranger = Vec::new();
    for eq in 0..k {
        let eq_name = var_names
            .get(eq)
            .cloned()
            .unwrap_or_else(|| format!("eq{}", eq));
        let cov = &cov_beta[eq];
        let beta = &coefficients[eq];

        // 对每个被排除的变量 j（j != eq）
        for j in 0..k {
            if j == eq {
                continue;
            }
            let indices: Vec<usize> = (0..n_lags).map(|s| j * n_lags + s).collect();
            let beta_r: Col<f64> = Col::from_iter(indices.iter().map(|&idx| beta[idx]));
            let v_block = Mat::from_fn(n_lags, n_lags, |r, c| cov[(indices[r], indices[c])]);
            let v_matrix = v_block.as_ref();
            let beta_vector = beta_r.as_ref().to_owned();
            let x = v_matrix
                .checked_cholesky()
                .map_err(|_| "VAR vargranger: block V not positive definite".to_string())?
                .solve(&beta_vector.as_ref());
            let x_nd = x.as_ref().to_owned();
            let wald: f64 = beta_r.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
            let p_val = chi_squared_sf(n_lags as f64, wald);
            let excluded_name = var_names
                .get(j)
                .cloned()
                .unwrap_or_else(|| format!("y{}", j));
            vargranger.push(VARGrangerRow {
                eq_name: eq_name.clone(),
                excluded: excluded_name,
                chi2: wald,
                df: n_lags,
                p_value: p_val,
            });
        }

        // Excluded ALL：排除所有 j != eq
        let mut all_indices = Vec::new();
        for j in 0..k {
            if j != eq {
                for s in 0..n_lags {
                    all_indices.push(j * n_lags + s);
                }
            }
        }
        let r = all_indices.len();
        if r > 0 {
            let beta_r: Col<f64> = Col::from_iter(all_indices.iter().map(|&idx| beta[idx]));
            let v_block = Mat::from_fn(r, r, |ri, ci| cov[(all_indices[ri], all_indices[ci])]);
            let v_matrix = v_block.as_ref();
            let beta_vector = beta_r.as_ref().to_owned();
            let x = v_matrix
                .checked_cholesky()
                .map_err(|_| "VAR vargranger: ALL block V not positive definite".to_string())?
                .solve(&beta_vector.as_ref());
            let x_nd = x.as_ref().to_owned();
            let wald: f64 = beta_r.iter().zip(x_nd.iter()).map(|(b, xi)| b * xi).sum();
            let p_val = chi_squared_sf(r as f64, wald);
            vargranger.push(VARGrangerRow {
                eq_name: eq_name.clone(),
                excluded: "ALL".to_string(),
                chi2: wald,
                df: r,
                p_value: p_val,
            });
        }
    }

    Ok(vargranger)
}

pub fn serial_correlation(fit: &VarFit, mlag: usize) -> Result<Vec<SerialCorrelationTest>, String> {
    checked_fit(fit)?;
    let k = fit.var_names.len();
    let n_obs = fit.statistics.observations;
    let n_z = fit.design[0].len();
    let residuals = &fit.residuals;
    let z = Mat::from_fn(n_obs, n_z, |i, j| fit.design[i][j]);
    let u_mat = Mat::from_fn(n_obs, k, |i, j| residuals[j][i]);
    let y_dep = Mat::from_fn(n_obs, k, |i, j| {
        (0..n_z)
            .map(|c| z[(i, c)] * fit.coefficients[j][c])
            .sum::<f64>()
            + residuals[j][i]
    });
    // varlmar: LM 残差自相关检验（Stata varlmar 命令，Johansen 1995）
    // LM_s = (T - d - 0.5) * ln(|Σ̂| / |Σ̃_s|)，df = K²
    // varlmar 始终使用 ML 估计 Σ（除数 T）
    let sigma_ml = (u_mat.transpose() * u_mat.as_ref()) / yss_sci_linalg::Scale(n_obs as f64);
    let mut det_sigma_ml_var = sigma_ml.clone();
    cholesky_lower_in_place(&mut det_sigma_ml_var)
        .map_err(|_| "VAR varlmar: Sigma_ml not positive definite".to_string())?;
    let det_g_ml: f64 = (0..k).map(|i| det_sigma_ml_var[(i, i)]).product();
    let det_sigma_hat = (det_g_ml * det_g_ml).abs().max(1e-300);

    let mut varlmar = Vec::new();
    for s in 1..=mlag {
        if s >= n_obs {
            break;
        }
        // 构建 augmented Z: [Z_orig | res_lag_s]，res_lag_s 为 K 列，第 j 列为 residuals[j] 滞后 s 期（前 s 行填 0）
        let n_z_aug = n_z + k;
        let mut z_aug = Mat::zeros(n_obs, n_z_aug);
        z_aug.submatrix_mut(0, 0, n_obs, n_z).copy_from(&z);
        for j in 0..k {
            for i in 0..n_obs {
                z_aug[(i, n_z + j)] = if i >= s { residuals[j][i - s] } else { 0.0 };
            }
        }

        let z_aug_matrix = z_aug.as_ref().to_owned();
        let zt_aug = z_aug_matrix.transpose();
        let ztz_aug = zt_aug.as_ref() * z_aug_matrix.as_ref();
        let ztz_aug_inv = ztz_aug
            .checked_cholesky()
            .map_err(|_| "VAR varlmar: augmented Z'Z not positive definite".to_string())?
            .solve(&Mat::<f64>::identity(ztz_aug.nrows(), ztz_aug.nrows()));

        let mut u_aug = Mat::zeros(n_obs, k);
        for eq in 0..k {
            let y_col = y_dep.col(eq).to_owned();
            let y_vector = y_col.as_ref().to_owned();
            let zty = zt_aug.as_ref() * y_vector.as_ref();
            let beta = ztz_aug_inv.as_ref() * zty.as_ref();
            let y_hat = z_aug_matrix.as_ref() * beta.as_ref();
            let u = y_vector.as_ref() - y_hat.as_ref();
            let u_nd = u.as_ref().to_owned();
            for i in 0..n_obs {
                u_aug[(i, eq)] = u_nd[i];
            }
        }

        let sigma_tilde =
            (u_aug.transpose() * u_aug.as_ref()) / yss_sci_linalg::Scale(n_obs as f64);
        let mut det_tilde = sigma_tilde.clone();
        cholesky_lower_in_place(&mut det_tilde)
            .map_err(|_| "VAR varlmar: Sigma_tilde not positive definite".to_string())?;
        let det_g_tilde: f64 = (0..k).map(|i| det_tilde[(i, i)]).product();
        let det_sigma_tilde = (det_g_tilde * det_g_tilde).abs().max(1e-300);

        let d = n_z_aug;
        let lm_stat = (n_obs as f64 - d as f64 - 0.5) * (det_sigma_hat / det_sigma_tilde).ln();
        let lm_stat = lm_stat.max(0.0);
        let df_lm = k * k;
        let p_lm = chi_squared_sf(df_lm as f64, lm_stat);

        varlmar.push(SerialCorrelationTest {
            lag: s,
            chi2: lm_stat,
            df: df_lm,
            p_value: p_lm,
        });
    }

    Ok(varlmar)
}

pub fn stability(fit: &VarFit) -> Result<Vec<StabilityRoot>, String> {
    let a_mats = lag_matrices(fit)?;
    let k = fit.var_names.len();
    let lags = &fit.lags;
    let p_model = a_mats.len() - 1;
    if p_model == 0 {
        return Ok(Vec::new());
    }
    // varstable: 特征值平稳性检验（Stata varstable 命令）
    // 伴随机阵 A = [A1 A2 ... Ap; I 0 ... 0; ...; 0 ... I 0]，VAR 平稳当且仅当所有特征值模 < 1
    let kp = k * p_model;
    let mut companion = Mat::<f64>::zeros(kp, kp);
    for &lag in lags {
        for i in 0..k {
            for j in 0..k {
                companion.as_mut()[(i, (lag - 1) * k + j)] = a_mats[lag][(i, j)];
            }
        }
    }
    for block in 0..p_model.saturating_sub(1) {
        for i in 0..k {
            companion.as_mut()[(k + block * k + i, block * k + i)] = 1.0;
        }
    }
    let evd = yss_sci_linalg::Eigen::factor(companion.as_ref())
        .map_err(|_| "VAR varstable: eigendecomposition failed".to_string())?;
    let s_diag = evd.values();
    let mut varstable = Vec::with_capacity(kp);
    for ev in s_diag.iter() {
        let re: f64 = ev.re;
        let im: f64 = ev.im;
        let modulus = (re * re + im * im).sqrt();
        varstable.push(StabilityRoot { re, im, modulus });
    }

    Ok(varstable)
}
