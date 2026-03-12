//! Panel First Difference estimator
//!
//! Δy_it = β Δx_it + Δu_it
//! Requires data sorted by (entity, time).

use crate::regression::covariance::CovParams;
use crate::regression::linear_model::{OLSConfig, OLS};
use ndarray::{Array1, Array2};

/// First difference within each entity. Returns (diff_values, diff_entity_ids).
/// Input must be sorted by (entity_id, time_id).
fn first_diff_within_entity(
    v: &[f64],
    entity_id: &[usize],
    time_id: &[usize],
) -> Result<(Vec<f64>, Vec<usize>), String> {
    let n = v.len();
    if entity_id.len() != n || time_id.len() != n {
        return Err(format!(
            "first_diff: len mismatch entity={} time={} data={}",
            entity_id.len(),
            time_id.len(),
            n
        ));
    }

    let mut rows: Vec<(usize, usize, f64)> = (0..n)
        .map(|i| (entity_id[i], time_id[i], v[i]))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut diff_vals = Vec::new();
    let mut diff_entity = Vec::new();

    let mut i = 0;
    while i < rows.len() {
        let (eid, _, val_i) = rows[i];
        let mut j = i + 1;
        while j < rows.len() && rows[j].0 == eid {
            let val_j = rows[j].2;
            let d = val_j - val_i;
            if !d.is_nan() {
                diff_vals.push(d);
                diff_entity.push(eid);
            }
            i = j;
            j += 1;
        }
        i += 1;
    }

    if diff_vals.is_empty() {
        return Err(
            "Panel FD: no valid first-differenced observations. Ensure (entity, time) has consecutive periods."
                .to_string(),
        );
    }
    Ok((diff_vals, diff_entity))
}

/// Panel First Difference estimator
pub fn fit_panel_fd(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelOLSResult, String> {
    let n = endog.len();
    if exog.nrows() != n {
        return Err(format!(
            "Panel FD: exog rows {} != endog len {}",
            exog.nrows(),
            n
        ));
    }
    if entity_id.len() != n || time_id.len() != n {
        return Err("Panel FD: entity_id and time_id must match data length".to_string());
    }

    let y_vec: Vec<f64> = endog.iter().cloned().collect();
    let (dy, diff_entity) = first_diff_within_entity(&y_vec, entity_id, time_id)?;

    let n_fd = dy.len();
    let k = exog.ncols();

    let mut dx_cols: Vec<Vec<f64>> = Vec::with_capacity(k);
    for c in 0..k {
        let col: Vec<f64> = exog.column(c).iter().cloned().collect();
        let (dc, _) = first_diff_within_entity(&col, entity_id, time_id)?;
        if dc.len() != n_fd {
            return Err(format!(
                "Panel FD: column {} produced {} diffs, expected {}",
                c,
                dc.len(),
                n_fd
            ));
        }
        dx_cols.push(dc);
    }

    let dy_arr = Array1::from_vec(dy);

    let mut dx_data = Vec::with_capacity(n_fd * k);
    for i in 0..n_fd {
        for c in 0..k {
            dx_data.push(dx_cols[c][i]);
        }
    }
    let dx_arr = Array2::from_shape_vec((n_fd, k), dx_data)
        .map_err(|e| format!("Panel FD: shape error {:?}", e))?;

    // Drop constant column if present (diff of constant = 0)
    let (dx_use, has_const) = if constant && k > 0 {
        let first_col = dx_arr.column(0);
        let all_zero = first_col.iter().all(|&v| v.abs() < 1e-12);
        if all_zero {
            (dx_arr.slice(ndarray::s![.., 1..]).to_owned(), false)
        } else {
            (dx_arr, constant)
        }
    } else {
        (dx_arr, constant)
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: diff_entity,
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
        endog: dy_arr,
        exog: dx_use,
        config,
    };

    let result = ols.fit()?;

    let n_entities = entity_id
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len();

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
        r2_within: Some(result.r2),
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
