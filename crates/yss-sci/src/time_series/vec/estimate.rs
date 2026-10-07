/// VEC 估计：Johansen 方法
pub fn vec_estimate(
    y: &Mat<f64>,
    config: &VECConfig,
    var_names: Option<Vec<String>>,
    sindicators: Option<&Mat<f64>>,
) -> Result<VecFit, String> {
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

    let mut beta_tilde = Mat::<f64>::zeros(m1, r);
    for (col, &(idx, _)) in evals.iter().take(r).enumerate() {
        for row in 0..m1 {
            beta_tilde.as_mut()[(row, col)] = u_eigen[(row, idx)];
        }
    }

    // Johansen 归一化: 前 r×r 块为 I_r
    let beta_1 = beta_tilde.submatrix(0, 0, r, r);
    let beta_1_inv = beta_1
        .checked_lu()
        .map_err(|error| error.to_string())?
        .solve(&Mat::<f64>::identity(r, r));

    let beta_norm = beta_tilde.as_ref() * beta_1_inv.as_ref();

    let beta_s11_beta = (beta_norm.transpose() * s11.as_ref()).as_ref() * beta_norm.as_ref();
    let beta_s11_beta_inv = beta_s11_beta
        .as_ref()
        .checked_cholesky()
        .map_err(|_| "VEC: beta' S11 beta not positive definite".to_string())?
        .solve(&Mat::<f64>::identity(r, r));

    let s01_beta = s01.as_ref() * beta_norm.as_ref();
    let alpha_mat = s01_beta.as_ref() * beta_s11_beta_inv.as_ref();

    let alpha_beta_s10 = (alpha_mat.as_ref() * beta_norm.transpose()).as_ref() * s10.as_ref();
    let omega = s00.as_ref() - alpha_beta_s10.as_ref();

    let mut omega_chol = omega.clone();
    cholesky_lower_in_place(&mut omega_chol)
        .map_err(|_| "VEC: Omega not positive definite".to_string())?;
    let det_omega: f64 = (0..k).map(|i| omega_chol[(i, i)]).product();
    let det_sigma_ml = (det_omega * det_omega).abs().max(1e-300);

    let ln_det_omega = 2.0 * (0..k).map(|i| omega_chol[(i, i)].ln()).sum::<f64>();
    let ll = -0.5
        * (n as f64)
        * (k as f64 * (2.0 * std::f64::consts::PI).ln() + k as f64 + ln_det_omega);

    let n_parms = (k * r + m1 * r + k * m2) as f64 - (r * r) as f64;
    let d = (n_parms / k as f64).floor() as usize;
    let aic = -2.0 * ll / (n as f64) + 2.0 * n_parms / (n as f64);
    let hqic = -2.0 * ll / (n as f64) + 2.0 * n_parms * (n as f64).ln().ln() / (n as f64);
    let sbic = -2.0 * ll / (n as f64) + n_parms * (n as f64).ln() / (n as f64);

    let beta_y = beta_norm.submatrix(0, 0, k, r).to_owned();

    let mut mu_rho: Vec<f64> = Vec::new();
    if has_const || has_trend {
        // Use Z1 (y_{t-1}) not r1 for backing out μ,ρ per Stata eq.(11)
        let ce_nd = z1.as_ref() * beta_y.as_ref();
        let mut x_ce = Mat::zeros(n, r + m2);
        for i in 0..n {
            for j in 0..r {
                x_ce[(i, j)] = ce_nd[(i, j)];
            }
            for j in 0..m2 {
                x_ce[(i, r + j)] = z2[(i, j)];
            }
        }
        let xt = x_ce.transpose();
        let xtx = xt.as_ref() * x_ce.as_ref();
        let xtx_inv = xtx
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "VEC: X'X not positive definite in short-run regression".to_string())?
            .solve(&Mat::<f64>::identity(xtx.nrows(), xtx.nrows()));
        let xty = xt.as_ref() * z0.as_ref();
        let gamma_full = xtx_inv.as_ref() * xty.as_ref();

        let const_row = r + n_lag_dy;
        let trend_row = r + n_lag_dy + 1;

        let v_hat = if has_const {
            Col::from_iter((0..k).map(|i| gamma_full[(const_row, i)]))
        } else {
            Col::zeros(k)
        };
        let delta_hat = if has_trend {
            Col::from_iter((0..k).map(|i| gamma_full[(trend_row, i)]))
        } else {
            Col::zeros(k)
        };

        let alpha_aa = alpha_mat.transpose() * alpha_mat.as_ref();
        let alpha_aa_inv = alpha_aa
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "VEC: alpha'alpha singular".to_string())?
            .solve(&Mat::<f64>::identity(r, r));
        if has_const && config.trend_spec == VecTrendSpec::Constant {
            let alpha_t_v = alpha_mat.transpose() * v_hat.as_ref();
            let v_col = Mat::from_fn(r, 1, |i, _| alpha_t_v[i]);
            let mu_col = alpha_aa_inv.as_ref() * v_col.as_ref();
            mu_rho.extend((0..r).map(|i| mu_col[(i, 0)]));
        }
        if has_trend {
            let alpha_t_d = alpha_mat.transpose() * delta_hat.as_ref();
            let d_col = Mat::from_fn(r, 1, |i, _| alpha_t_d[i]);
            let rho_col = alpha_aa_inv.as_ref() * d_col.as_ref();
            mu_rho.extend((0..r).map(|i| rho_col[(i, 0)]));
        }
    }

    // Stata uses demeaned CE: Ê_{t-1} = β'y_{t-1} + μ + ρ(t-1) (not r1 = residual of Z1 on Z2)
    let n_ce = r;
    let mut ce_vals = Mat::zeros(n, n_ce);
    for i in 0..n {
        let t_lag = (p + i - 1) as f64; // t-1 for Ê_{t-1}
        for j in 0..r {
            ce_vals[(i, j)] = (0..k).map(|kk| z1[(i, kk)] * beta_y[(kk, j)]).sum::<f64>()
                + mu_rho.get(j).copied().unwrap_or(0.0)
                + if has_trend {
                    mu_rho.get(r + j).copied().unwrap_or(0.0) * t_lag
                } else {
                    0.0
                };
        }
    }

    let n_z_sr = r + m2;
    let mut x_sr = Mat::zeros(n, n_z_sr);
    for i in 0..n {
        for j in 0..r {
            x_sr[(i, j)] = ce_vals[(i, j)];
        }
        for j in 0..m2 {
            x_sr[(i, r + j)] = z2[(i, j)];
        }
    }

    let xt = x_sr.transpose();
    let xtx = xt.as_ref() * x_sr.as_ref();
    let xtx_inv = xtx
        .as_ref()
        .checked_cholesky()
        .map_err(|_| "VEC: short-run X'X not positive definite".to_string())?
        .solve(&Mat::<f64>::identity(xtx.nrows(), xtx.nrows()));

    let mut coefficients = Vec::with_capacity(k);
    let mut std_errs = Vec::with_capacity(k);
    let mut residuals = Vec::with_capacity(k);
    let mut ss_res = Vec::with_capacity(k);
    let mut ss_tot = Vec::with_capacity(k);
    let mut cov_beta = Vec::with_capacity(k);
    let mut coef_labels = Vec::with_capacity(k);

    let sigma2_divisor = (n as f64 - d as f64).max(1.0);

    for eq in 0..k {
        let y_col = z0.col(eq).to_owned();
        let xty = xt.as_ref() * y_col.as_ref();
        let beta_sr = xtx_inv.as_ref() * xty.as_ref();
        let y_hat = x_sr.as_ref() * beta_sr.as_ref();
        let u = y_col.as_ref() - y_hat.as_ref();

        let ss_r: f64 = u.iter().map(|x| x * x).sum();
        let y_mean = y_col.iter().sum::<f64>() / y_col.nrows().max(1) as f64;
        let ss_t: f64 = y_col.iter().map(|x| (x - y_mean).powi(2)).sum();
        // R² = 1 - RSS/TSS per Stata reg3; TSS = Σ(Δy-Δȳ)² (standard formula)
        let ss_t_final: f64 = ss_t;

        let sigma2_eq = ss_r / sigma2_divisor;
        let cov_eq = yss_sci_linalg::Scale(sigma2_eq) * &xtx_inv;
        let se: Col<f64> = Col::from_fn(cov_eq.nrows().min(cov_eq.ncols()), |i| {
            cov_eq[(i, i)].sqrt()
        });
        cov_beta.push(cov_eq);

        let mut labels = Vec::with_capacity(n_z_sr);
        for j in 0..r {
            labels.push(format!("_ce{}_L1.", j + 1));
        }
        for lag in 1..p {
            for j in 0..k {
                let name = var_names
                    .get(j)
                    .cloned()
                    .unwrap_or_else(|| format!("y{}", j));
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

        coefficients.push(beta_sr.iter().copied().collect::<Vec<_>>());
        std_errs.push(se.iter().copied().collect::<Vec<_>>());
        residuals.push(u.iter().copied().collect::<Vec<_>>());
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
            let beta = Col::from_iter(coefficients[eq].iter().copied());
            let v = &cov_beta[eq];
            match v.as_ref().checked_cholesky() {
                Ok(llt) => {
                    let x = llt.solve(&beta.as_ref());
                    beta.transpose() * x.as_ref()
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
            var_names
                .get(eq)
                .cloned()
                .unwrap_or_else(|| format!("y{}", eq))
        );
        equations.push(EquationStatistics {
            eq_name,
            parms: n_z_sr,
            rmse,
            r_sq,
            chi2,
            p_chi2,
        });
    }

    let mut beta_out: Vec<Vec<f64>> = (0..k)
        .map(|i| (0..r).map(|j| beta_y[(i, j)]).collect())
        .collect();
    if has_const && mu_rho.len() >= r {
        beta_out.push((0..r).map(|j| mu_rho[j]).collect());
    }

    // Cointegrating equations chi2 (Stata formula: Wald on free params in beta)
    let cointegrating_equations =
        compute_cointegrating_equations_chi2(&beta_y, &alpha_mat, &omega, &s11, n, d);

    // beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]（Stata 公式 15）
    let (
        mut beta_std_err,
        mut beta_z_value,
        mut beta_p_value,
        mut beta_ci_lower,
        mut beta_ci_upper,
    ) = compute_beta_ce_stats(&beta_y, &alpha_mat, &omega, &s11, n, d);
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

    let inference = (0..k)
        .map(|eq| RegressionCoefficientStatistics {
            covariance: covariance_rows(&cov_beta[eq]),
            standard_errors: std::mem::take(&mut std_errs[eq]),
            statistic_values: std::mem::take(&mut z_values[eq]),
            p_values: std::mem::take(&mut p_values[eq]),
            confidence_interval_lower: std::mem::take(&mut ci_lower[eq]),
            confidence_interval_upper: std::mem::take(&mut ci_upper[eq]),
        })
        .collect();
    Ok(VecFit {
        var_names,
        rank: r,
        lags: p,
        trend_spec: trend_spec_str.into(),
        coefficients,
        residuals,
        design: covariance_rows(&x_sr),
        statistics: MultivariateStatistics {
            observations: n,
            log_likelihood: ll,
            aic,
            hqic,
            sbic,
            det_sigma_ml,
            equations,
            coefficients: inference,
            coefficient_labels: coef_labels,
        },
        cointegration: CointegrationStatistics {
            beta: beta_out,
            equations: cointegrating_equations,
            standard_errors: beta_std_err,
            statistic_values: beta_z_value,
            p_values: beta_p_value,
            confidence_interval_lower: beta_ci_lower,
            confidence_interval_upper: beta_ci_upper,
        },
    })
}
