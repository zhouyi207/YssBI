//! Panel Fixed Effects (Within transformation)
//!
//! Demeans variables within each entity, then runs OLS.
//! Standard errors: cluster by entity (default).
//! Stata xtreg, fe style: R2 Within/Between/Overall, sigma_u, sigma_e, rho, corr(u_i,Xb).

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
        r2_within,
        r2_between,
        r2_overall,
        obs_per_group_min: obs_min,
        obs_per_group_avg: obs_avg,
        obs_per_group_max: obs_max,
        sigma_u,
        sigma_e,
        rho,
        corr_u_i_xb,
    })
}

/// Within transformation: subtract entity-specific mean from each variable
fn within_transform(v: &[f64], entity_id: &[usize]) -> Result<Array1<f64>, String> {
    let n = v.len();
    if entity_id.len() != n {
        return Err(format!(
            "within_transform: entity_id len {} != data len {}",
            entity_id.len(),
            n
        ));
    }

    // Compute entity means
    let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
    for (i, &eid) in entity_id.iter().enumerate() {
        let val = v[i];
        if !val.is_nan() {
            let entry = sums.entry(eid).or_insert((0.0, 0));
            entry.0 += val;
            entry.1 += 1;
        }
    }

    let mut out = Vec::with_capacity(n);
    for (i, &eid) in entity_id.iter().enumerate() {
        let (s, cnt) = sums.get(&eid).copied().unwrap_or((0.0, 0));
        let mean = if cnt > 0 { s / cnt as f64 } else { 0.0 };
        out.push(v[i] - mean);
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
    let (x_use, has_const) = if constant && k > 0 {
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
        let f = if df_model > 0 { wald / df_model as f64 } else { 0.0 };
        let dist = FisherSnedecor::new(df_model as f64, df_residual as f64).unwrap();
        (f, 1.0 - dist.cdf(f))
    } else {
        (result.fvalue, result.f_p_value)
    };

    // Recovered constant _cons = ȳ - β'x̄ (Stata xtreg, fe style)
    let y_mean = endog.iter().sum::<f64>() / n as f64;
    let k_vars = result.betas.len();
    let x_mean: Array1<f64> = (0..k_vars)
        .map(|c| exog.column(c + 1).iter().sum::<f64>() / n as f64)
        .collect();
    let const_coef = y_mean - result.betas.dot(&x_mean);
    let var_const = x_mean.dot(&result.cov_beta).dot(&x_mean)
        + result.ms_residual / n as f64;
    let const_std_err = var_const.max(0.0).sqrt();

    let t_dist = StudentsT::new(0.0, 1.0, df_residual as f64).unwrap();
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

    // Stata xtreg, fe style: R2 Between/Overall, obs per group, sigma_u, sigma_e, rho, corr(u_i,Xb)
    let fe_stats = compute_fe_stats(
        endog,
        exog,
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
        cond_no: result.cond_no,
    })
}
