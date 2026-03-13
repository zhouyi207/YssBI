//! Panel Random Effects (GLS with variance components)
//!
//! Quasi-demeaning: y*_it = y_it - θ_i·ȳ_i, where θ_i = 1 - sqrt(σ²_e/(T_i·σ²_u + σ²_e)).
//! Stata xtreg, re default: consistent variance components (harmonic mean T̄ for σ²_u).

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::linear_model::OLS;
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::linalg::solvers::Solve;
use faer::Side;
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
    let mut cnt: HashMap<usize, usize> = HashMap::new();
    for &eid in entity_id {
        *cnt.entry(eid).or_insert(0) += 1;
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
    let mut sums_y: HashMap<usize, (f64, usize)> = HashMap::new();
    let k = exog.ncols();
    let mut sums_x: HashMap<usize, (Vec<f64>, usize)> = HashMap::new();

    for (i, &eid) in entity_id.iter().enumerate() {
        let val = endog[i];
        if !val.is_nan() {
            let entry = sums_y.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
        let entry = sums_x.entry(eid).or_insert_with(|| (vec![0.0; k], 0));
        for c in 0..k {
            entry.0[c] += exog[[i, c]];
        }
        entry.1 += 1;
    }

    let mut eids: Vec<usize> = sums_y.keys().copied().collect();
    eids.sort_unstable();
    let mut y_means = Vec::new();
    let mut x_means = Vec::new();
    for &eid in &eids {
        let (sy, cy) = sums_y.get(&eid).copied().unwrap_or((0.0, 0));
        let (sx, cx) = sums_x.get(&eid).cloned().unwrap_or_else(|| (vec![0.0; k], 0));
        y_means.push(if cy > 0 { sy / cy as f64 } else { 0.0 });
        x_means.push(if cx > 0 {
            sx.iter().map(|v| v / cx as f64).collect()
        } else {
            vec![0.0; k]
        });
    }
    (eids, y_means, x_means)
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
        r2_within,
        r2_between,
        r2_overall,
        obs_per_group_min: obs_min,
        obs_per_group_avg: obs_avg,
        obs_per_group_max: obs_max,
        sigma_u: sd_u_plus_avg_e,
        sigma_e: 0.0,
        rho: 0.0,
        corr_u_i_xb: 0.0,
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
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: None,
        prob_wald_chi2: None,
        log_likelihood: None,
        lr_chi2: None,
        prob_lr_chi2: None,
        chibar2: None,
        prob_chibar2: None,
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

    let fe_stats = Some(super::PanelFEStats {
        r2_within,
        r2_between,
        r2_overall,
        obs_per_group_min: obs_min,
        obs_per_group_avg: obs_avg,
        obs_per_group_max: obs_max,
        sigma_u,
        sigma_e,
        rho,
        corr_u_i_xb: 0.0,
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
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: Some(wald_chi2),
        prob_wald_chi2: Some(prob_wald_chi2),
        log_likelihood: None,
        lr_chi2: None,
        prob_lr_chi2: None,
        chibar2: None,
        prob_chibar2: None,
    })
}

/// RE MLE log likelihood (Stata xtreg, mle formula): l_i = -1/2 * ( 1/σ²_e * [ Σ_t r_it² - σ²_u/(T_i σ²_u + σ²_e) * s_i² ] + ln(T_i σ²_u/σ²_e + 1) + T_i ln(2π σ²_e) )
fn re_mle_log_lik(
    endog: &[f64],
    exog: &Array2<f64>,
    entity_id: &[usize],
    betas: &[f64],
    kept: &[usize],
    sigma2_u: f64,
    sigma2_e: f64,
    obs_per_entity: &HashMap<usize, usize>,
) -> f64 {
    let (eids, _, _) = entity_means(endog, exog, entity_id);
    let mut ll = 0.0;
    let two_pi = std::f64::consts::PI * 2.0;
    for &eid in &eids {
        let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
        let mut sum_r: f64 = 0.0;
        let mut sum_r2: f64 = 0.0;
        let mut count = 0usize;
        for (idx, &e) in entity_id.iter().enumerate() {
            if e != eid {
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

/// Panel Random Effects MLE (Stata xtreg, mle)
/// Uses Stata-style MLE: entity-specific θ_i, variance updates σ²_e = (1/N)Σ(y_it−x_itβ−û_i)², σ²_u = (1/n)Σû_i².
pub fn fit_panel_re_mle(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<crate::regression::covariance::CovParams>,
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
    {
        let r_w = within_transform(&endog_vec, entity_id);
        let r_w_ss: f64 = r_w.iter().map(|x| x * x).sum();
        let df_w = n.saturating_sub(n_entities).max(1);
        sigma2_e_null = (r_w_ss / df_w as f64).max(1e-12);
        let (_, y_b_vec, _) = entity_means(&endog_vec, exog, entity_id);
        let n_b = y_b_vec.len();
        let mut ss_b = 0.0;
        for &y in &y_b_vec {
            ss_b += (y - y_global_mean).powi(2);
        }
        let df_b = (n_b as i64 - 1).max(1) as usize;
        sigma2_u_null = (ss_b / df_b as f64 - sigma2_e_null / t_bar).max(1e-10);
        let max_iter_null = 50;
        for _ in 0..max_iter_null {
            let theta_arr_null: Vec<f64> = (0..n)
                .map(|i| {
                    let eid = entity_id[i];
                    let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
                    let denom = t_i * sigma2_u_null + sigma2_e_null;
                    1.0 - (sigma2_e_null / denom.max(1e-300)).sqrt()
                })
                .collect();
            let y_bar = between_transform(&endog_vec, entity_id);
            let y_star: Vec<f64> = (0..n).map(|i| endog_vec[i] - theta_arr_null[i] * y_bar[i]).collect();
            let mut const_col = vec![1.0; n];
            let x_const = Array2::from_shape_vec((n, 1), const_col).unwrap();
            let (x_use, _) = drop_collinear_columns(
                &x_const,
                &[false],
                Some(0),
            ).map_err(|e| format!("Panel RE (MLE) const-only: {}", e))?;
            let ols_null = OLS {
                endog: Array1::from_vec(y_star),
                exog: x_use,
                config: crate::regression::linear_model::OLSConfig {
                    constant: true,
                    cov_type: "nonrobust".to_string(),
                    cov_params: None,
                },
            };
            let res_null = ols_null.fit().map_err(|e| format!("Panel RE (MLE) const-only: {}", e))?;
            let alpha = res_null.betas[0];
            let mut resid_null: Vec<f64> = (0..n).map(|i| endog_vec[i] - alpha).collect();
            let (eids, y_b_vec, _) = entity_means(&resid_null, exog, entity_id);
            let mut u_hat: Vec<f64> = Vec::with_capacity(n_entities);
            for (i, &eid) in eids.iter().enumerate() {
                let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
                let y_bar_i = y_b_vec[i];
                let denom = t_i * sigma2_u_null + sigma2_e_null;
                let weight = if denom > 1e-300 { t_i * sigma2_u_null / denom } else { 0.0 };
                u_hat.push(weight * y_bar_i);
            }
            let mut u_hat_by_entity: HashMap<usize, f64> = HashMap::new();
            for (i, &eid) in eids.iter().enumerate() {
                u_hat_by_entity.insert(eid, u_hat[i]);
            }
            let mut ss_resid = 0.0;
            for (idx, &eid) in entity_id.iter().enumerate() {
                let u_i = *u_hat_by_entity.get(&eid).unwrap_or(&0.0);
                ss_resid += (resid_null[idx] - u_i).powi(2);
            }
            let sigma2_e_new2 = (ss_resid / n as f64).max(1e-12);
            let sigma2_u_new2 = (u_hat.iter().map(|x| x * x).sum::<f64>() / n_entities as f64).max(1e-12);
            ll_null = re_mle_log_lik(
                &endog_vec,
                exog,
                entity_id,
                &[alpha],
                &[0],
                sigma2_u_new2,
                sigma2_e_new2,
                &obs_per_entity,
            );
            if (sigma2_e_new2 - sigma2_e_null).abs() < 1e-8 && (sigma2_u_new2 - sigma2_u_null).abs() < 1e-8 {
                break;
            }
            sigma2_e_null = sigma2_e_new2;
            sigma2_u_null = sigma2_u_new2;
        }
    }

    // 2. FGLS-style init for full model
    let (sigma2_e_init, sigma2_u_init) = {
        let (_, y_b_vec, x_b_vec) = entity_means(&endog_vec, exog, entity_id);
        let n_b = y_b_vec.len();
        let mut x_b_data = Vec::with_capacity(n_b * k);
        for i in 0..n_b {
            for c in 0..k {
                x_b_data.push(x_b_vec[i][c]);
            }
        }
        let y_b = Array1::from_vec(y_b_vec);
        let x_b = Array2::from_shape_vec((n_b, k), x_b_data).unwrap();
        let (x_b_use, _) = {
            let col_is_dummy = vec![false; k];
            let intercept_col = if constant { Some(0) } else { None };
            drop_collinear_columns(&x_b, &col_is_dummy, intercept_col)
                .map_err(|e| format!("Panel RE (MLE) between init: {}", e))?
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
        let res_b = ols_b.fit().map_err(|e| format!("Panel RE (MLE) between: {}", e))?;
        let r_w = within_transform(&endog_vec, entity_id);
        let r_w_ss: f64 = r_w.iter().map(|x| x * x).sum();
        let df_w = n.saturating_sub(n_entities).saturating_sub(k).max(1);
        let sigma2_e = (r_w_ss / df_w as f64).max(1e-12);
        let df_b = res_b.df_residual;
        let sigma2_u = if df_b > 0 {
            (res_b.ss_residual / df_b as f64 - sigma2_e / t_bar).max(1e-10)
        } else {
            1e-10
        };
        (sigma2_e, sigma2_u)
    };

    let mut sigma2_e = sigma2_e_init;
    let mut sigma2_u = sigma2_u_init;
    let max_iter = 100;
    let tol = 1e-8;
    let mut betas: Vec<f64> = vec![0.0; k];
    let mut kept: Vec<usize> = (0..k).collect();

    for _iter in 0..max_iter {
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
                .map_err(|e| format!("Panel RE (MLE) iter: {}", e))?
        };
        kept = (0..k).filter(|j| !omitted_star.contains(j)).collect();

        let ols_re = OLS {
            endog: y_star.clone(),
            exog: x_star_use,
            config: crate::regression::linear_model::OLSConfig {
                constant,
                cov_type: "nonrobust".to_string(),
                cov_params: None,
            },
        };
        let res = ols_re.fit().map_err(|e| format!("Panel RE (MLE) iter: {}", e))?;
        betas = res.betas.to_vec();

        let (eids, y_b_vec, x_b_vec) = entity_means(&endog_vec, exog, entity_id);
        let mut u_hat: Vec<f64> = Vec::with_capacity(n_entities);
        for (i, &eid) in eids.iter().enumerate() {
            let t_i = *obs_per_entity.get(&eid).unwrap_or(&1) as f64;
            let mut xb_bar = 0.0;
            for (j, &c) in kept.iter().enumerate() {
                xb_bar += x_b_vec[i][c] * betas[j];
            }
            let r_bar_i = y_b_vec[i] - xb_bar;
            let denom = t_i * sigma2_u + sigma2_e;
            let weight = if denom > 1e-300 { t_i * sigma2_u / denom } else { 0.0 };
            u_hat.push(weight * r_bar_i);
        }
        let mut u_hat_by_entity: HashMap<usize, f64> = HashMap::new();
        for (i, &eid) in eids.iter().enumerate() {
            u_hat_by_entity.insert(eid, u_hat[i]);
        }

        let mut ss_resid = 0.0;
        for (idx, &eid) in entity_id.iter().enumerate() {
            let mut xb = 0.0;
            for (j, &c) in kept.iter().enumerate() {
                xb += exog[[idx, c]] * betas[j];
            }
            let r = endog_vec[idx] - xb;
            let u_i = *u_hat_by_entity.get(&eid).unwrap_or(&0.0);
            ss_resid += (r - u_i).powi(2);
        }
        let sigma2_e_new = (ss_resid / n as f64).max(1e-12);
        let sigma2_u_new = (u_hat.iter().map(|x| x * x).sum::<f64>() / n_entities as f64).max(1e-12);

        if sigma2_u_new <= 0.0 {
            return Err("Panel RE (MLE): sigma_u^2 <= 0 (try FE instead)".to_string());
        }

        let converged = (sigma2_e_new - sigma2_e).abs() < tol && (sigma2_u_new - sigma2_u).abs() < tol;
        sigma2_e = sigma2_e_new;
        sigma2_u = sigma2_u_new;
        if converged {
            break;
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

    let result = OLS {
        endog: y_star,
        exog: x_star_use,
        config,
    }
    .fit()?;

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

    // R² Within, Between, Overall (same formulas as FGLS)
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

    let (_, y_b_vec, x_b_vec) = entity_means(
        &endog.iter().cloned().collect::<Vec<_>>(),
        exog,
        entity_id,
    );
    let n_b = y_b_vec.len();
    let r2_between = {
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

    let fe_stats = Some(super::PanelFEStats {
        r2_within,
        r2_between,
        r2_overall,
        obs_per_group_min: obs_min,
        obs_per_group_avg: obs_avg,
        obs_per_group_max: obs_max,
        sigma_u,
        sigma_e,
        rho,
        corr_u_i_xb: 0.0,
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
        cond_no: result.cond_no,
        omitted_indices,
        wald_chi2: None,
        prob_wald_chi2: None,
        log_likelihood: Some(log_likelihood),
        lr_chi2: Some(lr_chi2),
        prob_lr_chi2: Some(prob_lr_chi2),
        chibar2: Some(chibar2),
        prob_chibar2: Some(prob_chibar2),
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
