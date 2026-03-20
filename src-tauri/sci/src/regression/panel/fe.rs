//! Panel Fixed Effects (Within transformation)
//!
//! Demeans variables within each entity, then runs OLS.
//! Standard errors: cluster by entity (default).
//! Stata xtreg, fe style: R2 Within/Between/Overall, sigma_u, sigma_e, rho, corr(u_i,Xb).

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::covariance::CovParams;
use crate::regression::linear_model::{OLSConfig, OLS};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use std::collections::HashMap;

/// Compute FE-specific stats (Stata xtreg, fe style)
fn compute_fe_stats(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    n_entities: usize,
    betas: &Array1<f64>,
    const_coef: f64,
    r2_within: f64,
    ss_residual: f64,
) -> Result<super::PanelFEStats, String> {
    let _n = endog.len();
    let k = exog.ncols();
    let k_vars = betas.len(); // exog cols 1..k (non-const)
    if k_vars + 1 != k {
        return Err(format!(
            "FE stats: betas len {} + 1 != exog cols {}",
            k_vars, k
        ));
    }

    // Entity-level: sums and counts
    let mut sums_y: HashMap<usize, (f64, usize)> = HashMap::new();
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

    let mut eids: Vec<usize> = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    eids.sort_unstable();
    let mut y_bar = Vec::with_capacity(n_entities);
    let mut x_bar = Vec::with_capacity(n_entities);
    let mut obs_per_entity = Vec::with_capacity(n_entities);
    for &eid in &eids {
        let (sy, cy) = sums_y.get(&eid).copied().unwrap_or((0.0, 0));
        let (sx, cx) = sums_x.get(&eid).cloned().unwrap_or_else(|| (vec![0.0; k], 0));
        y_bar.push(if cy > 0 { sy / cy as f64 } else { 0.0 });
        x_bar.push(if cx > 0 {
            sx.iter().map(|v| v / cx as f64).collect()
        } else {
            vec![0.0; k]
        });
        obs_per_entity.push(cx);
    }

    let obs_min = *obs_per_entity.iter().min().unwrap_or(&0);
    let obs_max = *obs_per_entity.iter().max().unwrap_or(&0);
    let obs_avg = obs_per_entity.iter().sum::<usize>() as f64 / n_entities as f64;

    // R2 Between = corr(x̄_i β̂, ȳ_i)² (Stata: correlation squared, not OLS R2)
    let xb_entity: Vec<f64> = (0..n_entities)
        .map(|i| const_coef + (0..k_vars).map(|c| x_bar[i][c + 1] * betas[c]).sum::<f64>())
        .collect();
    let r2_between = {
        let (xb_mean, y_mean) = (
            xb_entity.iter().sum::<f64>() / n_entities as f64,
            y_bar.iter().sum::<f64>() / n_entities as f64,
        );
        let cov = xb_entity.iter().zip(y_bar.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>()
            / (n_entities as f64 - 1.0).max(1.0);
        let (var_xb, var_y) = (
            xb_entity.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_entities as f64 - 1.0).max(1.0),
            y_bar.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n_entities as f64 - 1.0).max(1.0),
        );
        if (var_xb * var_y).sqrt() > 1e-300 {
            (cov / (var_xb * var_y).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    // R2 Overall = corr(x_it β̂, y_it)² (Stata: correlation squared)
    let n_total = endog.len();
    let xb_obs: Vec<f64> = (0..n_total)
        .map(|i| const_coef + (0..k_vars).map(|c| exog[[i, c + 1]] * betas[c]).sum::<f64>())
        .collect();
    let r2_overall = {
        let (xb_mean, y_mean) = (
            xb_obs.iter().sum::<f64>() / n_total as f64,
            endog.iter().sum::<f64>() / n_total as f64,
        );
        let cov = xb_obs.iter().zip(endog.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>()
            / (n_total as f64 - 1.0).max(1.0);
        let (var_xb, var_y) = (
            xb_obs.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
            endog.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
        );
        if (var_xb * var_y).sqrt() > 1e-300 {
            (cov / (var_xb * var_y).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    // sigma_e: Stata xtreg fe "adjusted for the n-1 estimated means" (df = N - n - k)
    let df_sigma_e = (n_total as i64 - n_entities as i64 - k_vars as i64).max(1) as usize;
    let sigma_e = (ss_residual / df_sigma_e as f64).max(0.0).sqrt();

    // u_i = ȳ_i - ̂α - x̄_i'β̂ (Stata formula)
    let u_i: Vec<f64> = (0..n_entities)
        .map(|i| y_bar[i] - xb_entity[i])
        .collect();
    let u_mean = u_i.iter().sum::<f64>() / n_entities as f64;
    let u_var = u_i.iter().map(|u| (u - u_mean).powi(2)).sum::<f64>() / (n_entities as f64 - 1.0).max(1.0);
    let sigma_u = u_var.max(0.0).sqrt();

    let rho = {
        let su2 = sigma_u * sigma_u;
        let se2 = sigma_e * sigma_e;
        if su2 + se2 > 1e-300 {
            su2 / (su2 + se2)
        } else {
            0.0
        }
    };

    // corr(u_i, Xb): Stata e(corr) = corr(u_i, x_it β) at observation level
    let eid_to_idx: HashMap<usize, usize> = eids.iter().enumerate().map(|(i, &eid)| (eid, i)).collect();
    let u_expanded: Vec<f64> = entity_id.iter()
        .map(|&eid| u_i[eid_to_idx[&eid]])
        .collect();
    let corr_u_i_xb = {
        let u_exp_mean = u_expanded.iter().sum::<f64>() / n_total as f64;
        let xb_mean = xb_obs.iter().sum::<f64>() / n_total as f64;
        let cov = u_expanded.iter().zip(xb_obs.iter())
            .map(|(u, xb)| (u - u_exp_mean) * (xb - xb_mean)).sum::<f64>()
            / (n_total as f64 - 1.0).max(1.0);
        let (var_u, var_xb) = (
            u_expanded.iter().map(|u| (u - u_exp_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
            xb_obs.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
        );
        if (var_u * var_xb).sqrt() > 1e-300 {
            (cov / (var_u * var_xb).sqrt()).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };

    Ok(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb,
        theta: None,
    })
}

/// Compute FE-specific stats for time FE (group by time period)
fn compute_fe_stats_time(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    time_id: &[usize],
    n_times: usize,
    betas: &Array1<f64>,
    const_coef: f64,
    r2_within: f64,
    ss_residual: f64,
) -> Result<super::PanelFEStats, String> {
    let n_total = endog.len();
    let k = exog.ncols();
    let k_vars = betas.len();
    if k_vars + 1 != k {
        return Err(format!(
            "FE stats (time): betas len {} + 1 != exog cols {}",
            k_vars, k
        ));
    }

    // Time-level: sums and counts
    let mut sums_y: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut sums_x: HashMap<usize, (Vec<f64>, usize)> = HashMap::new();
    for (i, &tid) in time_id.iter().enumerate() {
        let val = endog[i];
        if !val.is_nan() {
            let entry = sums_y.entry(tid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
        let entry = sums_x.entry(tid).or_insert_with(|| (vec![0.0; k], 0));
        for c in 0..k {
            entry.0[c] += exog[[i, c]];
        }
        entry.1 += 1;
    }

    let mut tids: Vec<usize> = time_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    tids.sort_unstable();
    let mut y_bar = Vec::with_capacity(n_times);
    let mut x_bar = Vec::with_capacity(n_times);
    let mut obs_per_time = Vec::with_capacity(n_times);
    for &tid in &tids {
        let (sy, cy) = sums_y.get(&tid).copied().unwrap_or((0.0, 0));
        let (sx, cx) = sums_x.get(&tid).cloned().unwrap_or_else(|| (vec![0.0; k], 0));
        y_bar.push(if cy > 0 { sy / cy as f64 } else { 0.0 });
        x_bar.push(if cx > 0 {
            sx.iter().map(|v| v / cx as f64).collect()
        } else {
            vec![0.0; k]
        });
        obs_per_time.push(cx);
    }

    let obs_min = *obs_per_time.iter().min().unwrap_or(&0);
    let obs_max = *obs_per_time.iter().max().unwrap_or(&0);
    let obs_avg = obs_per_time.iter().sum::<usize>() as f64 / n_times as f64;

    // R2 Between = corr(x̄_t β̂, ȳ_t)²
    let xb_time: Vec<f64> = (0..n_times)
        .map(|i| const_coef + (0..k_vars).map(|c| x_bar[i][c + 1] * betas[c]).sum::<f64>())
        .collect();
    let r2_between = {
        let (xb_mean, y_mean) = (
            xb_time.iter().sum::<f64>() / n_times as f64,
            y_bar.iter().sum::<f64>() / n_times as f64,
        );
        let cov = xb_time.iter().zip(y_bar.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>()
            / (n_times as f64 - 1.0).max(1.0);
        let (var_xb, var_y) = (
            xb_time.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_times as f64 - 1.0).max(1.0),
            y_bar.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n_times as f64 - 1.0).max(1.0),
        );
        if (var_xb * var_y).sqrt() > 1e-300 {
            (cov / (var_xb * var_y).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    // R2 Overall = corr(x_it β̂, y_it)²
    let xb_obs: Vec<f64> = (0..n_total)
        .map(|i| const_coef + (0..k_vars).map(|c| exog[[i, c + 1]] * betas[c]).sum::<f64>())
        .collect();
    let r2_overall = {
        let (xb_mean, y_mean) = (
            xb_obs.iter().sum::<f64>() / n_total as f64,
            endog.iter().sum::<f64>() / n_total as f64,
        );
        let cov = xb_obs.iter().zip(endog.iter())
            .map(|(xb, y)| (xb - xb_mean) * (y - y_mean)).sum::<f64>()
            / (n_total as f64 - 1.0).max(1.0);
        let (var_xb, var_y) = (
            xb_obs.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
            endog.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
        );
        if (var_xb * var_y).sqrt() > 1e-300 {
            (cov / (var_xb * var_y).sqrt()).powi(2).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };

    // sigma_e: df = N - T - k
    let df_sigma_e = (n_total as i64 - n_times as i64 - k_vars as i64).max(1) as usize;
    let sigma_e = (ss_residual / df_sigma_e as f64).max(0.0).sqrt();

    // λ_t = ȳ_t - ̂α - x̄_t'β̂ (time effects)
    let lambda_t: Vec<f64> = (0..n_times)
        .map(|i| y_bar[i] - xb_time[i])
        .collect();
    let lambda_mean = lambda_t.iter().sum::<f64>() / n_times as f64;
    let lambda_var = lambda_t.iter().map(|u| (u - lambda_mean).powi(2)).sum::<f64>() / (n_times as f64 - 1.0).max(1.0);
    let sigma_u = lambda_var.max(0.0).sqrt();

    let rho = {
        let su2 = sigma_u * sigma_u;
        let se2 = sigma_e * sigma_e;
        if su2 + se2 > 1e-300 {
            su2 / (su2 + se2)
        } else {
            0.0
        }
    };

    // corr(λ_t, Xb) at observation level
    let tid_to_idx: HashMap<usize, usize> = tids.iter().enumerate().map(|(i, &tid)| (tid, i)).collect();
    let lambda_expanded: Vec<f64> = time_id.iter()
        .map(|&tid| lambda_t[tid_to_idx[&tid]])
        .collect();
    let corr_u_i_xb = {
        let u_exp_mean = lambda_expanded.iter().sum::<f64>() / n_total as f64;
        let xb_mean = xb_obs.iter().sum::<f64>() / n_total as f64;
        let cov = lambda_expanded.iter().zip(xb_obs.iter())
            .map(|(u, xb)| (u - u_exp_mean) * (xb - xb_mean)).sum::<f64>()
            / (n_total as f64 - 1.0).max(1.0);
        let (var_u, var_xb) = (
            lambda_expanded.iter().map(|u| (u - u_exp_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
            xb_obs.iter().map(|xb| (xb - xb_mean).powi(2)).sum::<f64>() / (n_total as f64 - 1.0).max(1.0),
        );
        if (var_u * var_xb).sqrt() > 1e-300 {
            (cov / (var_u * var_xb).sqrt()).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };

    Ok(super::PanelFEStats {
        r2: Some(super::PanelR2Stats { r2_within, r2_between, r2_overall }),
        obs_per_group: super::ObsPerGroupStats { min: obs_min, avg: obs_avg, max: obs_max },
        sigma: super::SigmaStats { sigma_u, sigma_e, rho },
        corr_u_i_xb,
        theta: None,
    })
}

/// Within transformation by group: subtract group-specific mean
fn within_transform_by_group(v: &[f64], group_id: &[usize]) -> Result<Array1<f64>, String> {
    let n = v.len();
    if group_id.len() != n {
        return Err(format!(
            "within_transform_by_group: group_id len {} != data len {}",
            group_id.len(),
            n
        ));
    }
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &gid) in group_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(gid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }
    let mut out = Vec::with_capacity(n);
    for (i, &gid) in group_id.iter().enumerate() {
        let (s, cnt) = sums.get(&gid).copied().unwrap_or((0.0, 0));
        let mean = if cnt > 0 { s / cnt as f64 } else { 0.0 };
        out.push(v[i] - mean);
    }
    Ok(Array1::from_vec(out))
}

/// Within transformation: subtract entity-specific mean (alias for entity FE)
fn within_transform(v: &[f64], entity_id: &[usize]) -> Result<Array1<f64>, String> {
    within_transform_by_group(v, entity_id)
}

/// Two-way within transformation: z̃_it = z_it - z̄_i - z̄_t + z̄
fn within_transform_twoway(
    v: &[f64],
    entity_id: &[usize],
    time_id: &[usize],
) -> Result<Array1<f64>, String> {
    let n = v.len();
    if entity_id.len() != n || time_id.len() != n {
        return Err("within_transform_twoway: lengths must match".to_string());
    }
    let mut e_sums: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut t_sums: HashMap<usize, (f64, usize)> = HashMap::new();
    let mut total = 0.0;
    let mut total_cnt = 0usize;
    for (i, &val) in v.iter().enumerate() {
        if !val.is_nan() {
            let e_entry = e_sums.entry(entity_id[i]).or_insert((0.0, 0));
            e_entry.0 += val;
            e_entry.1 += 1;
            let t_entry = t_sums.entry(time_id[i]).or_insert((0.0, 0));
            t_entry.0 += val;
            t_entry.1 += 1;
            total += val;
            total_cnt += 1;
        }
    }
    let z_bar = if total_cnt > 0 { total / total_cnt as f64 } else { 0.0 };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (es, ec) = e_sums.get(&entity_id[i]).copied().unwrap_or((0.0, 0));
        let e_bar = if ec > 0 { es / ec as f64 } else { 0.0 };
        let (ts, tc) = t_sums.get(&time_id[i]).copied().unwrap_or((0.0, 0));
        let t_bar = if tc > 0 { ts / tc as f64 } else { 0.0 };
        out.push(v[i] - e_bar - t_bar + z_bar);
    }
    Ok(Array1::from_vec(out))
}

/// Panel Fixed Effects estimator
pub fn fit_panel_fe(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n {
        return Err(format!(
            "Panel FE: exog rows {} != endog len {}",
            exog.nrows(),
            n
        ));
    }
    if entity_id.len() != n {
        return Err(format!(
            "Panel FE: entity_id len {} != n {}",
            entity_id.len(),
            n
        ));
    }

    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 {
        return Err("Panel FE: need at least 2 entities".to_string());
    }

    // Within transform endog and each column of exog
    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_tilde = within_transform(&y_vec, entity_id)?;

    let k = exog.ncols();
    let mut x_tilde = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let transformed = within_transform(&col, entity_id)?;
        for (i, &v) in transformed.iter().enumerate() {
            x_tilde[[i, c]] = v;
        }
    }

    // After within transform, constant column (all 1s) becomes zero - drop it if present
    let (x_after_const, has_const) = if constant && k > 0 {
        let first_col = x_tilde.column(0);
        let is_const = first_col.iter().all(|&v| v.abs() < 1e-10);
        if is_const {
            (x_tilde.slice(ndarray::s![.., 1..]).to_owned(), false)
        } else {
            (x_tilde, constant)
        }
    } else {
        (x_tilde, constant)
    };

    // Drop collinear columns in transformed matrix (e.g. time-invariant vars → zero in entity FE)
    let k_after_const = x_after_const.ncols();
    let col_is_dummy = vec![false; k_after_const];
    let (x_use, omitted_x) = drop_collinear_columns(&x_after_const, &col_is_dummy, None)?;
    if x_use.ncols() == 0 {
        return Err(
            "Panel FE: no regressors left after within transform and collinearity drop (all absorbed or redundant)"
                .to_string(),
        );
    }
    let omitted_indices: Option<Vec<usize>> = if omitted_x.is_empty() {
        None
    } else {
        // Map to full coefficient indices: const=0, slopes at 1..; x_after_const col j = slope j+1
        Some(omitted_x.iter().map(|&j| j + 1).collect())
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: entity_id.to_vec(),
                xtreg_fe_style: true,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: has_const,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: y_tilde,
        exog: x_use,
        config,
    };

    let result = ols.fit()?;

    // Stata xtreg, fe with vce(cluster): F-test and t-test use df_residual = n_entities - 1
    let use_cluster_df = cov_type == "cluster";
    let df_residual = if use_cluster_df {
        n_entities.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };
    let df_model = result.df_model; // F test is for slope coefficients only (excl. constant)
    let df_total = df_model + df_residual;

    use statrs::distribution::{ContinuousCDF, FisherSnedecor, StudentsT};
    // Stata xtreg, fe vce(cluster): F = Wald/k with df(k, M-1). Wald = β'V^{-1}β (slope coeffs only).
    let (fvalue, f_p_value) = if use_cluster_df {
        let beta_s = &result.betas;
        let v_s = &result.cov_beta;
        let wald = if df_model == 0 {
            0.0
        } else {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            let x = v_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "Panel FE: cluster cov_beta not positive definite for Wald F".to_string())?
                .solve(beta_faer.as_ref());
            let x_nd = x.as_ref().into_ndarray();
            beta_s.dot(&x_nd)
        };
        let f = if df_model > 0 { (wald / df_model as f64).max(0.0) } else { 0.0 };
        let df1 = (df_model as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel FE FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (result.fvalue, result.f_p_value)
    };

    // Recovered constant _cons = ȳ - β'x̄ (Stata xtreg, fe style)
    let y_mean = endog.iter().sum::<f64>() / n as f64;
    let k_vars = result.betas.len();
    let kept_slope: Vec<usize> = (0..k_after_const)
        .filter(|&j| !omitted_x.contains(&j))
        .collect();
    let x_mean: Array1<f64> = (0..k_vars)
        .map(|c| exog.column(kept_slope[c] + 1).iter().sum::<f64>() / n as f64)
        .collect();
    let const_coef = y_mean - result.betas.dot(&x_mean);
    let var_const = x_mean.dot(&result.cov_beta).dot(&x_mean)
        + result.ms_residual / n as f64;
    let const_std_err = var_const.max(0.0).sqrt();

    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel FE StudentsT: {}", e))?;
    let const_t = const_coef / const_std_err;
    let const_p = 2.0 * (1.0 - t_dist.cdf(const_t.abs()));
    let t_crit = t_dist.inverse_cdf(0.975);
    let const_ci_l = const_coef - t_crit * const_std_err;
    let const_ci_u = const_coef + t_crit * const_std_err;

    let mut betas = ndarray::Array1::zeros(k_vars + 1);
    betas[0] = const_coef;
    for i in 0..k_vars {
        betas[i + 1] = result.betas[i];
    }
    let mut stds = ndarray::Array1::zeros(k_vars + 1);
    stds[0] = const_std_err;
    for i in 0..k_vars {
        stds[i + 1] = result.stds[i];
    }
    let mut tvalues = ndarray::Array1::zeros(k_vars + 1);
    tvalues[0] = const_t;
    for i in 0..k_vars {
        tvalues[i + 1] = result.tvalues[i];
    }
    let mut pvalues = ndarray::Array1::zeros(k_vars + 1);
    pvalues[0] = const_p;
    let mut conf_int_left = ndarray::Array1::zeros(k_vars + 1);
    conf_int_left[0] = const_ci_l;
    let mut conf_int_right = ndarray::Array1::zeros(k_vars + 1);
    conf_int_right[0] = const_ci_u;
    for i in 0..k_vars {
        pvalues[i + 1] = if use_cluster_df {
            2.0 * (1.0 - t_dist.cdf(result.tvalues[i].abs()))
        } else {
            result.pvalues[i]
        };
        conf_int_left[i + 1] = result.betas[i] - t_crit * result.stds[i];
        conf_int_right[i + 1] = result.betas[i] + t_crit * result.stds[i];
    }

    let mut cov_beta = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    cov_beta[[0, 0]] = var_const;
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_beta[[i + 1, j + 1]] = result.cov_beta[[i, j]];
        }
        let cov_const_beta_i = -x_mean.dot(&result.cov_beta.column(i));
        cov_beta[[0, i + 1]] = cov_const_beta_i;
        cov_beta[[i + 1, 0]] = cov_const_beta_i;
    }
    let mut cov_nr = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_nr[[i + 1, j + 1]] = result.cov_beta_nonrobust[[i, j]];
        }
    }

    // Stata xtreg, fe style: R2 Between/Overall, obs per group, sigma_u, sigma_e, rho, corr(u_i,Xb)
    let kept_cols: Vec<usize> = std::iter::once(0)
        .chain(kept_slope.iter().map(|&j| j + 1))
        .collect();
    let exog_kept = exog.select(ndarray::Axis(1), &kept_cols);
    let fe_stats = compute_fe_stats(
        endog,
        &exog_kept,
        entity_id,
        n_entities,
        &result.betas,
        const_coef,
        result.r2,
        result.ss_residual,
    )?;

    Ok(super::PanelOLSResult {
        const_coef: Some(const_coef),
        const_std_err: Some(const_std_err),
        fe_stats: Some(fe_stats),
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: 0,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total: result.ss_total / df_total as f64,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas,
        stds,
        tvalues,
        pvalues,
        conf_int_left,
        conf_int_right,
        cov_beta,
        cov_beta_nonrobust: Some(cov_nr),
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

/// Panel Time Fixed Effects: within transformation by time period
///
/// Analogous to entity FE but demeans by time: (y_it - ȳ_t) = (X_it - X̄_t)'β + (u_it - ū_t).
/// Cluster by time when vce(cluster): residuals correlated within time period.
pub fn fit_panel_fe_time(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel FE (Time): lengths must match".to_string());
    }
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_times < 2 {
        return Err("Panel FE (Time): need at least 2 time periods".to_string());
    }

    // Within transform by time: z̃_it = z_it - z̄_t
    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_tilde = within_transform_by_group(&y_vec, time_id)?;

    let k = exog.ncols();
    let mut x_tilde = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let transformed = within_transform_by_group(&col, time_id)?;
        for (i, &v) in transformed.iter().enumerate() {
            x_tilde[[i, c]] = v;
        }
    }

    // After within transform, constant column becomes zero - drop if present
    let (x_after_const, has_const) = if constant && k > 0 {
        let first_col = x_tilde.column(0);
        let is_const = first_col.iter().all(|&v| v.abs() < 1e-10);
        if is_const {
            (x_tilde.slice(ndarray::s![.., 1..]).to_owned(), false)
        } else {
            (x_tilde, constant)
        }
    } else {
        (x_tilde, constant)
    };

    // Drop collinear columns (e.g. entity-invariant vars → zero in time FE)
    let k_after_const = x_after_const.ncols();
    let col_is_dummy = vec![false; k_after_const];
    let (x_use, omitted_x) = drop_collinear_columns(&x_after_const, &col_is_dummy, None)?;
    if x_use.ncols() == 0 {
        return Err(
            "Panel FE (Time): no regressors left after within transform and collinearity drop (all absorbed or redundant)"
                .to_string(),
        );
    }
    let omitted_indices: Option<Vec<usize>> = if omitted_x.is_empty() {
        None
    } else {
        Some(omitted_x.iter().map(|&j| j + 1).collect())
    };
    let kept_slope: Vec<usize> = (0..k_after_const)
        .filter(|&j| !omitted_x.contains(&j))
        .collect();

    // Time FE: cluster by time (residuals correlated within time period)
    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: time_id.to_vec(),
                xtreg_fe_style: true,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: has_const,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: y_tilde,
        exog: x_use,
        config,
    };

    let result = ols.fit()?;

    // Cluster F: df_residual = n_times - 1 (M - 1 where M = number of time clusters)
    let use_cluster_df = cov_type == "cluster";
    let df_residual = if use_cluster_df {
        n_times.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };
    let df_model = result.df_model;
    let df_total = df_model + df_residual;

    use statrs::distribution::{ContinuousCDF, FisherSnedecor, StudentsT};
    let (fvalue, f_p_value) = if use_cluster_df {
        let beta_s = &result.betas;
        let v_s = &result.cov_beta;
        let wald = if df_model == 0 {
            0.0
        } else {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            let x = v_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "Panel FE (Time): cluster cov_beta not pd for Wald F".to_string())?
                .solve(beta_faer.as_ref());
            let x_nd = x.as_ref().into_ndarray();
            beta_s.dot(&x_nd)
        };
        let f = if df_model > 0 { (wald / df_model as f64).max(0.0) } else { 0.0 };
        let df1 = (df_model as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel FE FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (result.fvalue, result.f_p_value)
    };

    let y_mean = endog.iter().sum::<f64>() / n as f64;
    let k_vars = result.betas.len();
    let x_mean: Array1<f64> = (0..k_vars)
        .map(|c| exog.column(kept_slope[c] + 1).iter().sum::<f64>() / n as f64)
        .collect();
    let const_coef = y_mean - result.betas.dot(&x_mean);
    let var_const = x_mean.dot(&result.cov_beta).dot(&x_mean)
        + result.ms_residual / n as f64;
    let const_std_err = var_const.max(0.0).sqrt();

    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel FE StudentsT: {}", e))?;
    let const_t = const_coef / const_std_err;
    let const_p = 2.0 * (1.0 - t_dist.cdf(const_t.abs()));
    let t_crit = t_dist.inverse_cdf(0.975);
    let const_ci_l = const_coef - t_crit * const_std_err;
    let const_ci_u = const_coef + t_crit * const_std_err;

    let mut betas = ndarray::Array1::zeros(k_vars + 1);
    betas[0] = const_coef;
    for i in 0..k_vars {
        betas[i + 1] = result.betas[i];
    }
    let mut stds = ndarray::Array1::zeros(k_vars + 1);
    stds[0] = const_std_err;
    for i in 0..k_vars {
        stds[i + 1] = result.stds[i];
    }
    let mut tvalues = ndarray::Array1::zeros(k_vars + 1);
    tvalues[0] = const_t;
    for i in 0..k_vars {
        tvalues[i + 1] = result.tvalues[i];
    }
    let mut pvalues = ndarray::Array1::zeros(k_vars + 1);
    pvalues[0] = const_p;
    let mut conf_int_left = ndarray::Array1::zeros(k_vars + 1);
    conf_int_left[0] = const_ci_l;
    let mut conf_int_right = ndarray::Array1::zeros(k_vars + 1);
    conf_int_right[0] = const_ci_u;
    for i in 0..k_vars {
        pvalues[i + 1] = if use_cluster_df {
            2.0 * (1.0 - t_dist.cdf(result.tvalues[i].abs()))
        } else {
            result.pvalues[i]
        };
        conf_int_left[i + 1] = result.betas[i] - t_crit * result.stds[i];
        conf_int_right[i + 1] = result.betas[i] + t_crit * result.stds[i];
    }

    let mut cov_beta = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    cov_beta[[0, 0]] = var_const;
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_beta[[i + 1, j + 1]] = result.cov_beta[[i, j]];
        }
        let cov_const_beta_i = -x_mean.dot(&result.cov_beta.column(i));
        cov_beta[[0, i + 1]] = cov_const_beta_i;
        cov_beta[[i + 1, 0]] = cov_const_beta_i;
    }
    let mut cov_nr = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_nr[[i + 1, j + 1]] = result.cov_beta_nonrobust[[i, j]];
        }
    }

    let kept_cols: Vec<usize> = std::iter::once(0)
        .chain(kept_slope.iter().map(|&j| j + 1))
        .collect();
    let exog_kept = exog.select(ndarray::Axis(1), &kept_cols);
    let fe_stats = compute_fe_stats_time(
        endog,
        &exog_kept,
        time_id,
        n_times,
        &result.betas,
        const_coef,
        result.r2,
        result.ss_residual,
    )?;

    Ok(super::PanelOLSResult {
        const_coef: Some(const_coef),
        const_std_err: Some(const_std_err),
        fe_stats: Some(fe_stats),
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total: result.ss_total / df_total as f64,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas,
        stds,
        tvalues,
        pvalues,
        conf_int_left,
        conf_int_right,
        cov_beta,
        cov_beta_nonrobust: Some(cov_nr),
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

/// Panel Two-Way Fixed Effects: z̃_it = z_it - z̄_i - z̄_t + z̄
pub fn fit_panel_fe_twoway(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n || entity_id.len() != n || time_id.len() != n {
        return Err("Panel FE (Two-Way): lengths must match".to_string());
    }
    let n_times = time_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 || n_times < 2 {
        return Err("Panel FE (Two-Way): need at least 2 entities and 2 time periods".to_string());
    }

    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_tilde = within_transform_twoway(&y_vec, entity_id, time_id)?;

    let k = exog.ncols();
    let mut x_tilde = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let transformed = within_transform_twoway(&col, entity_id, time_id)?;
        for (i, &v) in transformed.iter().enumerate() {
            x_tilde[[i, c]] = v;
        }
    }

    let (x_after_const, has_const) = if constant && k > 0 {
        let first_col = x_tilde.column(0);
        let is_const = first_col.iter().all(|&v| v.abs() < 1e-10);
        if is_const {
            (x_tilde.slice(ndarray::s![.., 1..]).to_owned(), false)
        } else {
            (x_tilde, constant)
        }
    } else {
        (x_tilde, constant)
    };

    // Drop collinear columns (e.g. time-invariant or entity-invariant in two-way)
    let k_after_const = x_after_const.ncols();
    let col_is_dummy = vec![false; k_after_const];
    let (x_use, omitted_x) = drop_collinear_columns(&x_after_const, &col_is_dummy, None)?;
    if x_use.ncols() == 0 {
        return Err(
            "Panel FE (Two-Way): no regressors left after within transform and collinearity drop \
             (all absorbed by entity/time FE or redundant). Omit absorbed dummies—e.g. for DID, use Treat×Post only, not separate Treat/Post mains."
                .to_string(),
        );
    }
    let omitted_indices: Option<Vec<usize>> = if omitted_x.is_empty() {
        None
    } else {
        Some(omitted_x.iter().map(|&j| j + 1).collect())
    };
    let kept_slope: Vec<usize> = (0..k_after_const)
        .filter(|&j| !omitted_x.contains(&j))
        .collect();

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: entity_id.to_vec(),
                xtreg_fe_style: true,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: has_const,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: y_tilde,
        exog: x_use,
        config,
    };

    let result = ols.fit()?;

    let use_cluster_df = cov_type == "cluster";
    let df_residual = if use_cluster_df {
        n_entities.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };
    let df_model = result.df_model;
    let df_total = df_model + df_residual;

    use statrs::distribution::{ContinuousCDF, FisherSnedecor, StudentsT};
    let (fvalue, f_p_value) = if use_cluster_df {
        let beta_s = &result.betas;
        let v_s = &result.cov_beta;
        let wald = if df_model == 0 {
            0.0
        } else {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            let x = v_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "Panel FE (Two-Way): cluster cov_beta not pd for Wald F".to_string())?
                .solve(beta_faer.as_ref());
            let x_nd = x.as_ref().into_ndarray();
            beta_s.dot(&x_nd)
        };
        let f = if df_model > 0 { (wald / df_model as f64).max(0.0) } else { 0.0 };
        let df1 = (df_model as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel FE FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (result.fvalue, result.f_p_value)
    };

    let y_mean = endog.iter().sum::<f64>() / n as f64;
    let k_vars = result.betas.len();
    let x_mean: Array1<f64> = (0..k_vars)
        .map(|c| exog.column(kept_slope[c] + 1).iter().sum::<f64>() / n as f64)
        .collect();
    let const_coef = y_mean - result.betas.dot(&x_mean);
    let var_const = x_mean.dot(&result.cov_beta).dot(&x_mean)
        + result.ms_residual / n as f64;
    let const_std_err = var_const.max(0.0).sqrt();

    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel FE StudentsT: {}", e))?;
    let const_t = const_coef / const_std_err;
    let const_p = 2.0 * (1.0 - t_dist.cdf(const_t.abs()));
    let t_crit = t_dist.inverse_cdf(0.975);
    let const_ci_l = const_coef - t_crit * const_std_err;
    let const_ci_u = const_coef + t_crit * const_std_err;

    let mut betas = ndarray::Array1::zeros(k_vars + 1);
    betas[0] = const_coef;
    for i in 0..k_vars {
        betas[i + 1] = result.betas[i];
    }
    let mut stds = ndarray::Array1::zeros(k_vars + 1);
    stds[0] = const_std_err;
    for i in 0..k_vars {
        stds[i + 1] = result.stds[i];
    }
    let mut tvalues = ndarray::Array1::zeros(k_vars + 1);
    tvalues[0] = const_t;
    for i in 0..k_vars {
        tvalues[i + 1] = result.tvalues[i];
    }
    let mut pvalues = ndarray::Array1::zeros(k_vars + 1);
    pvalues[0] = const_p;
    let mut conf_int_left = ndarray::Array1::zeros(k_vars + 1);
    conf_int_left[0] = const_ci_l;
    let mut conf_int_right = ndarray::Array1::zeros(k_vars + 1);
    conf_int_right[0] = const_ci_u;
    for i in 0..k_vars {
        pvalues[i + 1] = if use_cluster_df {
            2.0 * (1.0 - t_dist.cdf(result.tvalues[i].abs()))
        } else {
            result.pvalues[i]
        };
        conf_int_left[i + 1] = result.betas[i] - t_crit * result.stds[i];
        conf_int_right[i + 1] = result.betas[i] + t_crit * result.stds[i];
    }

    let mut cov_beta = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    cov_beta[[0, 0]] = var_const;
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_beta[[i + 1, j + 1]] = result.cov_beta[[i, j]];
        }
        let cov_const_beta_i = -x_mean.dot(&result.cov_beta.column(i));
        cov_beta[[0, i + 1]] = cov_const_beta_i;
        cov_beta[[i + 1, 0]] = cov_const_beta_i;
    }
    let mut cov_nr = ndarray::Array2::zeros((k_vars + 1, k_vars + 1));
    for i in 0..k_vars {
        for j in 0..k_vars {
            cov_nr[[i + 1, j + 1]] = result.cov_beta_nonrobust[[i, j]];
        }
    }

    // Same Stata-style entity-level block as one-way FE (R² Between/Overall, σ_u, σ_e, ρ, corr); TWFE uses two-way within R².
    let kept_cols: Vec<usize> = std::iter::once(0)
        .chain(kept_slope.iter().map(|&j| j + 1))
        .collect();
    let exog_kept = exog.select(ndarray::Axis(1), &kept_cols);
    let fe_stats = compute_fe_stats(
        endog,
        &exog_kept,
        entity_id,
        n_entities,
        &result.betas,
        const_coef,
        result.r2,
        result.ss_residual,
    )?;

    Ok(super::PanelOLSResult {
        const_coef: Some(const_coef),
        const_std_err: Some(const_std_err),
        fe_stats: Some(fe_stats),
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total: result.ss_total / df_total as f64,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas,
        stds,
        tvalues,
        pvalues,
        conf_int_left,
        conf_int_right,
        cov_beta,
        cov_beta_nonrobust: Some(cov_nr),
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
