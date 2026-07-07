/// RE MLE log likelihood (Stata xtreg, mle formula). Generic over group_id (entity or time).
fn re_mle_log_lik(
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
    betas: &[f64],
    kept: &[usize],
    sigma2_u: f64,
    sigma2_e: f64,
    obs_per_group: &HashMap<usize, usize>,
) -> f64 {
    let (gids, _, _) = group_means(endog, exog, group_id);
    let mut ll = 0.0;
    let two_pi = std::f64::consts::PI * 2.0;
    for &gid in &gids {
        let t_i = *obs_per_group.get(&gid).unwrap_or(&1) as f64;
        let mut sum_r: f64 = 0.0;
        let mut sum_r2: f64 = 0.0;
        let mut count = 0usize;
        for (idx, &g) in group_id.iter().enumerate() {
            if g != gid {
                continue;
            }
            let mut xb = 0.0;
            for (j, &c) in kept.iter().enumerate() {
                xb += exog[[idx, c]] * betas[j];
            }
            let r = endog[idx] - xb;
            sum_r += r;
            sum_r2 += r * r;
            count += 1;
        }
        if count == 0 || sigma2_e <= 1e-300 {
            continue;
        }
        let denom = t_i * sigma2_u + sigma2_e;
        let weight = if denom > 1e-300 {
            sigma2_u / denom
        } else {
            0.0
        };
        let term1 = (sum_r2 - weight * sum_r * sum_r) / sigma2_e;
        let term2 = (t_i * sigma2_u / sigma2_e + 1.0).ln();
        let term3 = t_i * (two_pi * sigma2_e).ln();
        ll -= 0.5 * (term1 + term2 + term3);
    }
    ll
}

/// Negative log likelihood for Newton-Raphson. params = [betas..., ln(σ²_u), ln(σ²_e)].
fn re_mle_neg_ll_from_params(
    params: &[f64],
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
    kept: &[usize],
    obs_per_group: &HashMap<usize, usize>,
) -> f64 {
    let n_beta = kept.len();
    if params.len() < n_beta + 2 {
        return f64::INFINITY;
    }
    let betas: Vec<f64> = params[..n_beta].to_vec();
    let ln_sigma2_u = params[n_beta];
    let ln_sigma2_e = params[n_beta + 1];
    let sigma2_u = ln_sigma2_u.exp().clamp(1e-12, 1e10);
    let sigma2_e = ln_sigma2_e.exp().clamp(1e-12, 1e10);
    -re_mle_log_lik(endog, exog, group_id, &betas, kept, sigma2_u, sigma2_e, obs_per_group)
}

/// Central difference numerical gradient of neg_ll.
fn re_mle_numerical_gradient(
    params: &[f64],
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
    kept: &[usize],
    obs_per_group: &HashMap<usize, usize>,
    h: f64,
) -> Vec<f64> {
    let d = params.len();
    let mut grad = vec![0.0; d];
    for i in 0..d {
        let step = if i >= params.len() - 2 {
            (h * (1.0 + params[i].abs())).max(1e-8)
        } else {
            h
        };
        let mut p_plus = params.to_vec();
        p_plus[i] += step;
        let mut p_minus = params.to_vec();
        p_minus[i] -= step;
        let f_plus = re_mle_neg_ll_from_params(&p_plus, endog, exog, group_id, kept, obs_per_group);
        let f_minus = re_mle_neg_ll_from_params(&p_minus, endog, exog, group_id, kept, obs_per_group);
        grad[i] = (f_plus - f_minus) / (2.0 * step);
    }
    grad
}

/// Numerical Hessian via forward differences on gradient (for Newton-Raphson).
fn re_mle_numerical_hessian(
    params: &[f64],
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
    kept: &[usize],
    obs_per_group: &HashMap<usize, usize>,
    h: f64,
) -> Vec<Vec<f64>> {
    let d = params.len();
    let mut hess = vec![vec![0.0; d]; d];
    let g0 = re_mle_numerical_gradient(params, endog, exog, group_id, kept, obs_per_group, h);
    for j in 0..d {
        let step_j = if j >= params.len() - 2 {
            (h * (1.0 + params[j].abs())).max(1e-8)
        } else {
            h
        };
        let mut p_j = params.to_vec();
        p_j[j] += step_j;
        let g_j = re_mle_numerical_gradient(&p_j, endog, exog, group_id, kept, obs_per_group, h);
        for i in 0..d {
            hess[i][j] = (g_j[i] - g0[i]) / step_j;
        }
    }
    hess
}

