
/// VEC 估计：Johansen 方法
pub fn vec_estimate(
    y: &Array2<f64>,
    config: &VECConfig,
    var_names: Option<Vec<String>>,
    sindicators: Option<&Array2<f64>>,
) -> Result<VECResult, String> {
    let (_, k) = (y.nrows(), y.ncols());
    let p = config.lags;
    let r = config.rank;

    if p < 1 {
        return Err("VEC: lags must be >= 1".to_string());
    }
    if r >= k {
        return Err(format!(
            "VEC: rank({}) must be < number of variables ({})",
            r, k
        ));
    }

    let var_names = var_names.unwrap_or_else(|| (0..k).map(|i| format!("y{}", i)).collect());

    let s1 = johansen_stage1(y, p, config.trend_spec, sindicators)?;
    let n = s1.n;
    let m1 = s1.m1;
    let m2 = s1.m2;
    let has_const = s1.has_const;
    let has_trend = s1.has_trend;
    let z0 = s1.z0;
    let z1 = s1.z1;
    let z2 = s1.z2;
    let s00 = s1.s00;
    let s01 = s1.s01;
    let s10 = s1.s10;
    let s11 = s1.s11;
    let evals = s1.eval_pairs;
    let u_eigen = s1.u_eigen_real;
    let m_si = sindicators.map(|s| s.ncols()).unwrap_or(0);

    let n_lag_dy = k * (p - 1);
    let s11_faer = s11.view().into_faer().to_owned();

    let mut beta_tilde = Mat::zeros(m1, r);
    for (col, &(idx, _)) in evals.iter().take(r).enumerate() {
        for row in 0..m1 {
            beta_tilde.as_mut()[(row, col)] = u_eigen[[row, idx]];
        }
    }

    // Johansen 归一化: 前 r×r 块为 I_r
    let beta_1 = beta_tilde.as_ref().submatrix(0, 0, r, r);
    let beta_1_inv = beta_1.partial_piv_lu().solve(Mat::identity(r, r));

    let beta_norm = beta_tilde.as_ref() * beta_1_inv.as_ref();

    let beta_s11_beta = (beta_norm.as_ref().transpose() * s11_faer.as_ref() * beta_norm.as_ref()).to_owned();
    let beta_s11_beta_inv = beta_s11_beta
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: beta' S11 beta not positive definite".to_string())?
        .solve(Mat::identity(r, r));

    let s01_beta = s01.view().into_faer().to_owned() * beta_norm.as_ref();
    let alpha_mat = s01_beta.as_ref() * beta_s11_beta_inv.as_ref();

    let alpha_beta_s10 = alpha_mat.as_ref() * beta_norm.as_ref().transpose() * s10.view().into_faer();
    let omega = (s00.view().into_faer().to_owned() - alpha_beta_s10.as_ref()).to_owned();

    let omega_nd = omega.as_ref().into_ndarray().to_owned();
    let mut omega_chol = omega_nd.clone();
    cholesky_lower_in_place(&mut omega_chol)
        .map_err(|_| "VEC: Omega not positive definite".to_string())?;
    let det_omega: f64 = (0..k).map(|i| omega_chol[[i, i]]).product();
    let det_sigma_ml = (det_omega * det_omega).abs().max(1e-300);

    let ln_det_omega = 2.0 * (0..k).map(|i| omega_chol[[i, i]].ln()).sum::<f64>();
    let ll = -0.5
        * (n as f64)
        * (k as f64 * (2.0 * std::f64::consts::PI).ln() + k as f64 + ln_det_omega);

    let n_parms = (k * r + m1 * r + k * m2) as f64 - (r * r) as f64;
    let d = (n_parms / k as f64).floor() as usize;
    let aic = -2.0 * ll / (n as f64) + 2.0 * n_parms / (n as f64);
    let hqic = -2.0 * ll / (n as f64) + 2.0 * n_parms * (n as f64).ln().ln() / (n as f64);
    let sbic = -2.0 * ll / (n as f64) + n_parms * (n as f64).ln() / (n as f64);

    let mut beta_y_data = Vec::with_capacity(k * r);
    for i in 0..k {
        for j in 0..r {
            beta_y_data.push(beta_norm[(i, j)]);
        }
    }
    let beta_y = Array2::from_shape_vec((k, r), beta_y_data)
        .map_err(|_| "VEC: beta_y shape".to_string())?;

    let alpha_nd = alpha_mat.as_ref().into_ndarray().to_owned();

    let mut mu_rho: Vec<f64> = Vec::new();
    if has_const || has_trend {
        // Use Z1 (y_{t-1}) not r1 for backing out μ,ρ per Stata eq.(11)
        let ce_nd = z1.dot(&beta_y);
        let mut x_ce = Array2::zeros((n, r + m2));
        for i in 0..n {
            for j in 0..r {
                x_ce[[i, j]] = ce_nd[[i, j]];
            }
            for j in 0..m2 {
                x_ce[[i, r + j]] = z2[[i, j]];
            }
        }
        let x_faer = x_ce.view().into_faer().to_owned();
        let xt = x_faer.as_ref().transpose();
        let xtx = xt.as_ref() * x_faer.as_ref();
        let xtx_inv = xtx
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "VEC: X'X not positive definite in short-run regression".to_string())?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let z0_faer = z0.view().into_faer().to_owned();
        let xty = xt.as_ref() * z0_faer.as_ref();
        let gamma_full = xtx_inv.as_ref() * xty.as_ref();
        let gamma_nd = gamma_full.as_ref().into_ndarray().to_owned();

        let const_row = r + n_lag_dy;
        let trend_row = r + n_lag_dy + 1;

        let v_hat = if has_const {
            Array1::from_iter((0..k).map(|i| gamma_nd[[const_row, i]]))
        } else {
            Array1::zeros(k)
        };
        let delta_hat = if has_trend {
            Array1::from_iter((0..k).map(|i| gamma_nd[[trend_row, i]]))
        } else {
            Array1::zeros(k)
        };

        let alpha_aa = alpha_nd.t().dot(&alpha_nd);
        let alpha_aa_faer = alpha_aa.view().into_faer().to_owned();
        let alpha_aa_inv = alpha_aa_faer
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "VEC: alpha'alpha singular".to_string())?
            .solve(Mat::identity(r, r));
        if has_const && config.trend_spec == VecTrendSpec::Constant {
            let alpha_t_v = alpha_nd.t().dot(&v_hat);
            let v_col = Mat::from_fn(r, 1, |i, _| alpha_t_v[i]);
            let mu_col = alpha_aa_inv.as_ref() * v_col.as_ref();
            mu_rho.extend((0..r).map(|i| mu_col[(i, 0)]));
        }
        if has_trend {
            let alpha_t_d = alpha_nd.t().dot(&delta_hat);
            let d_col = Mat::from_fn(r, 1, |i, _| alpha_t_d[i]);
            let rho_col = alpha_aa_inv.as_ref() * d_col.as_ref();
            mu_rho.extend((0..r).map(|i| rho_col[(i, 0)]));
        }
    }

    // Stata uses demeaned CE: Ê_{t-1} = β'y_{t-1} + μ + ρ(t-1) (not r1 = residual of Z1 on Z2)
    let n_ce = r;
    let mut ce_vals = Array2::zeros((n, n_ce));
    for i in 0..n {
        let t_lag = (p + i - 1) as f64; // t-1 for Ê_{t-1}
        for j in 0..r {
            ce_vals[[i, j]] = (0..k).map(|kk| z1[[i, kk]] * beta_y[[kk, j]]).sum::<f64>()
                + mu_rho.get(j).copied().unwrap_or(0.0)
                + if has_trend {
                    mu_rho.get(r + j).copied().unwrap_or(0.0) * t_lag
                } else {
                    0.0
                };
        }
    }

    let n_z_sr = r + m2;
    let mut x_sr = Array2::zeros((n, n_z_sr));
    for i in 0..n {
        for j in 0..r {
            x_sr[[i, j]] = ce_vals[[i, j]];
        }
        for j in 0..m2 {
            x_sr[[i, r + j]] = z2[[i, j]];
        }
    }

    let x_faer = x_sr.view().into_faer().to_owned();
    let xt = x_faer.as_ref().transpose();
    let xtx = xt.as_ref() * x_faer.as_ref();
    let xtx_inv = xtx
        .as_ref()
        .llt(Side::Lower)
        .map_err(|_| "VEC: short-run X'X not positive definite".to_string())?
        .solve(Mat::identity(xtx.nrows(), xtx.ncols()));

    let mut coefficients = Vec::with_capacity(k);
    let mut std_errs = Vec::with_capacity(k);
    let mut residuals = Vec::with_capacity(k);
    let mut ss_res = Vec::with_capacity(k);
    let mut ss_tot = Vec::with_capacity(k);
    let mut cov_beta = Vec::with_capacity(k);
    let mut coef_labels = Vec::with_capacity(k);

    let sigma2_divisor = (n as f64 - d as f64).max(1.0);

    for eq in 0..k {
        let y_col = z0.column(eq).into_owned();
        let y_faer = y_col.view().into_faer_col().to_owned();
        let xty = xt.as_ref() * y_faer.as_ref();
        let beta_sr = xtx_inv.as_ref() * xty.as_ref();
        let y_hat = x_faer.as_ref() * beta_sr.as_ref();
        let u = y_faer.as_ref() - y_hat.as_ref();
        let u_nd = u.as_ref().into_ndarray().to_owned();

        let ss_r: f64 = u_nd.iter().map(|x| x * x).sum();
        let y_mean = y_col.mean().unwrap_or(0.0);
        let ss_t: f64 = y_col.iter().map(|x| (x - y_mean).powi(2)).sum();
        // R² = 1 - RSS/TSS per Stata reg3; TSS = Σ(Δy-Δȳ)² (standard formula)
        let ss_t_final: f64 = ss_t;

        let sigma2_eq = ss_r / sigma2_divisor;
        let xtx_inv_nd = xtx_inv.as_ref().into_ndarray().to_owned();
        let cov_eq = sigma2_eq * &xtx_inv_nd;
        cov_beta.push(cov_eq.clone());
        let se: Array1<f64> = cov_eq.diag().mapv(f64::sqrt);

        let mut labels = Vec::with_capacity(n_z_sr);
        for j in 0..r {
            labels.push(format!("_ce{}_L1.", j + 1));
        }
        for lag in 1..p {
            for j in 0..k {
                let name = var_names.get(j).cloned().unwrap_or_else(|| format!("y{}", j));
                // Stata notation: LD = lag of difference, L2D = lag 2 of difference
                let lag_prefix = if lag == 1 {
                    "LD.".to_string()
                } else {
                    format!("L{}D.", lag)
                };
                labels.push(format!("{}{}", lag_prefix, name));
            }
        }
        if has_const {
            labels.push("const".to_string());
        }
        if has_trend {
            labels.push("trend".to_string());
        }
        for j in 0..m_si {
            labels.push(format!("sind{}", j));
        }

        let beta_nd = beta_sr.as_ref().into_ndarray().to_owned();
        coefficients.push(beta_nd.to_vec());
        std_errs.push(se.to_vec());
        residuals.push(u_nd.to_vec());
        ss_res.push(ss_r);
        ss_tot.push(ss_t_final);
        coef_labels.push(labels);
    }

    let df_r = (n - d).max(1); // for RMSE/sigma: Stata VCE uses (T-d)
    let _df_r_eq = (n - n_z_sr).max(1); // Stata e(df r#) = n - params per equation

    let mut equations = Vec::with_capacity(k);
    let mut z_values = Vec::with_capacity(k);
    let mut p_values = Vec::with_capacity(k);
    let mut ci_lower = Vec::with_capacity(k);
    let mut ci_upper = Vec::with_capacity(k);

    for eq in 0..k {
        let rmse = (ss_res[eq] / df_r as f64).sqrt();
        // R² = 1 - SS_res/SS_tot (standard formula, Stata vec uses different definition)
        let r_sq = if ss_tot[eq] > 1e-300 {
            (1.0 - ss_res[eq] / ss_tot[eq]).max(0.0)
        } else {
            0.0
        };
        // chi2: Wald statistic W = β̂' V^{-1} β̂ (independent of R²)
        let chi2 = {
            let beta = Array1::from_vec(coefficients[eq].clone());
            let v = &cov_beta[eq];
            let v_faer = v.view().into_faer().to_owned();
            let beta_faer = beta.view().into_faer_col().to_owned();
            match v_faer.as_ref().llt(Side::Lower) {
                Ok(llt) => {
                    let x = llt.solve(beta_faer.as_ref());
                    let x_nd = x.as_ref().into_ndarray().to_owned();
                    beta.dot(&x_nd)
                }
                Err(_) => n as f64 * r_sq / (1.0 - r_sq.max(1e-10)),
            }
        };
        let p_chi2 = chi_squared_sf(n_z_sr as f64, chi2);

        let mut zv = Vec::with_capacity(n_z_sr);
        let mut pv = Vec::with_capacity(n_z_sr);
        let mut cl = Vec::with_capacity(n_z_sr);
        let mut cu = Vec::with_capacity(n_z_sr);
        for j in 0..n_z_sr {
            let z_val = if std_errs[eq][j].abs() > 1e-300 {
                coefficients[eq][j] / std_errs[eq][j]
            } else {
                0.0
            };
            let p_val = normal_two_sided_p(z_val);
            let ci_half = 1.96 * std_errs[eq][j];
            zv.push(z_val);
            pv.push(p_val);
            cl.push(coefficients[eq][j] - ci_half);
            cu.push(coefficients[eq][j] + ci_half);
        }
        z_values.push(zv);
        p_values.push(pv);
        ci_lower.push(cl);
        ci_upper.push(cu);

        let eq_name = format!(
            "D_{}",
            var_names.get(eq).cloned().unwrap_or_else(|| format!("y{}", eq))
        );
        equations.push(VECEquationStats {
            eq_name,
            parms: n_z_sr,
            rmse,
            r_sq,
            chi2,
            p_chi2,
        });
    }

    let mut beta_out: Vec<Vec<f64>> = (0..k).map(|i| (0..r).map(|j| beta_y[[i, j]]).collect()).collect();
    if has_const && mu_rho.len() >= r {
        beta_out.push((0..r).map(|j| mu_rho[j]).collect());
    }

    // Cointegrating equations chi2 (Stata formula: Wald on free params in beta)
    let cointegrating_equations = compute_cointegrating_equations_chi2(
        &beta_y,
        &alpha_nd,
        &omega_nd,
        &s11,
        n,
        d,
        r,
        k,
    );

    // beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]（Stata 公式 15）
    let (mut beta_std_err, mut beta_z_value, mut beta_p_value, mut beta_ci_lower, mut beta_ci_upper) =
        compute_beta_ce_stats(&beta_y, &alpha_nd, &omega_nd, &s11, n, d, r, k);
    if has_const {
        beta_std_err.push(vec![None; r]);
        beta_z_value.push(vec![None; r]);
        beta_p_value.push(vec![None; r]);
        beta_ci_lower.push(vec![None; r]);
        beta_ci_upper.push(vec![None; r]);
    }

    let trend_spec_str = match config.trend_spec {
        VecTrendSpec::None => "none",
        VecTrendSpec::Constant => "constant",
        VecTrendSpec::Trend => "trend",
    };

    // veclmar: LM 残差自相关检验（Stata veclmar，与 varlmar 相同思路）
    // LM_s = (T - d - 0.5) * ln(|Σ̂| / |Σ̃_s|)，df = K²，使用 ML 估计 Σ
    let u_mat = Array2::from_shape_fn((n, k), |(i, j)| residuals[j][i]);
    let sigma_ml = (u_mat.t().dot(&u_mat) / n as f64).to_owned();
    let mut det_sigma_ml_copy = sigma_ml.clone();
    let det_sigma_hat = match cholesky_lower_in_place(&mut det_sigma_ml_copy) {
        Ok(()) => {
            let det_g: f64 = (0..k).map(|i| det_sigma_ml_copy[[i, i]]).product();
            (det_g * det_g).abs().max(1e-300)
        }
        Err(()) => 1e-300,
    };

    let mlag = config.mlag;
    let mut veclmar = Vec::new();
    let n_z_aug_base = n_z_sr + k;
    for s in 1..=mlag {
        if s >= n {
            break;
        }
        let mut x_aug = Array2::zeros((n, n_z_aug_base));
        x_aug.slice_mut(ndarray::s![.., ..n_z_sr]).assign(&x_sr.view());
        for j in 0..k {
            for i in 0..n {
                x_aug[[i, n_z_sr + j]] = if i >= s { residuals[j][i - s] } else { 0.0 };
            }
        }

        let x_aug_faer = x_aug.view().into_faer().to_owned();
        let xt_aug = x_aug_faer.as_ref().transpose();
        let xtx_aug = xt_aug.as_ref() * x_aug_faer.as_ref();
        let xtx_aug_inv = match xtx_aug.as_ref().llt(Side::Lower) {
            Ok(llt) => llt.solve(Mat::identity(xtx_aug.nrows(), xtx_aug.ncols())),
            Err(_) => continue,
        };

        let mut u_aug = Array2::zeros((n, k));
        for eq in 0..k {
            let y_col = z0.column(eq).into_owned();
            let y_faer = y_col.view().into_faer_col().to_owned();
            let xty = xt_aug.as_ref() * y_faer.as_ref();
            let beta_aug = xtx_aug_inv.as_ref() * xty.as_ref();
            let y_hat = x_aug_faer.as_ref() * beta_aug.as_ref();
            let u = y_faer.as_ref() - y_hat.as_ref();
            let u_nd = u.as_ref().into_ndarray().to_owned();
            for i in 0..n {
                u_aug[[i, eq]] = u_nd[i];
            }
        }

        let sigma_tilde = (u_aug.t().dot(&u_aug) / n as f64).to_owned();
        let mut det_tilde = sigma_tilde.clone();
        let det_sigma_tilde = match cholesky_lower_in_place(&mut det_tilde) {
            Ok(()) => {
                let det_g: f64 = (0..k).map(|i| det_tilde[[i, i]]).product();
                (det_g * det_g).abs().max(1e-300)
            }
            Err(()) => continue,
        };

        let lm_stat = (n as f64 - n_z_aug_base as f64 - 0.5) * (det_sigma_hat / det_sigma_tilde).ln();
        let lm_stat = lm_stat.max(0.0);
        let df_lm = k * k;
        let p_lm = chi_squared_sf(df_lm as f64, lm_stat);

        veclmar.push(VecLmarRow {
            lag: s,
            chi2: lm_stat,
            df: df_lm,
            p_value: p_lm,
        });
    }

    // vecstable: 特征值平稳性检验（Stata vecstable）
    // VEC 隐含 VAR 水平形式: y_t = A_1 y_{t-1} + ... + A_p y_{t-p}
    // A_1 = I + Π + Γ_1, A_i = Γ_i - Γ_{i-1} (i=2..p-1), A_p = -Γ_{p-1}, Π = αβ'
    let pi = alpha_nd.dot(&beta_y.t());
    let mut gamma_mats: Vec<Array2<f64>> = Vec::with_capacity(p);
    gamma_mats.push(Array2::zeros((k, k))); // Γ_0 = 0
    for i in 1..p {
        let mut g = Array2::zeros((k, k));
        for eq in 0..k {
            for j in 0..k {
                let idx = r + (i - 1) * k + j;
                if idx < coefficients[0].len() {
                    g[[eq, j]] = coefficients[eq][idx];
                }
            }
        }
        gamma_mats.push(g);
    }

    let mut a_mats: Vec<Array2<f64>> = Vec::with_capacity(p + 1);
    a_mats.push(Array2::zeros((k, k)));
    let eye = Array2::eye(k);
    a_mats.push((&eye + &pi + &gamma_mats[1]).to_owned());
    for i in 2..p {
        a_mats.push((&gamma_mats[i] - &gamma_mats[i - 1]).to_owned());
    }
    a_mats.push((-&gamma_mats[p - 1]).to_owned());

    let kp = k * p;
    let mut companion = Mat::zeros(kp, kp);
    for (lag_idx, a) in a_mats.iter().skip(1).enumerate() {
        for i in 0..k {
            for j in 0..k {
                companion.as_mut()[(i, lag_idx * k + j)] = a[[i, j]];
            }
        }
    }
    for block in 0..(p - 1) {
        for i in 0..k {
            companion.as_mut()[(k + block * k + i, block * k + i)] = 1.0;
        }
    }

    let vecstable = match Eigen::new_from_real(companion.as_ref()) {
        Ok(evd) => {
            let s_diag = evd.S().column_vector();
            (0..kp)
                .map(|i| {
                    let ev = s_diag.get(i);
                    let re = ev.re;
                    let im = ev.im;
                    let modulus = (re * re + im * im).sqrt();
                    VecStableRow { re, im, modulus }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    Ok(VECResult {
        var_names,
        num_observation: n,
        log_likelihood: ll,
        aic,
        hqic,
        sbic,
        det_sigma_ml,
        rank: r,
        lags: p,
        trend_spec: trend_spec_str.to_string(),
        beta: beta_out,
        coefficients,
        std_errs,
        z_values,
        p_values,
        ci_lower,
        ci_upper,
        coef_labels,
        equations,
        cointegrating_equations,
        beta_std_err,
        beta_z_value,
        beta_p_value,
        beta_ci_lower,
        beta_ci_upper,
        veclmar,
        vecstable,
    })
}
