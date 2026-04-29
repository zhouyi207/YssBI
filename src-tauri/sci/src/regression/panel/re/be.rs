/// Panel Between estimator: regress ȳ_i on x̄_i (entity means)
pub fn fit_panel_re_be(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n {
        return Err("Panel RE (BE): lengths must match".to_string());
    }
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 {
        return Err("Panel RE (BE): need at least 2 entities".to_string());
    }

    let (eids, y_b_vec, x_b_vec) = entity_means(
        &endog.iter().cloned().collect::<Vec<_>>(),
        exog,
        entity_id,
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
        .map_err(|e| format!("Panel RE (BE): {:?}", e))?;

    let (x_b_use, omitted_be) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_b, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (BE): {}", e))?
    };

    // Stata xtreg be does not support vce(cluster); use conventional (nonrobust) to match Stata
    let config = crate::regression::linear_model::OLSConfig {
        constant,
        cov_type: "nonrobust".to_string(),
        cov_params: None,
    };

    let result = OLS {
        endog: y_b.clone(),
        exog: x_b_use.clone(),
        config,
    }
    .fit()?;

    let omitted_indices = if omitted_be.is_empty() {
        None
    } else {
        Some(omitted_be.clone())
    };

    // BE-specific stats (Stata xtreg, be style)
    let kept: Vec<usize> = (0..k).filter(|j| !omitted_be.contains(j)).collect();
    let betas = &result.betas;

    let mut obs_per_entity: HashMap<usize, usize> = HashMap::new();
    for &eid in entity_id {
        *obs_per_entity.entry(eid).or_insert(0) += 1;
    }
    let obs_per_group: Vec<usize> = eids.iter().map(|&eid| obs_per_entity.get(&eid).copied().unwrap_or(0)).collect();
    let obs_min = obs_per_group.iter().copied().min().unwrap_or(0);
    let obs_max = obs_per_group.iter().copied().max().unwrap_or(0);
    let obs_avg = obs_per_group.iter().sum::<usize>() as f64 / n_b as f64;

    let r2_between = result.r2;

    // Stata: Within R² = corr²((y - ȳ), (X - X̄)·β̂)
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

    let sd_u_plus_avg_e = (result.ms_residual).sqrt();
    let fe_stats = Some(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u: sd_u_plus_avg_e, sigma_e: 0.0, rho: 0.0 },
        corr_u_i_xb: 0.0,
        theta: None,
    });

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats,
        num_observation: n,
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
        pvalues: result.pvalues,
        conf_int_left: result.conf_int_left,
        conf_int_right: result.conf_int_right,
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

