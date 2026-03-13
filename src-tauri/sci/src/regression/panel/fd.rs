//! Panel First Difference estimator
//!
//! Δy_it = β Δx_it + Δu_it
//! 与 Stata D. 算子一致：仅在原始数据中相邻时间点之间差分（delta=1），不跨 gap。

use crate::regression::covariance::CovParams;
use crate::regression::linear_model::{OLSConfig, OLS};
use ndarray::{Array1, Array2};

/// Panel First Difference estimator
///
/// 与 Stata `reg D.y D.x, nocons` 一致：仅当 time_values[i+1] - time_values[i] == 1 时差分，
/// 即仅在相邻时间点（delta=1）之间差分，不跨 gap。
pub fn fit_panel_fd(
    endog: &Array1<f64>,
    exog: &Array2<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    time_values: &[i64],
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
    if entity_id.len() != n || time_id.len() != n || time_values.len() != n {
        return Err("Panel FD: entity_id, time_id, time_values must match data length".to_string());
    }

    let k = exog.ncols();
    let mut diff_entity = Vec::new();
    let mut dy = Vec::new();
    let mut dx_cols: Vec<Vec<f64>> = (0..k).map(|_| Vec::new()).collect();

    // 仅当 entity 相同且 time_values[i+1] - time_values[i] == 1 时差分（Stata delta=1）
    let mut i = 0;
    while i + 1 < n {
        if entity_id[i] == entity_id[i + 1] && time_values[i + 1] - time_values[i] == 1 {
            diff_entity.push(entity_id[i]);
            dy.push(endog[i + 1] - endog[i]);
            for c in 0..k {
                dx_cols[c].push(exog[[i + 1, c]] - exog[[i, c]]);
            }
        }
        i += 1;
    }

    let n_fd = diff_entity.len();

    if n_fd == 0 {
        return Err("Panel FD: no valid first-differenced observations. Ensure (entity, time) has consecutive periods (delta=1).".to_string());
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

    // 若存在常数列，差分后全为 0，需剔除
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
