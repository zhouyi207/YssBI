// ============== Time Random Effects (group by time_id) ==============

/// Panel RE FGLS (Time): same as entity RE but group by time period
pub fn fit_panel_re_fgls_time(
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
        return Err("Panel RE (FGLS Time): lengths must match".to_string());
    }
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_times < 2 {
        return Err("Panel RE (FGLS Time): need at least 2 time periods".to_string());
    }

    let (obs_per_time, t_bar_harmonic) = obs_per_group_and_harmonic_mean(time_id);

    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_w = within_transform(&y_vec, time_id);
    let k = exog.ncols();
    let mut x_w = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let tc = within_transform(&col, time_id);
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
            .map_err(|e| format!("Panel RE (Time) within: {}", e))?
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
    let res_w = ols_w.fit().map_err(|e| format!("Panel RE (Time) within: {}", e))?;
    let df_e = (n as i64 - n_times as i64 - k_w as i64).max(1) as usize;
    let sigma2_e = res_w.ss_residual / df_e as f64;

    let (_, y_b_vec, x_b_vec) = group_means(&y_vec, exog, time_id);
    let n_b = y_b_vec.len();
    let mut x_b_data = Vec::with_capacity(n_b * k);
    for i in 0..n_b {
        for c in 0..k {
            x_b_data.push(x_b_vec[i][c]);
        }
    }
    let y_b = Array1::from_vec(y_b_vec);
    let x_b = Array2::from_shape_vec((n_b, k), x_b_data)
        .map_err(|e| format!("Panel RE (Time) between: {:?}", e))?;

    let (x_b_use, _) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (Time) between: {}", e))?
    };

    let ols_b = OLS {
        endog: y_b,
        exog: x_b_use,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let res_b = ols_b.fit().map_err(|e| format!("Panel RE (Time) between: {}", e))?;
    let df_b = res_b.df_residual;
    let t_bar = if t_bar_harmonic > 1e-300 { t_bar_harmonic } else { n as f64 / n_times as f64 };
    let sigma2_u = if df_b > 0 {
        (res_b.ss_residual / df_b as f64 - sigma2_e / t_bar).max(0.0)
    } else {
        0.0
    };

    if sigma2_u <= 0.0 {
        return Err("Panel RE (Time): sigma_u^2 <= 0".to_string());
    }

    let y_bar = between_transform(&y_vec, time_id);
    let theta_arr: Vec<f64> = (0..n)
        .map(|i| {
            let tid = time_id[i];
            let t_i = *obs_per_time.get(&tid).unwrap_or(&1) as f64;
            let denom = t_i * sigma2_u + sigma2_e;
            1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
        })
        .collect();

    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| y_vec[i] - theta_arr[i] * y_bar[i]);

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar = between_transform(&col, time_id);
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
        }
    }

    let (x_star_use, omitted_final) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (Time) FGLS: {}", e))?
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(crate::regression::covariance::CovParams::Cluster {
                cluster_id: time_id.to_vec(),
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
    let omitted_indices = if omitted_final.is_empty() { None } else { Some(omitted_final) };
    let betas = &result.betas;

    let mut obs_per_group: HashMap<usize, usize> = HashMap::new();
    for &tid in time_id {
        *obs_per_group.entry(tid).or_insert(0) += 1;
    }
    let tids: Vec<usize> = obs_per_group.keys().copied().collect();
    let obs_per_grp: Vec<usize> = tids.iter().map(|&tid| obs_per_group.get(&tid).copied().unwrap_or(0)).collect();
    let obs_min = obs_per_grp.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_grp.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_grp.iter().sum::<usize>() as f64 / n_times as f64;

    let r2_within = {
        let y_w_vec: Vec<f64> = within_transform(&y_vec, time_id).iter().cloned().collect();
        let xb: Vec<f64> = (0..n)
            .map(|i| kept.iter().enumerate().map(|(idx, &c)| exog[[i, c]] * betas[idx]).sum())
            .collect();
        let xb_bar = between_transform(&xb, time_id);
        let xb_w: Vec<f64> = (0..n).map(|i| xb[i] - xb_bar[i]).collect();
        let (y_mean, xb_mean) = (y_w_vec.iter().sum::<f64>() / n as f64, xb_w.iter().sum::<f64>() / n as f64);
        let cov = y_w_vec.iter().zip(xb_w.iter()).map(|(y, x)| (y - y_mean) * (x - xb_mean)).sum::<f64>() / (n as f64 - 1.0).max(1.0);
        let (var_y, var_xb) = (
            y_w_vec.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
            xb_w.iter().map(|x| (x - xb_mean).powi(2)).sum::<f64>() / (n as f64 - 1.0).max(1.0),
        );
        if (var_y * var_xb).sqrt() > 1e-300 {
            (cov / (var_y * var_xb).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    let r2_between = {
        let (_, y_b_vec, x_b_vec) = group_means(&y_vec, exog, time_id);
        let n_b = y_b_vec.len();
        let y_mean = y_b_vec.iter().sum::<f64>() / n_b as f64;
        let xb_b: Vec<f64> = (0..n_b)
            .map(|i| kept.iter().enumerate().map(|(idx, &c)| x_b_vec[i][c] * betas[idx]).sum())
            .collect();
        let xb_mean = xb_b.iter().sum::<f64>() / n_b as f64;
        let cov = y_b_vec.iter().zip(xb_b.iter()).map(|(y, x)| (y - y_mean) * (x - xb_mean)).sum::<f64>() / (n_b as f64 - 1.0).max(1.0);
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
        let (xb_mean, y_mean) = (xb_obs.iter().sum::<f64>() / n as f64, endog.iter().sum::<f64>() / n as f64);
        let cov = xb_obs.iter().zip(endog.iter()).map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>() / (n as f64 - 1.0).max(1.0);
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

    let sigma_u = sigma2_u.sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = sigma2_u / (sigma2_u + sigma2_e);

    let fe_stats = Some(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
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
            (betas_nd.slice(ndarray::s![1..]).to_owned(), cov_beta.slice(ndarray::s![1.., 1..]).to_owned(), k_b - 1)
        } else {
            (betas_nd.clone(), cov_beta.clone(), k_b)
        };
        let v_s_faer = v_s.view().into_faer().to_owned();
        let beta_s_faer = beta_s.view().into_faer_col().to_owned();
        let x = v_s_faer.as_ref().llt(Side::Lower).map_err(|_| "RE Time Wald".to_string())?.solve(beta_s_faer.as_ref());
        let x_nd = x.as_ref().into_ndarray();
        let wald = beta_s.dot(&x_nd);
        let chi2_dist = ChiSquared::new(df_wald as f64).map_err(|e| format!("{}", e))?;
        (wald, 1.0 - chi2_dist.cdf(wald))
    };

    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("{}", e))?;
    let pvalues_z: Array1<f64> = Array1::from_shape_fn(result.tvalues.len(), |i| 2.0 * (1.0 - std_normal.cdf(result.tvalues[i].abs())));
    let z_crit = std_normal.inverse_cdf(0.975);
    let conf_int_left_z = &result.betas - z_crit * &result.stds;
    let conf_int_right_z = &result.betas + z_crit * &result.stds;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: result.num_observation,
        num_entities: n_times,
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

/// Panel RE Between (Time): regress time-period means ȳ_t on x̄_t
pub fn fit_panel_re_be_time(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    _cov_type: &str,
    _cov_params: Option<crate::regression::covariance::CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel RE (BE Time): lengths must match".to_string());
    }
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_times < 2 {
        return Err("Panel RE (BE Time): need at least 2 time periods".to_string());
    }

    let (_, y_b_vec, x_b_vec) = group_means(
        &endog.iter().cloned().collect::<Vec<_>>(),
        exog,
        time_id,
    );
    let k = exog.ncols();
    let n_b = y_b_vec.len();
    let mut x_b_data = Vec::with_capacity(n_b * k);
    for i in 0..n_b {
        for c in 0..k {
            x_b_data.push(x_b_vec[i][c]);
        }
    }
    let y_b = Array1::from_vec(y_b_vec);
    let x_b = Array2::from_shape_vec((n_b, k), x_b_data)
        .map_err(|e| format!("Panel RE (BE Time): {:?}", e))?;

    let (x_b_use, omitted_b) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (BE Time): {}", e))?
    };

    // Stata xtreg be does not support vce(cluster); between regression has n_b rows (one per time),
    // so cluster_id from full panel (length n) would not match. Use nonrobust like entity BE.
    let config = crate::regression::linear_model::OLSConfig {
        constant,
        cov_type: "nonrobust".to_string(),
        cov_params: None,
    };

    let ols = OLS {
        endog: y_b,
        exog: x_b_use,
        config,
    };
    let result = ols.fit()?;

    let kept: Vec<usize> = (0..k).filter(|j| !omitted_b.contains(j)).collect();
    let omitted_indices = if omitted_b.is_empty() { None } else { Some(omitted_b) };
    let betas = &result.betas;

    let mut obs_per_group: HashMap<usize, usize> = HashMap::new();
    for &tid in time_id {
        *obs_per_group.entry(tid).or_insert(0) += 1;
    }
    let obs_per_grp: Vec<usize> = obs_per_group.values().copied().collect();
    let obs_min = obs_per_grp.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_grp.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_grp.iter().sum::<usize>() as f64 / n_times as f64;

    let r2_between = result.r2;

    // R² Within = corr²((y - ȳ_t), (X - X̄_t)·β̂) for time BE
    let r2_within = {
        let y_w: Vec<f64> = within_transform(&endog.iter().cloned().collect::<Vec<_>>(), time_id)
            .iter()
            .cloned()
            .collect();
        let xb: Vec<f64> = (0..n)
            .map(|i| {
                let mut s = 0.0;
                for (idx, &c) in kept.iter().enumerate() {
                    s += exog[[i, c]] * betas[idx];
                }
                s
            })
            .collect();
        let xb_bar = between_transform(&xb, time_id);
        let xb_w: Vec<f64> = (0..n).map(|i| xb[i] - xb_bar[i]).collect();
        let (y_mean, xb_mean) = (
            y_w.iter().sum::<f64>() / n as f64,
            xb_w.iter().sum::<f64>() / n as f64,
        );
        let cov = y_w.iter().zip(xb_w.iter())
            .map(|(y, x)| (y - y_mean) * (x - xb_mean)).sum::<f64>()
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

    let r2_overall = {
        let xb_obs: Vec<f64> = (0..n)
            .map(|i| {
                let mut xb = 0.0;
                for (idx, &c) in kept.iter().enumerate() {
                    xb += exog[[i, c]] * betas[idx];
                }
                xb
            })
            .collect();
        let (xb_mean, y_mean) = (
            xb_obs.iter().sum::<f64>() / n as f64,
            endog.iter().sum::<f64>() / n as f64,
        );
        let cov = xb_obs.iter().zip(endog.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>()
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

    // sd(λ_t + avg(e_.t)) — time-level composite error (like entity BE: sd(u_i + avg(e_i.)))
    let sd_lambda_plus_avg_e = (result.ms_residual).sqrt();
    let fe_stats = Some(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u: sd_lambda_plus_avg_e, sigma_e: 0.0, rho: 0.0 },
        corr_u_i_xb: 0.0,
        theta: None,
    });

    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("{}", e))?;
    let pvalues_z: Array1<f64> = Array1::from_shape_fn(result.tvalues.len(), |i| 2.0 * (1.0 - std_normal.cdf(result.tvalues[i].abs())));
    let z_crit = std_normal.inverse_cdf(0.975);
    let conf_int_left_z = &result.betas - z_crit * &result.stds;
    let conf_int_right_z = &result.betas + z_crit * &result.stds;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: n,
        num_entities: n_times,
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
        cov_beta_nonrobust: None,
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: None,
        prob_wald_chi2: None,
        log_likelihood: None,
        lr_chi2: None,
        prob_lr_chi2: None,
        chibar2: None,
        prob_chibar2: None,
        mle_iter_log_lik_const: None,
        mle_iter_log_lik: None,
    })
}