/// Newton-Raphson step for maximization: θ_new = θ_old - H^{-1} * g (minimize neg_ll).
/// Returns (new_params, new_neg_ll, converged).
fn re_mle_newton_step(
    params: &[f64],
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
    kept: &[usize],
    obs_per_group: &HashMap<usize, usize>,
    h: f64,
) -> Result<(Vec<f64>, f64, bool), String> {
    let d = params.len();
    let g = re_mle_numerical_gradient(params, endog, exog, group_id, kept, obs_per_group, h);
    let hess = re_mle_numerical_hessian(params, endog, exog, group_id, kept, obs_per_group, h);
    let g_norm: f64 = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    if g_norm < 1e-6 {
        let neg_ll = re_mle_neg_ll_from_params(params, endog, exog, group_id, kept, obs_per_group);
        return Ok((params.to_vec(), neg_ll, true));
    }
    let hess_mat = Mat::from_fn(d, d, |i, j| hess[i][j]);
    let g_col = Mat::from_fn(d, 1, |i, _| -g[i]);
    let step = hess_mat
        .as_ref()
        .partial_piv_lu()
        .solve(g_col.as_ref());
    let mut new_params = params.to_vec();
    let mut step_scale = 1.0;
    let neg_ll0 = re_mle_neg_ll_from_params(params, endog, exog, group_id, kept, obs_per_group);
    for _ in 0..20 {
        for i in 0..d {
            new_params[i] = params[i] + step_scale * step[(i, 0)];
        }
        let neg_ll = re_mle_neg_ll_from_params(&new_params, endog, exog, group_id, kept, obs_per_group);
        if neg_ll <= neg_ll0 + 1e-12 {
            let converged = g_norm < 1e-5;
            return Ok((new_params, neg_ll, converged));
        }
        step_scale *= 0.5;
    }
    Err("Newton: step halving failed".to_string())
}

