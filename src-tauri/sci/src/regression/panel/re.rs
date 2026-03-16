//! Panel Random Effects (GLS with variance components)
//!
//! Quasi-demeaning: y*_it = y_it - θ_i·ȳ_i, where θ_i = 1 - sqrt(σ²_e/(T_i·σ²_u + σ²_e)).
//! Stata xtreg, re default: consistent variance components (harmonic mean T̄ for σ²_u).

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::linear_model::OLS;
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::linalg::solvers::Solve;
use faer::{Mat, Side};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use std::collections::HashMap;

/// Within transformation (same as FE)
fn within_transform(v: &[f64], entity_id: &[usize]) -> Array1<f64> {
    let n = v.len();
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &eid) in entity_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    let out: Vec<f64> = (0..n)
        .map(|i| {
            let eid = entity_id[i];
            let (s, cnt) = sums.get(&eid).copied().unwrap_or((0.0, 0));
            let mean = if cnt > 0 { s / cnt as f64 } else { 0.0 };
            v[i] - mean
        })
        .collect();
    Array1::from_vec(out)
}

/// Between transformation: replace each obs with entity mean (for quasi-demeaning)
fn between_transform(v: &[f64], entity_id: &[usize]) -> Array1<f64> {
    let n = v.len();
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &eid) in entity_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    let out: Vec<f64> = (0..n)
        .map(|i| {
            let eid = entity_id[i];
            let (s, cnt) = sums.get(&eid).copied().unwrap_or((0.0, 0));
            if cnt > 0 {
                s / cnt as f64
            } else {
                v[i]
            }
        })
        .collect();
    Array1::from_vec(out)
}

/// Obs per entity T_i and harmonic mean T̄ = n / Σ(1/T_i) (Stata xtreg, re)
fn obs_per_entity_and_harmonic_mean(entity_id: &[usize]) -> (HashMap<usize, usize>, f64) {
    obs_per_group_and_harmonic_mean(entity_id)
}

/// Obs per group T_i and harmonic mean T̄ = n / Σ(1/T_i). Generic over group_id.
fn obs_per_group_and_harmonic_mean(group_id: &[usize]) -> (HashMap<usize, usize>, f64) {
    let mut cnt: HashMap<usize, usize> = HashMap::new();
    for &gid in group_id {
        *cnt.entry(gid).or_insert(0) += 1;
    }
    let n = cnt.len();
    let inv_sum: f64 = cnt.values().map(|&t| 1.0 / (t as f64).max(1e-10)).sum();
    let t_bar_harmonic = if inv_sum > 1e-300 { n as f64 / inv_sum } else { 0.0 };
    (cnt, t_bar_harmonic)
}

/// Compute entity-level means. Returns (entity_ids, y_means, x_means) for between regression.
fn entity_means(
    endog: &[f64],
    exog: &Array2<f64>,
    entity_id: &[usize],
) -> (Vec<usize>, Vec<f64>, Vec<Vec<f64>>) {
    group_means(endog, exog, entity_id)
}

/// Compute group-level means (generic for entity or time). Returns (group_ids, y_means, x_means).
fn group_means(
    endog: &[f64],
    exog: &Array2<f64>,
    group_id: &[usize],
) -> (Vec<usize>, Vec<f64>, Vec<Vec<f64>>) {
    let mut sums_y: HashMap<usize, (f64, usize)> = HashMap::new();
    let k = exog.ncols();
    let mut sums_x: HashMap<usize, (Vec<f64>, usize)> = HashMap::new();

    for (i, &gid) in group_id.iter().enumerate() {
        let val = endog[i];
        if !val.is_nan() {
            let entry = sums_y.entry(gid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
        let entry = sums_x.entry(gid).or_insert_with(|| (vec![0.0; k], 0));
        for c in 0..k {
            entry.0[c] += exog[[i, c]];
        }
        entry.1 += 1;
    }

    let mut gids: Vec<usize> = sums_y.keys().copied().collect();
    gids.sort_unstable();
    let mut y_means = Vec::new();
    let mut x_means = Vec::new();
    for &gid in &gids {
        let (sy, cy) = sums_y.get(&gid).copied().unwrap_or((0.0, 0));
        let (sx, cx) = sums_x.get(&gid).cloned().unwrap_or_else(|| (vec![0.0; k], 0));
        y_means.push(if cy > 0 { sy / cy as f64 } else { 0.0 });
        x_means.push(if cx > 0 {
            sx.iter().map(|v| v / cx as f64).collect()
        } else {
            vec![0.0; k]
        });
    }
    (gids, y_means, x_means)
}

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
    let mut sigma2_e_null = 0.0;
    let mut sigma2_u_null = 0.0;
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
    let mut betas: Vec<f64> = vec![0.0; k];
    let mut kept: Vec<usize> = (0..k).collect();
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
                betas = new_params[..n_beta].to_vec();
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

/// Alias for backward compatibility
pub fn fit_panel_re(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
) -> Result<super::PanelOLSResult, String> {
    fit_panel_re_fgls(endog, exog, entity_id, constant, cov_type, cov_params)
}

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
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
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
    let mut sigma2_e_null = 0.0;
    let mut sigma2_u_null = 0.0;
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
    let mut betas: Vec<f64> = vec![0.0; k];
    let mut kept: Vec<usize> = (0..k).collect();
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
                betas = new_params[..n_beta].to_vec();
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