/// Panel RE MLE (Time): same as entity MLE but group by time period
pub fn fit_panel_re_mle_time(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel RE (MLE Time): lengths must match".to_string());
    }
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_times < 2 {
        return Err("Panel RE (MLE Time): need at least 2 time periods".to_string());
    }

    let (obs_per_time, t_bar_harmonic) = obs_per_group_and_harmonic_mean(time_id);
    let k = exog.ncols();
    let endog_vec: Vec<f64> = endog.iter().cloned().collect();
    let t_bar = if t_bar_harmonic > 1e-300 { t_bar_harmonic } else { n as f64 / n_times as f64 };

    let y_global_mean = endog_vec.iter().sum::<f64>() / n as f64;
    let mut sigma2_e_null;
    let mut sigma2_u_null;
    let mut ll_null = 0.0;
    let mut mle_iter_log_lik_const: Vec<f64> = Vec::new();
    {
        let (_, y_b_vec, _) = group_means(&endog_vec, exog, time_id);
        let n_b = y_b_vec.len();
        let mut ss_b = 0.0;
        for &y in &y_b_vec {
            ss_b += (y - y_global_mean).powi(2);
        }
        let df_b = (n_b as i64 - 1).max(1) as usize;
        let var_y_bar = ss_b / df_b as f64;
        let r_w = within_transform(&endog_vec, time_id);
        let r_w_ss: f64 = r_w.iter().map(|x| x * x).sum();
        let df_w = n.saturating_sub(n_times).max(1);
        let sigma2_e_sa = (r_w_ss / df_w as f64).max(1e-12);
        let sigma2_u_sa = (var_y_bar - sigma2_e_sa / t_bar).max(1e-10);
        let ss_pooled: f64 = endog_vec.iter().map(|y| (y - y_global_mean).powi(2)).sum();
        let sigma2_v = (ss_pooled / ((n as i64 - 1).max(1) as f64)).max(1e-12);
        let fac = (1.0 - 1.0 / t_bar.max(1.0)).max(1e-6);
        let (sigma2_e_pool, sigma2_u_pool) = if sigma2_v > var_y_bar && fac > 1e-6 {
            let se = ((sigma2_v - var_y_bar) / fac).max(1e-12);
            let su = (sigma2_v - se).max(1e-10);
            (se, su)
        } else {
            (sigma2_e_sa, sigma2_u_sa)
        };
        let use_pooled_init = sigma2_v > var_y_bar && fac > 1e-6 && sigma2_u_pool > 1e-10;
        let max_iter_null = 200;

        fn gls_alpha_and_ll_time(
            endog_vec: &[f64],
            exog: &Array2<f64>,
            time_id: &[usize],
            obs_per_time: &HashMap<usize, usize>,
            su: f64,
            se: f64,
        ) -> Result<(f64, f64), String> {
            let n = endog_vec.len();
            let theta_arr: Vec<f64> = (0..n)
                .map(|i| {
                    let tid = time_id[i];
                    let t_i = *obs_per_time.get(&tid).unwrap_or(&1) as f64;
                    let denom = t_i * su + se;
                    1.0 - (se / denom.max(1e-300)).sqrt()
                })
                .collect();
            let y_bar = between_transform(endog_vec, time_id);
            let y_star: Vec<f64> = (0..n).map(|i| endog_vec[i] - theta_arr[i] * y_bar[i]).collect();
            let x_star_const: Vec<f64> = (0..n).map(|i| 1.0 - theta_arr[i]).collect();
            let x_const = Array2::from_shape_vec((n, 1), x_star_const).unwrap();
            let (x_use, _) = drop_collinear_columns(&x_const, &[false], Some(0))
                .map_err(|e| format!("const-only init: {}", e))?;
            let res = OLS {
                endog: Array1::from_vec(y_star),
                exog: x_use,
                config: crate::regression::linear_model::OLSConfig {
                    constant: true,
                    cov_type: "nonrobust".to_string(),
                    cov_params: None,
                },
            }
            .fit()
            .map_err(|e| format!("const-only GLS: {}", e))?;
            let alpha = res.betas[0];
            let ll = re_mle_log_lik(endog_vec, exog, time_id, &[alpha], &[0], su, se, obs_per_time);
            Ok((alpha, ll))
        }

        let (_, ll_sa) = gls_alpha_and_ll_time(&endog_vec, exog, time_id, &obs_per_time, sigma2_u_sa, sigma2_e_sa)?;
        let (_, ll_pool) = if use_pooled_init {
            gls_alpha_and_ll_time(&endog_vec, exog, time_id, &obs_per_time, sigma2_u_pool, sigma2_e_pool)?
        } else {
            (0.0, f64::NEG_INFINITY)
        };
        let ll_grand = re_mle_log_lik(
            &endog_vec,
            exog,
            time_id,
            &[y_global_mean],
            &[0],
            sigma2_u_sa,
            sigma2_e_sa,
            &obs_per_time,
        );
        let (alpha_init, sigma2_e_init, sigma2_u_init, ll_init) = {
            let (alpha_sa, _) = gls_alpha_and_ll_time(&endog_vec, exog, time_id, &obs_per_time, sigma2_u_sa, sigma2_e_sa)?;
            let (alpha_pool, _) = if use_pooled_init {
                gls_alpha_and_ll_time(&endog_vec, exog, time_id, &obs_per_time, sigma2_u_pool, sigma2_e_pool)?
            } else {
                (0.0, f64::NEG_INFINITY)
            };
            let mut best = (alpha_sa, sigma2_e_sa, sigma2_u_sa, ll_sa);
            if use_pooled_init && ll_pool > best.3 {
                best = (alpha_pool, sigma2_e_pool, sigma2_u_pool, ll_pool);
            }
            if ll_grand > best.3 {
                best = (y_global_mean, sigma2_e_sa, sigma2_u_sa, ll_grand);
            }
            best
        };
        sigma2_e_null = sigma2_e_init;
        sigma2_u_null = sigma2_u_init;
        mle_iter_log_lik_const.push(ll_init);

        let kept_const: Vec<usize> = vec![0];
        let mut params_const = vec![alpha_init, sigma2_u_init.ln(), sigma2_e_init.ln()];
        let h_num = 1e-6;
        for _ in 0..max_iter_null {
            match re_mle_newton_step(&params_const, &endog_vec, exog, time_id, &kept_const, &obs_per_time, h_num) {
                Ok((new_params, _neg_ll, converged)) => {
                    let ll = -re_mle_neg_ll_from_params(&new_params, &endog_vec, exog, time_id, &kept_const, &obs_per_time);
                    mle_iter_log_lik_const.push(ll);
                    params_const = new_params;
                    sigma2_u_null = params_const[1].exp().clamp(1e-12, 1e10);
                    sigma2_e_null = params_const[2].exp().clamp(1e-12, 1e10);
                    ll_null = ll;
                    if converged {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    let mut sigma2_e = sigma2_e_null;
    let mut sigma2_u = sigma2_u_null;
    let max_iter = 200;
    let betas: Vec<f64>;
    let kept: Vec<usize>;
    let mut mle_iter_log_lik: Vec<f64> = Vec::new();

    {
        let theta_arr: Vec<f64> = (0..n)
            .map(|i| {
                let tid = time_id[i];
                let t_i = *obs_per_time.get(&tid).unwrap_or(&1) as f64;
                let denom = t_i * sigma2_u + sigma2_e;
                1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
            })
            .collect();
        let y_bar = between_transform(&endog_vec, time_id);
        let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| endog_vec[i] - theta_arr[i] * y_bar[i]);
        let mut x_star = Array2::zeros((n, k));
        for c in 0..k {
            let col: Vec<f64> = exog.column(c).iter().cloned().collect();
            let x_bar = between_transform(&col, time_id);
            for i in 0..n {
                x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
            }
        }
        let (x_star_use, omitted_star) = {
            let col_is_dummy = vec![false; k];
            let intercept_col = if constant { Some(0) } else { None };
            drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
                .map_err(|e| format!("Panel RE (MLE Time) iter 0: {}", e))?
        };
        kept = (0..k).filter(|j| !omitted_star.contains(j)).collect();
        let ols_re = OLS {
            endog: y_star,
            exog: x_star_use,
            config: crate::regression::linear_model::OLSConfig {
                constant,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        };
        let res0 = ols_re.fit().map_err(|e| format!("Panel RE (MLE Time) iter 0: {}", e))?;
        betas = res0.betas.to_vec();
        let ll0_full = re_mle_log_lik(&endog_vec, exog, time_id, &betas, &kept, sigma2_u, sigma2_e, &obs_per_time);
        mle_iter_log_lik.push(ll0_full);
    }

    let mut params: Vec<f64> = betas.iter().cloned().collect();
    params.push(sigma2_u.ln());
    params.push(sigma2_e.ln());
    let h_num = 1e-6;
    for _ in 0..max_iter {
        match re_mle_newton_step(&params, &endog_vec, exog, time_id, &kept, &obs_per_time, h_num) {
            Ok((new_params, _neg_ll, converged)) => {
                let n_beta = kept.len();
                sigma2_u = new_params[n_beta].exp().clamp(1e-12, 1e10);
                sigma2_e = new_params[n_beta + 1].exp().clamp(1e-12, 1e10);
                let ll = -re_mle_neg_ll_from_params(&new_params, &endog_vec, exog, time_id, &kept, &obs_per_time);
                mle_iter_log_lik.push(ll);
                params = new_params;
                if converged {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let theta_arr: Vec<f64> = (0..n)
        .map(|i| {
            let tid = time_id[i];
            let t_i = *obs_per_time.get(&tid).unwrap_or(&1) as f64;
            let denom = t_i * sigma2_u + sigma2_e;
            1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
        })
        .collect();

    let y_bar = between_transform(&endog_vec, time_id);
    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| endog_vec[i] - theta_arr[i] * y_bar[i]);

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar = between_transform(&col, time_id);
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
        }
    }

    let (x_star_use, omitted_mle) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (MLE Time) final: {}", e))?
    };

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

    if result.ms_residual > 1e-300 {
        let scale = sigma2_e / result.ms_residual;
        result.cov_beta = &result.cov_beta * scale;
        result.stds = result.cov_beta.diag().mapv(f64::sqrt);
        result.tvalues = &result.betas / &result.stds;
    }

    let kept: Vec<usize> = (0..k).filter(|j| !omitted_mle.contains(j)).collect();
    let omitted_indices = if omitted_mle.is_empty() { None } else { Some(omitted_mle) };

    let mut obs_per_group: HashMap<usize, usize> = HashMap::new();
    for &tid in time_id {
        *obs_per_group.entry(tid).or_insert(0) += 1;
    }
    let obs_per_grp: Vec<usize> = obs_per_group.values().copied().collect();
    let obs_min = obs_per_grp.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_grp.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_grp.iter().sum::<usize>() as f64 / n_times as f64;

    let sigma_u = sigma2_u.sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = sigma2_u / (sigma2_u + sigma2_e);

    let betas_vec: Vec<f64> = result.betas.iter().cloned().collect();
    let log_likelihood = re_mle_log_lik(&endog_vec, exog, time_id, &betas_vec, &kept, sigma2_u, sigma2_e, &obs_per_time);
    let k_slopes = if constant && kept.len() > 1 { kept.len() - 1 } else { kept.len() };
    let lr_chi2 = {
        let raw = 2.0 * (log_likelihood - ll_null);
        if raw.is_nan() || raw.is_infinite() { 0.0 } else { raw.max(0.0) }
    };
    let chi2_lr = ChiSquared::new(k_slopes as f64).map_err(|e| format!("Panel RE MLE Time LR: {}", e))?;
    let prob_lr_chi2 = 1.0 - chi2_lr.cdf(lr_chi2);

    let ll_ols = {
        let (x_ols_use, ols_omitted) = {
            let col_is_dummy = vec![false; k];
            let intercept_col = if constant { Some(0) } else { None };
            drop_collinear_columns(exog, &col_is_dummy, intercept_col)
                .map_err(|e| format!("Panel RE MLE Time OLS: {}", e))?
        };
        let ols_pooled = OLS {
            endog: endog.clone(),
            exog: x_ols_use,
            config: crate::regression::linear_model::OLSConfig {
                constant,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        };
        let res_ols = ols_pooled.fit().map_err(|e| format!("Panel RE MLE Time OLS: {}", e))?;
        let ols_betas: Vec<f64> = res_ols.betas.iter().cloned().collect();
        let ols_kept: Vec<usize> = (0..k).filter(|j| !ols_omitted.contains(j)).collect();
        let sigma2_e_ols = (res_ols.ss_residual / n as f64).max(1e-12);
        re_mle_log_lik(&endog_vec, exog, time_id, &ols_betas, &ols_kept, 0.0, sigma2_e_ols, &obs_per_time)
    };
    let chibar2 = {
        let raw = 2.0 * (log_likelihood - ll_ols);
        if raw.is_nan() || raw.is_infinite() { 0.0 } else { raw.max(0.0) }
    };
    let chi2_1 = ChiSquared::new(1.0).map_err(|e| format!("Panel RE MLE Time chibar2: {}", e))?;
    let prob_chibar2 = 0.5 * (1.0 - chi2_1.cdf(chibar2));

    let fe_stats = Some(super::PanelFEStats {
        r2: None,
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb: 0.0,
        theta: None,
    });

    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("Panel RE MLE Time: {}", e))?;
    let pvalues_z: Array1<f64> = Array1::from_shape_fn(result.tvalues.len(), |i| 2.0 * (1.0 - std_normal.cdf(result.tvalues[i].abs())));
    let z_crit = std_normal.inverse_cdf(0.975);
    let conf_int_left_z = &result.betas - z_crit * &result.stds;
    let conf_int_right_z = &result.betas + z_crit * &result.stds;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: result.num_observation,
        num_entities: n_times,
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