/// Panel Random Effects MLE (Stata xtreg, mle)
/// Uses Stata-style MLE: entity-specific θ_i, variance updates σ²_e = (1/N)Σ(y_it−x_itβ−û_i)², σ²_u = (1/n)Σû_i².
/// Standard errors: OIM only (no robust, cluster, etc.).
pub fn fit_panel_re_mle(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n {
        return Err("Panel RE (MLE): lengths must match".to_string());
    }
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 {
        return Err("Panel RE (MLE): need at least 2 entities".to_string());
    }

    let (obs_per_entity, t_bar_harmonic) = obs_per_entity_and_harmonic_mean(entity_id);
    let k = exog.ncols();
    let endog_vec: Vec<f64> = endog.iter().cloned().collect();
    let t_bar = if t_bar_harmonic > 1e-300 { t_bar_harmonic } else { n as f64 / n_entities as f64 };

    // 1. Fit constant-only model for ll_null (Stata "Fitting constant-only model")
    let y_global_mean = endog_vec.iter().sum::<f64>() / n as f64;
    let mut sigma2_e_null;
    let mut sigma2_u_null;
    let mut ll_null = 0.0;
    let mut mle_iter_log_lik_const: Vec<f64> = Vec::new();
    {
        let (_, y_b_vec, _) = entity_means(&endog_vec, exog, entity_id);
        let n_b = y_b_vec.len();
        let mut ss_b = 0.0;
        for &y in &y_b_vec {
            ss_b += (y - y_global_mean).powi(2);
        }
        let df_b = (n_b as i64 - 1).max(1) as usize;
        let var_y_bar = ss_b / df_b as f64;
        let r_w = within_transform(&endog_vec, entity_id);
        let r_w_ss: f64 = r_w.iter().map(|x| x * x).sum();
        let df_w = n.saturating_sub(n_entities).max(1);
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
        fn gls_alpha_and_ll(
            endog_vec: &[f64],
            exog: &Array2<f64>,
            entity_id: &[usize],
            obs_per_entity: &HashMap<usize, usize>,
            su: f64,
            se: f64,
        ) -> Result<(f64, f64), String> {
            let n = endog_vec.len();
            let theta_arr: Vec<f64> = (0..n)
                .map(|i| {
                    let eid = entity_id[i];
                    let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
                    let denom = t_i * su + se;
                    1.0 - (se / denom.max(1e-300)).sqrt()
                })
                .collect();
            let y_bar = between_transform(endog_vec, entity_id);
            let y_star: Vec<f64> = (0..n).map(|i| endog_vec[i] - theta_arr[i] * y_bar[i]).collect();
            // Quasi-demeaned constant: 1 - θ (not 1), so OLS gives α = ȳ
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
            let ll = re_mle_log_lik(
                endog_vec,
                exog,
                entity_id,
                &[alpha],
                &[0],
                su,
                se,
                obs_per_entity,
            );
            Ok((alpha, ll))
        }
        let (_, ll_sa) = gls_alpha_and_ll(
            &endog_vec,
            exog,
            entity_id,
            &obs_per_entity,
            sigma2_u_sa,
            sigma2_e_sa,
        )?;
        let (_, ll_pool) = if use_pooled_init {
            gls_alpha_and_ll(
                &endog_vec,
                exog,
                entity_id,
                &obs_per_entity,
                sigma2_u_pool,
                sigma2_e_pool,
            )?
        } else {
            (0.0, f64::NEG_INFINITY)
        };
        // Stata-style Iteration 0: grand mean α = ȳ often gives better initial ll than GLS
        let ll_grand = re_mle_log_lik(
            &endog_vec,
            exog,
            entity_id,
            &[y_global_mean],
            &[0],
            sigma2_u_sa,
            sigma2_e_sa,
            &obs_per_entity,
        );
        let (alpha_init, sigma2_e_init, sigma2_u_init, ll_init) = {
            let (alpha_sa, _) = gls_alpha_and_ll(
                &endog_vec,
                exog,
                entity_id,
                &obs_per_entity,
                sigma2_u_sa,
                sigma2_e_sa,
            )?;
            let (alpha_pool, _) = if use_pooled_init {
                gls_alpha_and_ll(
                    &endog_vec,
                    exog,
                    entity_id,
                    &obs_per_entity,
                    sigma2_u_pool,
                    sigma2_e_pool,
                )?
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
        // Newton-Raphson on (α, ln σ²_u, ln σ²_e) for constant-only model
        let kept_const: Vec<usize> = vec![0];
        let mut params_const = vec![
            alpha_init,
            sigma2_u_init.ln(),
            sigma2_e_init.ln(),
        ];
        let h_num = 1e-6;
        for _ in 0..max_iter_null {
            match re_mle_newton_step(
                &params_const,
                &endog_vec,
                exog,
                entity_id,
                &kept_const,
                &obs_per_entity,
                h_num,
            ) {
                Ok((new_params, _neg_ll, converged)) => {
                    let ll = -re_mle_neg_ll_from_params(
                        &new_params,
                        &endog_vec,
                        exog,
                        entity_id,
                        &kept_const,
                        &obs_per_entity,
                    );
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

    // 2. Use constant-only model's (σ²_u, σ²_e) as init for full model (Stata-style)
    let mut sigma2_e = sigma2_e_null;
    let mut sigma2_u = sigma2_u_null;
    let max_iter = 200;
    let betas: Vec<f64>;
    let kept: Vec<usize>;
    let mut mle_iter_log_lik: Vec<f64> = Vec::new();

    // Full model: use quasi-demeaned OLS for init, then Newton-Raphson on (β, ln σ²_u, ln σ²_e)
    {
        let theta_arr: Vec<f64> = (0..n)
            .map(|i| {
                let eid = entity_id[i];
                let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
                let denom = t_i * sigma2_u + sigma2_e;
                1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
            })
            .collect();
        let y_bar = between_transform(&endog_vec, entity_id);
        let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| endog_vec[i] - theta_arr[i] * y_bar[i]);
        let mut x_star = Array2::zeros((n, k));
        for c in 0..k {
            let col: Vec<f64> = exog.column(c).iter().cloned().collect();
            let x_bar = between_transform(&col, entity_id);
            for i in 0..n {
                x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
            }
        }
        let (x_star_use, omitted_star) = {
            let col_is_dummy = vec![false; k];
            let intercept_col = if constant { Some(0) } else { None };
            drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
                .map_err(|e| format!("Panel RE (MLE) iter 0: {}", e))?
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
        let res0 = ols_re.fit().map_err(|e| format!("Panel RE (MLE) iter 0: {}", e))?;
        betas = res0.betas.to_vec();
        let ll0_full = re_mle_log_lik(
            &endog_vec,
            exog,
            entity_id,
            &betas,
            &kept,
            sigma2_u,
            sigma2_e,
            &obs_per_entity,
        );
        mle_iter_log_lik.push(ll0_full);
    }

    // Newton-Raphson on (β, ln σ²_u, ln σ²_e)
    let mut params: Vec<f64> = betas.iter().cloned().collect();
    params.push(sigma2_u.ln());
    params.push(sigma2_e.ln());
    let h_num = 1e-6;
    for _ in 0..max_iter {
        match re_mle_newton_step(
            &params,
            &endog_vec,
            exog,
            entity_id,
            &kept,
            &obs_per_entity,
            h_num,
        ) {
            Ok((new_params, _neg_ll, converged)) => {
                let n_beta = kept.len();
                sigma2_u = new_params[n_beta].exp().clamp(1e-12, 1e10);
                sigma2_e = new_params[n_beta + 1].exp().clamp(1e-12, 1e10);
                let ll = -re_mle_neg_ll_from_params(
                    &new_params,
                    &endog_vec,
                    exog,
                    entity_id,
                    &kept,
                    &obs_per_entity,
                );
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
            let eid = entity_id[i];
            let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
            let denom = t_i * sigma2_u + sigma2_e;
            1.0 - (sigma2_e / denom.max(1e-300)).sqrt()
        })
        .collect();

    let y_bar = between_transform(&endog_vec, entity_id);
    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| endog_vec[i] - theta_arr[i] * y_bar[i]);

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar = between_transform(&col, entity_id);
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta_arr[i] * x_bar[i];
        }
    }

    let (x_star_use, omitted_mle) = {
        let col_is_dummy = vec![false; k];
        let intercept_col = if constant { Some(0) } else { None };
        drop_collinear_columns(&x_star, &col_is_dummy, intercept_col)
            .map_err(|e| format!("Panel RE (MLE) final: {}", e))?
    };

    // MLE always uses OIM standard errors; ignore cov_type/cov_params.
    let config = crate::regression::linear_model::OLSConfig {
        constant,
        cov_type: "nonrobust".to_string(),
        cov_params: None,
    };

    let mut result = OLS {
        endog: y_star,
        exog: x_star_use,
        config,
    }
    .fit()?;

    // OIM: Var(β̂) = σ²_e (X*'X*)^{-1}. OLS gives cov = ms_residual * (X*'X*)^{-1}.
    if result.ms_residual > 1e-300 {
        let scale = sigma2_e / result.ms_residual;
        result.cov_beta = &result.cov_beta * scale;
        result.stds = result.cov_beta.diag().mapv(f64::sqrt);
        result.tvalues = &result.betas / &result.stds;
    }

    let kept: Vec<usize> = (0..k).filter(|j| !omitted_mle.contains(j)).collect();
    let omitted_indices = if omitted_mle.is_empty() {
        None
    } else {
        Some(omitted_mle)
    };

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

    // MLE does not report R² Within/Between/Overall (Stata xtreg, mle)
    let sigma_u = sigma2_u.sqrt();
    let sigma_e = sigma2_e.sqrt();
    let rho = sigma2_u / (sigma2_u + sigma2_e);

    // MLE: log likelihood, LR chi2, chibar2 for sigma_u=0 (Stata xtreg, mle)
    let betas_vec: Vec<f64> = result.betas.iter().cloned().collect();
    let log_likelihood = re_mle_log_lik(
        &endog_vec,
        exog,
        entity_id,
        &betas_vec,
        &kept,
        sigma2_u,
        sigma2_e,
        &obs_per_entity,
    );
    let k_slopes = if constant && kept.len() > 1 { kept.len() - 1 } else { kept.len() };
    let lr_chi2 = {
        let raw = 2.0 * (log_likelihood - ll_null);
        if raw.is_nan() || raw.is_infinite() { 0.0 } else { raw.max(0.0) }
    };
    let chi2_lr = ChiSquared::new(k_slopes as f64).map_err(|e| format!("Panel RE MLE LR: {}", e))?;
    let prob_lr_chi2 = 1.0 - chi2_lr.cdf(lr_chi2);

    // chibar2(01) for H0: sigma_u=0. Restricted model = pooled OLS.
    let ll_ols = {
        let (x_ols_use, ols_omitted) = {
            let col_is_dummy = vec![false; k];
            let intercept_col = if constant { Some(0) } else { None };
            drop_collinear_columns(exog, &col_is_dummy, intercept_col)
                .map_err(|e| format!("Panel RE MLE OLS: {}", e))?
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
        let res_ols = ols_pooled.fit().map_err(|e| format!("Panel RE MLE OLS: {}", e))?;
        let ols_betas: Vec<f64> = res_ols.betas.iter().cloned().collect();
        let ols_kept: Vec<usize> = (0..k).filter(|j| !ols_omitted.contains(j)).collect();
        let sigma2_e_ols = (res_ols.ss_residual / n as f64).max(1e-12);
        re_mle_log_lik(
            &endog_vec,
            exog,
            entity_id,
            &ols_betas,
            &ols_kept,
            0.0,
            sigma2_e_ols,
            &obs_per_entity,
        )
    };
    let chibar2 = {
        let raw = 2.0 * (log_likelihood - ll_ols);
        if raw.is_nan() || raw.is_infinite() { 0.0 } else { raw.max(0.0) }
    };
    let chi2_1 = ChiSquared::new(1.0).map_err(|e| format!("Panel RE MLE chibar2: {}", e))?;
    let prob_chibar2 = 0.5 * (1.0 - chi2_1.cdf(chibar2));

    let mle_theta = {
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
        r2: None,
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb: 0.0,
        theta: Some(mle_theta),
    });

    // Stata xtreg, re uses z (asymptotic normal), not t
    let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("Panel RE MLE: {}", e))?;
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

// ============== Two-Way Random Effects ==============

