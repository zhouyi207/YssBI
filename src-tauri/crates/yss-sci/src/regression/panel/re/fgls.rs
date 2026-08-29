/// Panel Random Effects FGLS (Swamy-Arora variance components)
pub fn fit_panel_re_fgls(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n {
        return Err(format!(
            "Panel RE (FGLS): exog rows {} != endog len {}",
            exog.nrows(),
            n
        ));
    }
    if entity_id.len() != n {
        return Err(format!(
            "Panel RE (FGLS): entity_id len {} != n {}",
            entity_id.len(),
            n
        ));
    }

    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 {
        return Err("Panel RE (FGLS): need at least 2 entities".to_string());
    }

    let (obs_per_entity, t_bar_harmonic) = obs_per_entity_and_harmonic_mean(entity_id);

    // Step 1: Within regression to get sigma_e^2 (variance of idiosyncratic error)
    let y_w = within_transform(&endog.iter().cloned().collect::<Vec<_>>(), entity_id);
    let k = exog.ncols();
    let mut x_w = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let tc = within_transform(&col, entity_id);
        for i in 0..n {
            x_w[[i, c]] = tc[i];
        }
    }

    // After within transform, constant column becomes zero - drop it and any collinear cols
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
            .map_err(|e| format!("Panel RE within step: {}", e))?
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
    let res_w = ols_w.fit().map_err(|e| format!("Panel RE within step: {}", e))?;
    // Stata: σ²_e = Σe²_it / (N - n - K + 1), K = 1 + k_slopes (within has constant)
    // Our within has k_w slopes (no const), so df = N - n - k_w
    let df_e = (n as i64 - n_entities as i64 - k_w as i64).max(1) as usize;
    let sigma2_e = res_w.ss_residual / df_e as f64;

    // Step 2: Between regression (entity means) to get sigma_u^2
    let (_, y_b_vec, x_b_vec) = entity_means(
        &endog.iter().cloned().collect::<Vec<_>>(),
        exog,
        entity_id,
    );
    let n_b = y_b_vec.len();
    let mut x_b_data = Vec::with_capacity(n_b * k);
    for i in 0..n_b {
        for c in 0..k {
            x_b_data.push(x_b_vec[i][c]);
        }
    }
    let y_b = Array1::from_vec(y_b_vec);
    let x_b = Array2::from_shape_vec((n_b, k), x_b_data)
        .map_err(|e| format!("Panel RE between: {:?}", e))?;

    let (x_b_use, _) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE between step: {}", e))?
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
    let res_b = ols_b.fit().map_err(|e| format!("Panel RE between step: {}", e))?;
    let df_b = res_b.df_residual;
    // Stata: σ²_u = max(0, SSR_b/(n-K) - σ²_e/T̄), T̄ = harmonic mean of T_i
    let t_bar = if t_bar_harmonic > 1e-300 { t_bar_harmonic } else { n as f64 / n_entities as f64 };
    let sigma2_u = if df_b > 0 {
        (res_b.ss_residual / df_b as f64 - sigma2_e / t_bar).max(0.0)
    } else {
        0.0
    };

    if sigma2_u <= 0.0 {
        return Err("Panel RE: variance component sigma_u^2 <= 0 (try FE instead)".to_string());
    }

    // Theta per entity: θ_i = 1 - sqrt(σ²_e / (T_i·σ²_u + σ²_e)) (Stata xtreg, re)
    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_bar = between_transform(&y_vec, entity_id);
    let theta_arr: Vec<f64> = (0..n)
        .map(|i| {
            let eid = entity_id[i];
            let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
            let denom = t_i * sigma2_u + sigma2_e;
            1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
        })
        .collect();

    // Quasi-demean: y*_it = y_it - θ_i * ȳ_i
    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| y_vec[i] - theta_arr[i] * y_bar[i]);

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar = between_transform(&col, entity_id);
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
        }
    }

    let (x_star_use, omitted_final) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (FGLS) final: {}", e))?
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

    // Obs per group
    let mut obs_per_entity: HashMap<usize, usize> = HashMap::new();
    for &eid in entity_id {
        *obs_per_entity.entry(eid).or_insert(0) += 1;
    }
    let eids: Vec<usize> = obs_per_entity.keys().copied().collect();
    let obs_per_group: Vec<usize> = eids.iter().map(|&eid| obs_per_entity.get(&eid).copied().unwrap_or(0)).collect();
    let obs_min = obs_per_group.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_group.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_group.iter().sum::<usize>() as f64 / n_entities as f64;

    // R² Within = corr²((y - ȳ), (X - X̄)·β̂)
    let r2_within = {
        let y_w: Vec<f64> = within_transform(&endog.iter().cloned().collect::<Vec<_>>(), entity_id)
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
        let xb_bar = between_transform(&xb, entity_id);
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

    // R² Between: regress ȳ on x̄ with FGLS betas
    let r2_between = {
        let (_, y_b_vec, x_b_vec) = entity_means(
            &endog.iter().cloned().collect::<Vec<_>>(),
            exog,
            entity_id,
        );
        let n_b = y_b_vec.len();
        let y_mean = y_b_vec.iter().sum::<f64>() / n_b as f64;
        let xb_b: Vec<f64> = (0..n_b)
            .map(|i| {
                let mut s = 0.0;
                for (idx, &c) in kept.iter().enumerate() {
                    s += x_b_vec[i][c] * betas[idx];
                }
                s
            })
            .collect();
        let xb_mean = xb_b.iter().sum::<f64>() / n_b as f64;
        let cov = y_b_vec.iter().zip(xb_b.iter())
            .map(|(y, x)| (y - y_mean) * (x - xb_mean)).sum::<f64>()
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

    // R² Overall = corr²(y, X·β̂)
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

    let sigma_u = sigma2_u.sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = sigma2_u / (sigma2_u + sigma2_e);

    let theta = {
        let thetas: Vec<f64> = obs_per_entity.values().map(|&t_i| {
            let denom = t_i as f64 * sigma2_u + sigma2_e;
            1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
        }).collect();
        let mn = thetas.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = thetas.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = thetas.iter().sum::<f64>() / thetas.len().max(1) as f64;
        super::ThetaStats { min: mn, avg, max: mx }
    };

    let fe_stats = Some(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb: 0.0,
        theta: Some(theta),
    });

    // Wald chi2 for joint significance (Stata xtreg, re uses chi2, not F)
    let (wald_chi2, prob_wald_chi2) = {
        let cov_beta = &result.cov_beta;
        let betas_nd = &result.betas;
        let k_b = betas_nd.len();
        let (beta_s, v_s, df_wald) = if constant && k_b > 1 {
            let beta_s = betas_nd.slice(ndarray::s![1..]).to_owned();
            let v_s = cov_beta.slice(ndarray::s![1.., 1..]).to_owned();
            (beta_s, v_s, k_b - 1)
        } else {
            (betas_nd.clone(), cov_beta.clone(), k_b)
        };
        let v_s_faer = v_s.view().into_faer().to_owned();
        let beta_s_faer = beta_s.view().into_faer_col().to_owned();
        let x = v_s_faer
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "Panel RE FGLS: V not pd for Wald".to_string())?
            .solve(beta_s_faer.as_ref());
        let x_nd = x.as_ref().into_ndarray();
        let wald = beta_s.dot(&x_nd);
        let chi2_dist = ChiSquared::new(df_wald as f64).map_err(|e| format!("Panel RE FGLS Wald: {}", e))?;
        let wald_p = 1.0 - chi2_dist.cdf(wald);
        (wald, wald_p)
    };

    // Stata xtreg, re uses z (asymptotic normal), not t
    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("Panel RE FGLS: {}", e))?;
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
        num_time_periods: 0,
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

