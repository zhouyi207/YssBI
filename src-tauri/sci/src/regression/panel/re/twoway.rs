/// Two-way within: z̃_it = z_it - z̄_i - z̄_t + z̄
fn within_transform_twoway(v: &[f64], entity_id: &[usize], time_id: &[usize]) -> Array1<f64> {
    let n = v.len();
    let z_bar_i = between_transform(v, entity_id);
    let z_bar_t = between_transform(v, time_id);
    let z_bar: f64 = v.iter().sum::<f64>() / n as f64;
    Array1::from_shape_fn(n, |i| v[i] - z_bar_i[i] - z_bar_t[i] + z_bar)
}

/// Panel RE FGLS (Two-Way): entity + time random effects (Fuller-Battese / plm-style)
pub fn fit_panel_re_fgls_twoway(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel RE (Two-Way FGLS): lengths must match".to_string());
    }
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 || n_times < 2 {
        return Err("Panel RE (Two-Way FGLS): need at least 2 entities and 2 time periods".to_string());
    }

    let (obs_per_entity, t_bar_entity) = obs_per_group_and_harmonic_mean(entity_id);
    let (_obs_per_time, n_bar_time) = obs_per_group_and_harmonic_mean(time_id);

    let k = exog.ncols();
    let y_vec: Vec<f64> = endog.iter().cloned().collect();

    // Step 1: Two-way within regression → σ²_e
    let y_w = within_transform_twoway(&y_vec, entity_id, time_id);
    let mut x_w = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let tc = within_transform_twoway(&col, entity_id, time_id);
        for i in 0..n {
            x_w[[i, c]] = tc[i];
        }
    }

    let (x_w_use, _omitted_w) = {
        let (x_after_const, _) = if constant && k > 0 {
            let first_col = x_w.column(0);
            let is_const = first_col.iter().all(|&v| v.abs() < 1e-10);
            if is_const {
                (x_w.slice(ndarray::s![.., 1..]).to_owned(), false)
            } else {
                (x_w.clone(), constant)
            }
        } else {
            (x_w.clone(), constant)
        };
        let k_ac = x_after_const.ncols();
        let col_is_dummy = vec![false; k_ac];
        drop_collinear_columns(&x_after_const, &col_is_dummy, None)
            .map_err(|e| format!("Panel RE (Two-Way) within: {}", e))?
    };
    let k_w = x_w_use.ncols();

    let ols_w = OLS {
        endog: y_w.clone(),
        exog: x_w_use,
        config: crate::regression::linear_model::OLSConfig {
            constant: false,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let res_w = ols_w.fit().map_err(|e| format!("Panel RE (Two-Way) within: {}", e))?;
    let df_w = (n as i64 - n_entities as i64 - n_times as i64 + 1 - k_w as i64).max(1) as usize;
    let sigma2_e = res_w.ss_residual / df_w as f64;

    let t_bar = if t_bar_entity > 1e-300 {
        t_bar_entity
    } else {
        n as f64 / n_entities as f64
    };
    let n_bar = if n_bar_time > 1e-300 {
        n_bar_time
    } else {
        n as f64 / n_times as f64
    };

    // Step 2: Between-entity regression → σ²_α
    let (_, y_b_e, x_b_e) = group_means(&y_vec, exog, entity_id);
    let n_b_e = y_b_e.len();
    let mut x_b_e_data = Vec::with_capacity(n_b_e * k);
    for i in 0..n_b_e {
        for c in 0..k {
            x_b_e_data.push(x_b_e[i][c]);
        }
    }
    let y_b_e_arr = Array1::from_vec(y_b_e);
    let x_b_e_arr = Array2::from_shape_vec((n_b_e, k), x_b_e_data)
        .map_err(|e| format!("Panel RE (Two-Way) between-entity: {:?}", e))?;
    let (x_b_e_use, _) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b_e_arr, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (Two-Way) between-entity: {}", e))?
    };
    let res_b_e = OLS {
        endog: y_b_e_arr,
        exog: x_b_e_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way) between-entity: {}", e))?;
    let df_b_e = res_b_e.df_residual;
    let sigma2_alpha = if df_b_e > 0 {
        (res_b_e.ss_residual / df_b_e as f64 - sigma2_e / t_bar).max(0.0)
    } else {
        0.0
    };

    // Step 3: Between-time regression → σ²_λ
    let (_, y_b_t, x_b_t) = group_means(&y_vec, exog, time_id);
    let n_b_t = y_b_t.len();
    let mut x_b_t_data = Vec::with_capacity(n_b_t * k);
    for i in 0..n_b_t {
        for c in 0..k {
            x_b_t_data.push(x_b_t[i][c]);
        }
    }
    let y_b_t_arr = Array1::from_vec(y_b_t);
    let x_b_t_arr = Array2::from_shape_vec((n_b_t, k), x_b_t_data)
        .map_err(|e| format!("Panel RE (Two-Way) between-time: {:?}", e))?;
    let (x_b_t_use, _) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b_t_arr, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (Two-Way) between-time: {}", e))?
    };
    let res_b_t = OLS {
        endog: y_b_t_arr,
        exog: x_b_t_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way) between-time: {}", e))?;
    let df_b_t = res_b_t.df_residual;
    let sigma2_lambda = if df_b_t > 0 {
        (res_b_t.ss_residual / df_b_t as f64 - sigma2_e / n_bar).max(0.0)
    } else {
        0.0
    };

    if sigma2_alpha <= 0.0 && sigma2_lambda <= 0.0 {
        return Err("Panel RE (Two-Way): both sigma_alpha^2 and sigma_lambda^2 <= 0".to_string());
    }

    // Theta (plm-style): θ_id = 1 - (1 + T·σ²_α/σ²_e)^(-0.5), θ_time = 1 - (1 + N·σ²_λ/σ²_e)^(-0.5)
    let ratio_alpha = (t_bar * sigma2_alpha / sigma2_e.max(1e-300)).max(0.0);
    let ratio_lambda = (n_bar * sigma2_lambda / sigma2_e.max(1e-300)).max(0.0);
    let theta_id = 1.0 - (1.0 + ratio_alpha).powf(-0.5);
    let theta_time = 1.0 - (1.0 + ratio_lambda).powf(-0.5);
    let theta_total = theta_id + theta_time - 1.0
        + (1.0 + ratio_alpha + ratio_lambda).powf(-0.5);

    let y_bar_i = between_transform(&y_vec, entity_id);
    let y_bar_t = between_transform(&y_vec, time_id);
    let y_bar: f64 = y_vec.iter().sum::<f64>() / n as f64;

    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| {
        y_vec[i] - theta_id * y_bar_i[i] - theta_time * y_bar_t[i] + theta_total * y_bar
    });

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar_i = between_transform(&col, entity_id);
        let x_bar_t = between_transform(&col, time_id);
        let x_bar: f64 = col.iter().sum::<f64>() / n as f64;
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta_id * x_bar_i[i] - theta_time * x_bar_t[i] + theta_total * x_bar;
        }
    }

    let (x_star_use, omitted_final) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (Two-Way FGLS) final: {}", e))?
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(crate::regression::covariance::CovParams::Cluster {
                cluster_id: entity_id.to_vec(),
                xtreg_fe_style: false,
            })
        } else {
            None
        }
    });
    let config = crate::regression::linear_model::OLSConfig {
        constant,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols_re = OLS {
        endog: y_star,
        exog: x_star_use,
        config,
    };

    let result = ols_re.fit()?;

    let kept: Vec<usize> = (0..k).filter(|j| !omitted_final.contains(j)).collect();
    let omitted_indices = if omitted_final.is_empty() {
        None
    } else {
        Some(omitted_final)
    };
    let betas = &result.betas;

    let obs_per_grp: Vec<usize> = obs_per_entity
        .values()
        .copied()
        .collect();
    let obs_min = obs_per_grp.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_grp.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_grp.iter().sum::<usize>() as f64 / n_entities as f64;

    let r2_within = {
        let y_w: Vec<f64> = within_transform_twoway(&y_vec, entity_id, time_id)
            .iter()
            .cloned()
            .collect();
        let xb: Vec<f64> = (0..n)
            .map(|i| kept.iter().enumerate().map(|(idx, &c)| exog[[i, c]] * betas[idx]).sum())
            .collect();
        let xb_bar_i = between_transform(&xb, entity_id);
        let xb_bar_t = between_transform(&xb, time_id);
        let xb_bar: f64 = xb.iter().sum::<f64>() / n as f64;
        let xb_w: Vec<f64> = (0..n).map(|i| xb[i] - xb_bar_i[i] - xb_bar_t[i] + xb_bar).collect();
        let (y_mean, xb_mean) = (
            y_w.iter().sum::<f64>() / n as f64,
            xb_w.iter().sum::<f64>() / n as f64,
        );
        let cov = y_w
            .iter()
            .zip(xb_w.iter())
            .map(|(y, x)| (y - y_mean) * (x - xb_mean))
            .sum::<f64>()
            / (n as f64 - 1.0).max(1.0);
        let (var_y, var_xb) = (
            y_w.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
            xb_w.iter().map(|x| (x - xb_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
        );
        if (var_y * var_xb).sqrt() > 1e-300 {
            (cov / (var_y * var_xb).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    let r2_between = {
        let (_, y_b_vec, x_b_vec) = group_means(&y_vec, exog, entity_id);
        let n_b = y_b_vec.len();
        let y_mean = y_b_vec.iter().sum::<f64>() / n_b as f64;
        let xb_b: Vec<f64> = (0..n_b)
            .map(|i| kept.iter().enumerate().map(|(idx, &c)| x_b_vec[i][c] * betas[idx]).sum())
            .collect();
        let xb_mean = xb_b.iter().sum::<f64>() / n_b as f64;
        let cov = y_b_vec
            .iter()
            .zip(xb_b.iter())
            .map(|(y, x)| (y - y_mean) * (x - xb_mean))
            .sum::<f64>()
            / (n_b as f64 - 1.0).max(1.0);
        let (var_y, var_xb) = (
            y_b_vec.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n_b as f64 - 1.0).max(1.0),
            xb_b.iter().map(|x| (x - xb_mean).powi(2)).sum::<f64>() / (n_b as f64 - 1.0).max(1.0),
        );
        if (var_y * var_xb).sqrt() > 1e-300 {
            (cov / (var_y * var_xb).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    let r2_overall = {
        let xb_obs: Vec<f64> = (0..n)
            .map(|i| kept.iter().enumerate().map(|(idx, &c)| exog[[i, c]] * betas[idx]).sum())
            .collect();
        let (xb_mean, y_mean) = (
            xb_obs.iter().sum::<f64>() / n as f64,
            endog.iter().sum::<f64>() / n as f64,
        );
        let cov = xb_obs
            .iter()
            .zip(endog.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean))
            .sum::<f64>()
            / (n as f64 - 1.0).max(1.0);
        let (var_xb, var_y) = (
            xb_obs.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
            endog.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
        );
        if (var_xb * var_y).sqrt() > 1e-300 {
            (cov / (var_xb * var_y).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    let sigma_u = (sigma2_alpha + sigma2_lambda).sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = (sigma2_alpha + sigma2_lambda) / (sigma2_alpha + sigma2_lambda + sigma2_e);

    let fe_stats = Some(super::PanelFEStats {
        r2: Some(super::PanelR2Stats {
            r2_within,
            r2_between,
            r2_overall,
        }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb: 0.0,
        theta: None,
    });

    let (wald_chi2, prob_wald_chi2) = {
        let cov_beta = &result.cov_beta;
        let betas_nd = &result.betas;
        let k_b = betas_nd.len();
        let (beta_s, v_s, df_wald) = if constant && k_b > 1 {
            (
                betas_nd.slice(ndarray::s![1..]).to_owned(),
                cov_beta.slice(ndarray::s![1.., 1..]).to_owned(),
                k_b - 1,
            )
        } else {
            (betas_nd.clone(), cov_beta.clone(), k_b)
        };
        let v_s_faer = v_s.view().into_faer().to_owned();
        let beta_s_faer = beta_s.view().into_faer_col().to_owned();
        let x = v_s_faer
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "Panel RE (Two-Way) Wald".to_string())?
            .solve(beta_s_faer.as_ref());
        let x_nd = x.as_ref().into_ndarray();
        let wald = beta_s.dot(&x_nd);
        let chi2_dist =
            ChiSquared::new(df_wald as f64).map_err(|e| format!("Panel RE (Two-Way) Wald: {}", e))?;
        (wald, 1.0 - chi2_dist.cdf(wald))
    };

    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("Panel RE (Two-Way): {}", e))?;
    let pvalues_z: Array1<f64> = Array1::from_shape_fn(result.tvalues.len(), |i| {
        2.0 * (1.0 - std_normal.cdf(result.tvalues[i].abs()))
    });
    let z_crit = std_normal.inverse_cdf(0.975);
    let conf_int_left_z = &result.betas - z_crit * &result.stds;
    let conf_int_right_z = &result.betas + z_crit * &result.stds;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: n,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model: result.df_model,
        df_residual: result.df_residual,
        df_total: result.df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total: result.ms_total,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: None,
        fvalue: result.fvalue,
        f_p_value: result.f_p_value,
        betas: result.betas,
        stds: result.stds,
        tvalues: result.tvalues,
        pvalues: pvalues_z,
        conf_int_left: conf_int_left_z,
        conf_int_right: conf_int_right_z,
        cov_beta: result.cov_beta,
        cov_beta_nonrobust: Some(result.cov_beta_nonrobust),
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: Some(wald_chi2),
        prob_wald_chi2: Some(prob_wald_chi2),
        log_likelihood: None,
        lr_chi2: None,
        prob_lr_chi2: None,
        chibar2: None,
        prob_chibar2: None,
        mle_iter_log_lik_const: None,
        mle_iter_log_lik: None,
    })
}

/// Compute theta (quasi-demean weights) from two-way variance components.
fn twoway_theta(
    sigma2_alpha: f64,
    sigma2_lambda: f64,
    sigma2_e: f64,
    t_bar: f64,
    n_bar: f64,
) -> (f64, f64, f64) {
    let se = sigma2_e.max(1e-300);
    let ratio_alpha = (t_bar * sigma2_alpha / se).max(0.0);
    let ratio_lambda = (n_bar * sigma2_lambda / se).max(0.0);
    let theta_id = 1.0 - (1.0 + ratio_alpha).powf(-0.5);
    let theta_time = 1.0 - (1.0 + ratio_lambda).powf(-0.5);
    let theta_total =
        theta_id + theta_time - 1.0 + (1.0 + ratio_alpha + ratio_lambda).powf(-0.5);
    (theta_id, theta_time, theta_total)
}

/// Two-way RE log-likelihood: LL = -0.5 [n ln(2π) + ln|Ω| + r'Ω^{-1}r].
/// Uses quasi-demeaned SSR for r'Ω^{-1}r = SSR_star/σ²_e; ln|Ω| from balanced approximation.
fn re_mle_log_lik_twoway(
    endog: &[f64],
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    betas: &[f64],
    kept: &[usize],
    sigma2_alpha: f64,
    sigma2_lambda: f64,
    sigma2_e: f64,
    n_entities: usize,
    n_times: usize,
    t_bar: f64,
    n_bar: f64,
) -> f64 {
    let n = endog.len();
    let se = sigma2_e.max(1e-300);
    let sa = sigma2_alpha.max(1e-12);
    let sl = sigma2_lambda.max(1e-12);

    let r: Vec<f64> = (0..n)
        .map(|i| {
            endog[i]
                - kept
                    .iter()
                    .enumerate()
                    .map(|(_j, &c)| exog[[i, c]] * betas[c])
                    .sum::<f64>()
        })
        .collect();

    let (theta_id, theta_time, theta_total) = twoway_theta(sa, sl, se, t_bar, n_bar);
    let r_bar_i = between_transform(&r, entity_id);
    let r_bar_t = between_transform(&r, time_id);
    let r_bar: f64 = r.iter().sum::<f64>() / n as f64;
    let r_star: Vec<f64> = (0..n)
        .map(|i| r[i] - theta_id * r_bar_i[i] - theta_time * r_bar_t[i] + theta_total * r_bar)
        .collect();
    let ssr_star: f64 = r_star.iter().map(|x| x * x).sum();

    let ln_omega = (t_bar * sa + n_bar * sl + se).ln()
        + (n_entities as f64 - 1.0).max(0.0) * (t_bar * sa + se).ln()
        + (n_times as f64 - 1.0).max(0.0) * (n_bar * sl + se).ln()
        + ((n_entities as f64 - 1.0) * (n_times as f64 - 1.0)).max(0.0) * se.ln();

    -0.5 * (n as f64 * (std::f64::consts::PI * 2.0).ln() + ln_omega + ssr_star / se)
}

/// Panel RE MLE (Two-Way): iterative GLS with variance component updates (MLE-style).
pub fn fit_panel_re_mle_twoway(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel RE (Two-Way MLE): lengths must match".to_string());
    }
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 || n_times < 2 {
        return Err("Panel RE (Two-Way MLE): need at least 2 entities and 2 time periods".to_string());
    }

    let (obs_per_entity, t_bar_entity) = obs_per_group_and_harmonic_mean(entity_id);
    let (_obs_per_time, n_bar_time) = obs_per_group_and_harmonic_mean(time_id);
    let t_bar = if t_bar_entity > 1e-300 {
        t_bar_entity
    } else {
        n as f64 / n_entities as f64
    };
    let n_bar = if n_bar_time > 1e-300 {
        n_bar_time
    } else {
        n as f64 / n_times as f64
    };

    let k = exog.ncols();
    let y_vec: Vec<f64> = endog.iter().cloned().collect();

    // Initial variance components from within/between (same as FGLS)
    let y_w = within_transform_twoway(&y_vec, entity_id, time_id);
    let mut x_w = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let tc = within_transform_twoway(&col, entity_id, time_id);
        for i in 0..n {
            x_w[[i, c]] = tc[i];
        }
    }
    let (x_w_use, _) = {
        let x_ac = if constant && k > 0 {
            let fc = x_w.column(0);
            if fc.iter().all(|&v| v.abs() < 1e-10) {
                x_w.slice(ndarray::s![.., 1..]).to_owned()
            } else {
                x_w.clone()
            }
        } else {
            x_w.clone()
        };
        let k_ac = x_ac.ncols();
        drop_collinear_columns(&x_ac, &vec![false; k_ac], None)
            .map_err(|e| format!("Panel RE (Two-Way MLE) within: {}", e))?
    };
    let k_w = x_w_use.ncols();
    let res_w = OLS {
        endog: y_w.clone(),
        exog: x_w_use,
        config: crate::regression::linear_model::OLSConfig {
            constant: false,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way MLE) within: {}", e))?;
    let df_w = (n as i64 - n_entities as i64 - n_times as i64 + 1 - k_w as i64).max(1) as usize;
    let mut sigma2_e = (res_w.ss_residual / df_w as f64).max(1e-12);

    let (_, y_b_e, x_b_e) = group_means(&y_vec, exog, entity_id);
    let n_b_e = y_b_e.len();
    let mut x_b_e_data = Vec::with_capacity(n_b_e * k);
    for i in 0..n_b_e {
        for c in 0..k {
            x_b_e_data.push(x_b_e[i][c]);
        }
    }
    let x_b_e_arr = Array2::from_shape_vec((n_b_e, k), x_b_e_data)
        .map_err(|_| "Panel RE (Two-Way MLE) between-entity")?;
    let (x_b_e_use, _) = drop_collinear_columns(
        &x_b_e_arr,
        &vec![false; k],
        if constant { Some(0) } else { None },
    )
    .map_err(|e| format!("Panel RE (Two-Way MLE) between-entity: {}", e))?;
    let res_b_e = OLS {
        endog: Array1::from_vec(y_b_e.clone()),
        exog: x_b_e_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way MLE) between-entity: {}", e))?;
    let df_b_e = res_b_e.df_residual;
    let mut sigma2_alpha =
        (res_b_e.ss_residual / df_b_e as f64 - sigma2_e / t_bar).max(1e-10);

    let (_, y_b_t, x_b_t) = group_means(&y_vec, exog, time_id);
    let n_b_t = y_b_t.len();
    let x_b_t_arr = Array2::from_shape_vec((n_b_t, k), {
        let mut v = Vec::with_capacity(n_b_t * k);
        for i in 0..n_b_t {
            for c in 0..k {
                v.push(x_b_t[i][c]);
            }
        }
        v
    })
    .map_err(|_| "Panel RE (Two-Way MLE) between-time")?;
    let (x_b_t_use, _) = drop_collinear_columns(
        &x_b_t_arr,
        &vec![false; k],
        if constant { Some(0) } else { None },
    )
    .map_err(|e| format!("Panel RE (Two-Way MLE) between-time: {}", e))?;
    let res_b_t = OLS {
        endog: Array1::from_vec(y_b_t.clone()),
        exog: x_b_t_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way MLE) between-time: {}", e))?;
    let df_b_t = res_b_t.df_residual;
    let mut sigma2_lambda =
        (res_b_t.ss_residual / df_b_t as f64 - sigma2_e / n_bar).max(1e-10);

    if sigma2_alpha <= 0.0 && sigma2_lambda <= 0.0 {
        return Err("Panel RE (Two-Way MLE): both sigma_alpha^2 and sigma_lambda^2 <= 0".to_string());
    }

    // Constant-only model for ll_null
    let y_global_mean = y_vec.iter().sum::<f64>() / n as f64;
    let mut mle_iter_log_lik_const: Vec<f64> = Vec::new();
    {
        let (theta_id, theta_time, theta_total) =
            twoway_theta(sigma2_alpha, sigma2_lambda, sigma2_e, t_bar, n_bar);
        let y_bar_i = between_transform(&y_vec, entity_id);
        let y_bar_t = between_transform(&y_vec, time_id);
        let y_star_const: Array1<f64> =
            Array1::from_shape_fn(n, |i| y_vec[i] - theta_id * y_bar_i[i] - theta_time * y_bar_t[i]
                + theta_total * y_global_mean);
        let x_const_star: Vec<f64> = (0..n)
            .map(|_| 1.0 - theta_id - theta_time + theta_total)
            .collect();
        let x_const = Array2::from_shape_vec((n, 1), x_const_star)
            .map_err(|_| "Panel RE (Two-Way MLE) const")?;
        let res_const = OLS {
            endog: y_star_const,
            exog: x_const,
            config: crate::regression::linear_model::OLSConfig {
                constant: true,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        }
        .fit()
        .map_err(|e| format!("Panel RE (Two-Way MLE) const: {}", e))?;
        let alpha_init = res_const.betas[0];
        let mut ll_const = re_mle_log_lik_twoway(
            &y_vec,
            exog,
            entity_id,
            time_id,
            &[alpha_init],
            &[0],
            sigma2_alpha,
            sigma2_lambda,
            sigma2_e,
            n_entities,
            n_times,
            t_bar,
            n_bar,
        );
        mle_iter_log_lik_const.push(ll_const);
        for _ in 0..30 {
            let (theta_id, theta_time, theta_total) =
                twoway_theta(sigma2_alpha, sigma2_lambda, sigma2_e, t_bar, n_bar);
            let y_bar_i = between_transform(&y_vec, entity_id);
            let y_bar_t = between_transform(&y_vec, time_id);
            let y_star_c: Array1<f64> = Array1::from_shape_fn(n, |i| {
                y_vec[i] - theta_id * y_bar_i[i] - theta_time * y_bar_t[i]
                    + theta_total * y_global_mean
            });
            let x_c: Vec<f64> = (0..n).map(|_| 1.0 - theta_id - theta_time + theta_total).collect();
            let x_c_arr = Array2::from_shape_vec((n, 1), x_c).unwrap();
            let res_c = OLS {
                endog: y_star_c,
                exog: x_c_arr,
                config: crate::regression::linear_model::OLSConfig {
                    constant: true,
                    cov_type: "nonrobust".to_string(),
                    cov_params: None,
                },
            }
            .fit()
            .unwrap();
            let alpha = res_c.betas[0];
            let r: Vec<f64> = (0..n).map(|i| y_vec[i] - alpha).collect();
            let r_w = within_transform_twoway(&r, entity_id, time_id);
            let ss_w: f64 = r_w.iter().map(|x| x * x).sum();
            sigma2_e = (ss_w / df_w as f64).max(1e-12);
            let (_, r_b_e, _) = group_means(&r, exog, entity_id);
            let var_b_e = if n_b_e > 1 {
                let m: f64 = r_b_e.iter().sum::<f64>() / n_b_e as f64;
                r_b_e.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n_b_e - 1) as f64
            } else {
                0.0
            };
            sigma2_alpha = (var_b_e - sigma2_e / t_bar).max(1e-10);
            let (_, r_b_t, _) = group_means(&r, exog, time_id);
            let var_b_t = if n_b_t > 1 {
                let m: f64 = r_b_t.iter().sum::<f64>() / n_b_t as f64;
                r_b_t.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n_b_t - 1) as f64
            } else {
                0.0
            };
            sigma2_lambda = (var_b_t - sigma2_e / n_bar).max(1e-10);
            ll_const = re_mle_log_lik_twoway(
                &y_vec,
                exog,
                entity_id,
                time_id,
                &[alpha],
                &[0],
                sigma2_alpha,
                sigma2_lambda,
                sigma2_e,
                n_entities,
                n_times,
                t_bar,
                n_bar,
            );
            mle_iter_log_lik_const.push(ll_const);
        }
    }
    let ll_null = *mle_iter_log_lik_const.last().unwrap_or(&f64::NEG_INFINITY);

    // Re-init for full model from FGLS logic (variance components may have changed from const-only)
    let y_w2 = within_transform_twoway(&y_vec, entity_id, time_id);
    let mut x_w2 = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let tc = within_transform_twoway(&col, entity_id, time_id);
        for i in 0..n {
            x_w2[[i, c]] = tc[i];
        }
    }
    let (x_w2_use, _) = {
        let x_ac = if constant && k > 0 {
            let fc = x_w2.column(0);
            if fc.iter().all(|&v| v.abs() < 1e-10) {
                x_w2.slice(ndarray::s![.., 1..]).to_owned()
            } else {
                x_w2.clone()
            }
        } else {
            x_w2.clone()
        };
        let k_ac = x_ac.ncols();
        drop_collinear_columns(&x_ac, &vec![false; k_ac], None)
            .map_err(|e| format!("Panel RE (Two-Way MLE): {}", e))?
    };
    let res_w2 = OLS {
        endog: y_w2,
        exog: x_w2_use,
        config: crate::regression::linear_model::OLSConfig {
            constant: false,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()
    .map_err(|e| format!("Panel RE (Two-Way MLE): {}", e))?;
    sigma2_e = (res_w2.ss_residual / df_w as f64).max(1e-12);
    sigma2_alpha =
        (res_b_e.ss_residual / df_b_e as f64 - sigma2_e / t_bar).max(1e-10);
    sigma2_lambda =
        (res_b_t.ss_residual / df_b_t as f64 - sigma2_e / n_bar).max(1e-10);

    let mut kept: Vec<usize> = (0..k).collect();
    let mut betas: Vec<f64> = vec![0.0; k];
    let mut mle_iter_log_lik: Vec<f64> = Vec::new();

    for iter in 0..80 {
        let (theta_id, theta_time, theta_total) =
            twoway_theta(sigma2_alpha, sigma2_lambda, sigma2_e, t_bar, n_bar);
        let y_bar_i = between_transform(&y_vec, entity_id);
        let y_bar_t = between_transform(&y_vec, time_id);
        let y_bar: f64 = y_vec.iter().sum::<f64>() / n as f64;
        let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| {
            y_vec[i] - theta_id * y_bar_i[i] - theta_time * y_bar_t[i] + theta_total * y_bar
        });
        let mut x_star = Array2::zeros((n, k));
        for c in 0..k {
            let col: Vec<f64> = exog.column(c).iter().cloned().collect();
            let x_bar_i = between_transform(&col, entity_id);
            let x_bar_t = between_transform(&col, time_id);
            let x_bar: f64 = col.iter().sum::<f64>() / n as f64;
            for i in 0..n {
                x_star[[i, c]] =
                    col[i] - theta_id * x_bar_i[i] - theta_time * x_bar_t[i] + theta_total * x_bar;
            }
        }
        let (x_star_use, omitted) = drop_collinear_columns(
            &x_star,
            &vec![false; k],
            if constant { Some(0) } else { None },
        )
        .map_err(|e| format!("Panel RE (Two-Way MLE): {}", e))?;
        kept = (0..k).filter(|j| !omitted.contains(j)).collect();
        let res = OLS {
            endog: y_star,
            exog: x_star_use,
            config: crate::regression::linear_model::OLSConfig {
                constant,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        }
        .fit()
        .map_err(|e| format!("Panel RE (Two-Way MLE): {}", e))?;
        betas = vec![0.0; k];
        for (idx, &c) in kept.iter().enumerate() {
            betas[c] = res.betas[idx];
        }
        let ll = re_mle_log_lik_twoway(
            &y_vec,
            exog,
            entity_id,
            time_id,
            &betas,
            &kept,
            sigma2_alpha,
            sigma2_lambda,
            sigma2_e,
            n_entities,
            n_times,
            t_bar,
            n_bar,
        );
        mle_iter_log_lik.push(ll);

        let r: Vec<f64> = (0..n)
            .map(|i| y_vec[i] - kept.iter().enumerate().map(|(_j, &c)| exog[[i, c]] * betas[c]).sum::<f64>())
            .collect();
        let r_w = within_transform_twoway(&r, entity_id, time_id);
        let ss_w: f64 = r_w.iter().map(|x| x * x).sum();
        let sigma2_e_new = (ss_w / df_w as f64).max(1e-12);
        let (_, r_b_e, _) = group_means(&r, exog, entity_id);
        let var_b_e = if n_b_e > 1 {
            let m: f64 = r_b_e.iter().sum::<f64>() / n_b_e as f64;
            r_b_e.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n_b_e - 1) as f64
        } else {
            0.0
        };
        let sigma2_alpha_new = (var_b_e - sigma2_e_new / t_bar).max(1e-10);
        let (_, r_b_t, _) = group_means(&r, exog, time_id);
        let var_b_t = if n_b_t > 1 {
            let m: f64 = r_b_t.iter().sum::<f64>() / n_b_t as f64;
            r_b_t.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n_b_t - 1) as f64
        } else {
            0.0
        };
        let sigma2_lambda_new = (var_b_t - sigma2_e_new / n_bar).max(1e-10);

        let tol = 1e-8;
        if (sigma2_e_new - sigma2_e).abs() < tol
            && (sigma2_alpha_new - sigma2_alpha).abs() < tol
            && (sigma2_lambda_new - sigma2_lambda).abs() < tol
        {
            sigma2_e = sigma2_e_new;
            sigma2_alpha = sigma2_alpha_new;
            sigma2_lambda = sigma2_lambda_new;
            break;
        }
        sigma2_e = sigma2_e_new;
        sigma2_alpha = sigma2_alpha_new;
        sigma2_lambda = sigma2_lambda_new;
        if iter == 79 {
            return Err("Panel RE (Two-Way MLE): did not converge".to_string());
        }
    }

    let (theta_id, theta_time, theta_total) =
        twoway_theta(sigma2_alpha, sigma2_lambda, sigma2_e, t_bar, n_bar);
    let y_bar_i = between_transform(&y_vec, entity_id);
    let y_bar_t = between_transform(&y_vec, time_id);
    let y_bar: f64 = y_vec.iter().sum::<f64>() / n as f64;
    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| {
        y_vec[i] - theta_id * y_bar_i[i] - theta_time * y_bar_t[i] + theta_total * y_bar
    });
    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar_i = between_transform(&col, entity_id);
        let x_bar_t = between_transform(&col, time_id);
        let x_bar: f64 = col.iter().sum::<f64>() / n as f64;
        for i in 0..n {
            x_star[[i, c]] =
                col[i] - theta_id * x_bar_i[i] - theta_time * x_bar_t[i] + theta_total * x_bar;
        }
    }
    let (x_star_use, omitted_mle) = drop_collinear_columns(
        &x_star,
        &vec![false; k],
        if constant { Some(0) } else { None },
    )
    .map_err(|e| format!("Panel RE (Two-Way MLE): {}", e))?;

    let mut result = OLS {
        endog: y_star,
        exog: x_star_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    }
    .fit()?;

    kept = (0..k).filter(|j| !omitted_mle.contains(j)).collect();
    let omitted_indices = if omitted_mle.is_empty() {
        None
    } else {
        Some(omitted_mle)
    };

    let betas_vec: Vec<f64> = result.betas.iter().cloned().collect();
    let log_likelihood = re_mle_log_lik_twoway(
        &y_vec,
        exog,
        entity_id,
        time_id,
        &betas_vec,
        &kept,
        sigma2_alpha,
        sigma2_lambda,
        sigma2_e,
        n_entities,
        n_times,
        t_bar,
        n_bar,
    );

    if result.ms_residual > 1e-300 {
        let scale = sigma2_e / result.ms_residual;
        result.cov_beta = &result.cov_beta * scale;
        result.stds = result.cov_beta.diag().mapv(f64::sqrt);
        result.tvalues = &result.betas / &result.stds;
    }

    let obs_per_grp: Vec<usize> = obs_per_entity.values().copied().collect();
    let obs_min = obs_per_grp.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_grp.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_grp.iter().sum::<usize>() as f64 / n_entities as f64;

    let sigma_u = (sigma2_alpha + sigma2_lambda).sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = (sigma2_alpha + sigma2_lambda) / (sigma2_alpha + sigma2_lambda + sigma2_e);

    let k_slopes = if constant && kept.len() > 1 {
        kept.len() - 1
    } else {
        kept.len()
    };
    let lr_chi2 = (2.0 * (log_likelihood - ll_null)).max(0.0);
    let chi2_lr = ChiSquared::new(k_slopes as f64).map_err(|e| format!("Panel RE (Two-Way) MLE LR: {}", e))?;
    let prob_lr_chi2 = 1.0 - chi2_lr.cdf(lr_chi2);

    let ll_ols = {
        let (x_ols_use, ols_omitted) = drop_collinear_columns(
            exog,
            &vec![false; k],
            if constant { Some(0) } else { None },
        )
        .map_err(|e| format!("Panel RE (Two-Way) MLE OLS: {}", e))?;
        let ols_res = OLS {
            endog: endog.clone(),
            exog: x_ols_use,
            config: crate::regression::linear_model::OLSConfig {
                constant,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        }
        .fit()
        .map_err(|e| format!("Panel RE (Two-Way) MLE OLS: {}", e))?;
        let ols_betas: Vec<f64> = ols_res.betas.iter().cloned().collect();
        let ols_kept: Vec<usize> = (0..k).filter(|j| !ols_omitted.contains(j)).collect();
        let sigma2_e_ols = (ols_res.ss_residual / n as f64).max(1e-12);
        re_mle_log_lik_twoway(
            &y_vec,
            exog,
            entity_id,
            time_id,
            &ols_betas,
            &ols_kept,
            0.0,
            0.0,
            sigma2_e_ols,
            n_entities,
            n_times,
            t_bar,
            n_bar,
        )
    };
    let chibar2 = 2.0 * (log_likelihood - ll_ols).max(0.0);
    let chibar2_dist = ChiSquared::new(0.5).map_err(|e| format!("Panel RE (Two-Way) chibar2: {}", e))?;
    let prob_chibar2 = 1.0 - chibar2_dist.cdf(chibar2);

    let fe_stats = Some(super::PanelFEStats {
        r2: None,
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb: 0.0,
        theta: None,
    });

    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("Panel RE (Two-Way) MLE: {}", e))?;
    let pvalues_z: Array1<f64> = Array1::from_shape_fn(result.tvalues.len(), |i| {
        2.0 * (1.0 - std_normal.cdf(result.tvalues[i].abs()))
    });
    let z_crit = std_normal.inverse_cdf(0.975);
    let conf_int_left_z = &result.betas - z_crit * &result.stds;
    let conf_int_right_z = &result.betas + z_crit * &result.stds;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model: result.df_model,
        df_residual: result.df_residual,
        df_total: result.df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total: result.ms_total,
        covariance_type: result.covariance_type.clone(),
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: None,
        fvalue: result.fvalue,
        f_p_value: result.f_p_value,
        betas: result.betas,
        stds: result.stds,
        tvalues: result.tvalues,
        pvalues: pvalues_z,
        conf_int_left: conf_int_left_z,
        conf_int_right: conf_int_right_z,
        cov_beta: result.cov_beta,
        cov_beta_nonrobust: None,
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: None,
        prob_wald_chi2: None,
        log_likelihood: Some(log_likelihood),
        lr_chi2: Some(lr_chi2),
        prob_lr_chi2: Some(prob_lr_chi2),
        chibar2: Some(chibar2),
        prob_chibar2: Some(prob_chibar2),
        mle_iter_log_lik_const: Some(mle_iter_log_lik_const),
        mle_iter_log_lik: Some(mle_iter_log_lik),
    })
}

