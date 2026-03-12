//! Panel Random Effects (GLS with variance components)
//!
//! Quasi-demeaning: y*_it = y_it - θ·ȳ_i, where θ depends on variance components.
//! Swamy-Arora (1972) variance component estimation.

use crate::regression::linear_model::OLS;
use ndarray::{Array1, Array2};
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

/// Panel Random Effects estimator (Swamy-Arora)
pub fn fit_panel_re(
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
            "Panel RE: exog rows {} != endog len {}",
            exog.nrows(),
            n
        ));
    }
    if entity_id.len() != n {
        return Err(format!(
            "Panel RE: entity_id len {} != n {}",
            entity_id.len(),
            n
        ));
    }

    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_entities < 2 {
        return Err("Panel RE: need at least 2 entities".to_string());
    }

    // Compute T_i (obs per entity)
    let mut t_per_entity: HashMap<usize, usize> = HashMap::new();
    for &eid in entity_id {
        *t_per_entity.entry(eid).or_insert(0) += 1;
    }
    let t_bar = n as f64 / n_entities as f64;

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

    let ols_w = OLS {
        endog: y_w.clone(),
        exog: x_w.clone(),
        config: crate::regression::linear_model::OLSConfig {
            constant: false,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let res_w = ols_w.fit().map_err(|e| format!("Panel RE within step: {}", e))?;
    let sigma2_e = res_w.ss_residual / res_w.df_residual as f64;

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

    let ols_b = OLS {
        endog: y_b,
        exog: x_b,
        config: crate::regression::linear_model::OLSConfig {
            constant,
            cov_type: "nonrobust".to_string(),
            cov_params: None,
        },
    };
    let res_b = ols_b.fit().map_err(|e| format!("Panel RE between step: {}", e))?;
    let df_b = res_b.df_residual;
    let sigma2_u = if df_b > 0 {
        (res_b.ss_residual / df_b as f64 - sigma2_e / t_bar).max(0.0)
    } else {
        0.0
    };

    if sigma2_u <= 0.0 {
        return Err("Panel RE: variance component sigma_u^2 <= 0 (try FE instead)".to_string());
    }

    // Theta for quasi-demeaning: θ = 1 - sigma_e / sqrt(sigma_e^2 + T_i * sigma_u^2)
    // For unbalanced panel, use average T
    let theta = 1.0 - (sigma2_e / (sigma2_e + t_bar * sigma2_u)).sqrt();

    // Quasi-demean: y*_it = y_it - θ * ȳ_i
    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let y_bar = between_transform(&y_vec, entity_id);

    let y_star: Array1<f64> = Array1::from_shape_fn(n, |i| y_vec[i] - theta * y_bar[i]);

    let mut x_star = Array2::zeros((n, k));
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let x_bar = between_transform(&col, entity_id);
        for i in 0..n {
            x_star[[i, c]] = col[i] - theta * x_bar[i];
        }
    }

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
        exog: x_star,
        config,
    };

    let result = ols_re.fit()?;

    Ok(super::PanelOLSResult {
        const_coef: None,
        const_std_err: None,
        fe_stats: None,
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
        pvalues: result.pvalues,
        conf_int_left: result.conf_int_left,
        conf_int_right: result.conf_int_right,
        cov_beta: result.cov_beta,
        cond_no: result.cond_no,
    })
}
