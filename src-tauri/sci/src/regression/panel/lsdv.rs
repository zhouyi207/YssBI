//! Panel LSDV (Least Squares Dummy Variables) estimator
//!
//! Entity LSDV: y_it = α + x_it'β + Σ_i D_i·γ_i + ε_it
//! Time LSDV:   y_it = α + x_it'β + Σ_t D_t·γ_t + ε_it
//! Regress y on [1, X, dummies] with one category as reference.
//! Gives same slope coefficients β as within estimator.

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::covariance::CovParams;
use crate::regression::linear_model::{OLSConfig, OLS};
use ndarray::{Array1, Array2};
use std::collections::HashMap;

/// Panel LSDV estimator (Stata areg style)
/// exog: [1, x1, x2, ...] with constant in column 0
pub fn fit_panel_lsdv(
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
            "Panel LSDV: exog rows {} != endog len {}",
            exog.nrows(),
            n
        ));
    }
    if entity_id.len() != n {
        return Err(format!(
            "Panel LSDV: entity_id len {} != n {}",
            entity_id.len(),
            n
        ));
    }

    let mut eids: Vec<usize> = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    eids.sort_unstable();
    let n_entities = eids.len();
    if n_entities < 2 {
        return Err("Panel LSDV: need at least 2 entities".to_string());
    }

    let eid_to_idx: HashMap<usize, usize> = eids.iter().enumerate().map(|(i, &e)| (e, i)).collect();

    let k = exog.ncols();
    let n_dummies = n_entities - 1;

    let mut x_data = Vec::with_capacity(n * (k + n_dummies));
    for i in 0..n {
        for c in 0..k {
            x_data.push(exog[[i, c]]);
        }
        let eid = entity_id[i];
        let idx = *eid_to_idx.get(&eid).unwrap_or(&0);
        for d in 0..n_dummies {
            x_data.push(if idx == d + 1 { 1.0 } else { 0.0 });
        }
    }

    let x_lsdv = Array2::from_shape_vec((n, k + n_dummies), x_data)
        .map_err(|e| format!("Panel LSDV: shape {:?}", e))?;

    // col_is_dummy: exog cols = false, entity dummies = true. Intercept at col 0.
    let col_is_dummy: Vec<bool> = (0..k + n_dummies).map(|j| j >= k).collect();
    let (x_reduced, omitted_indices) =
        drop_collinear_columns(&x_lsdv, &col_is_dummy, Some(0))?;
    let omitted_indices = if omitted_indices.is_empty() {
        None
    } else {
        Some(omitted_indices)
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: entity_id.to_vec(),
                xtreg_fe_style: false,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: true,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: endog.clone(),
        exog: x_reduced,
        config,
    };

    let result = ols.fit()?;

    let kept_indices: Vec<usize> = (0..k + n_dummies)
        .filter(|i| !omitted_indices.as_ref().map(|o| o.contains(i)).unwrap_or(false))
        .collect();
    let const_pos = kept_indices.iter().position(|&x| x == 0);
    let (const_coef, const_std_err) = match const_pos {
        Some(pos) => (Some(result.betas[pos]), Some(result.stds[pos])),
        None => (None, None),
    };

    let k_slope = if constant && k > 0 { k - 1 } else { k };
    let n_report = (1 + k_slope).min(result.betas.len());
    let n_full = result.betas.len();
    let df_model_slope = k_slope;
    let df_residual = if cov_type == "cluster" {
        n_entities.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };

    let use_cluster_df = cov_type == "cluster";
    use statrs::distribution::{ContinuousCDF, StudentsT};
    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel LSDV StudentsT: {}", e))?;
    let t_crit = t_dist.inverse_cdf(0.975);

    let (pvalues_full, conf_left_full, conf_right_full) = if use_cluster_df {
        let mut pv = Array1::zeros(n_full);
        let mut cl = Array1::zeros(n_full);
        let mut cr = Array1::zeros(n_full);
        for i in 0..n_full {
            let t = result.tvalues[i];
            pv[i] = 2.0 * (1.0 - t_dist.cdf(t.abs()));
            cl[i] = result.betas[i] - t_crit * result.stds[i];
            cr[i] = result.betas[i] + t_crit * result.stds[i];
        }
        (pv, cl, cr)
    } else {
        (
            result.pvalues.clone(),
            result.conf_int_left.clone(),
            result.conf_int_right.clone(),
        )
    };
    let cov_slope = result.cov_beta.slice(ndarray::s![..n_report, ..n_report]).to_owned();
    let df_total = df_model_slope + df_residual;

    let (fvalue, f_p_value) = if df_model_slope > 0 {
        use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
        use faer::{Side, linalg::solvers::Solve};
        use statrs::distribution::{ContinuousCDF, FisherSnedecor};

        let beta_s = result.betas.slice(ndarray::s![1..n_report]);
        let v_s = cov_slope.slice(ndarray::s![1..n_report, 1..n_report]);
        let wald = if beta_s.len() > 0 {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            match v_faer.as_ref().llt(Side::Lower) {
                Ok(llt) => {
                    let x = llt.solve(beta_faer.as_ref());
                    beta_s.dot(&x.as_ref().into_ndarray())
                }
                Err(_) => {
                    // Cov not PD (e.g. cluster-robust with few clusters); fallback
                    0.0
                }
            }
        } else {
            0.0
        };
        let f = (wald / df_model_slope as f64).max(0.0);
        let df1 = (df_model_slope as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel LSDV FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (0.0, 1.0)
    };

    let ms_total = result.ss_total / df_total as f64;

    Ok(super::PanelOLSResult {
        const_coef,
        const_std_err,
        fe_stats: None,
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: 0,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model: df_model_slope,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas: result.betas.clone(),
        stds: result.stds.clone(),
        tvalues: result.tvalues.clone(),
        pvalues: pvalues_full,
        conf_int_left: conf_left_full,
        conf_int_right: conf_right_full,
        cov_beta: result.cov_beta.clone(),
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

/// Panel LSDV with time dummies (Time Fixed Effects)
/// y_it = α + X_it'β + Σ_{t=2}^T γ_t D_t + u_it
pub fn fit_panel_lsdv_time(
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
        return Err("Panel LSDV (Time): lengths must match".to_string());
    }

    let mut tids: Vec<usize> = time_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    tids.sort_unstable();
    let n_times = tids.len();
    let n_entities = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().len();
    if n_times < 2 {
        return Err("Panel LSDV (Time): need at least 2 time periods".to_string());
    }

    let tid_to_idx: HashMap<usize, usize> = tids.iter().enumerate().map(|(i, &t)| (t, i)).collect();

    let k = exog.ncols();
    let n_dummies = n_times - 1;

    let mut x_data = Vec::with_capacity(n * (k + n_dummies));
    for i in 0..n {
        for c in 0..k {
            x_data.push(exog[[i, c]]);
        }
        let tid = time_id[i];
        let idx = *tid_to_idx.get(&tid).unwrap_or(&0);
        for d in 0..n_dummies {
            x_data.push(if idx == d + 1 { 1.0 } else { 0.0 });
        }
    }

    let x_lsdv = Array2::from_shape_vec((n, k + n_dummies), x_data)
        .map_err(|e| format!("Panel LSDV (Time): shape {:?}", e))?;

    // col_is_dummy: exog cols = false, time dummies = true. Intercept at col 0.
    let col_is_dummy: Vec<bool> = (0..k + n_dummies).map(|j| j >= k).collect();
    let (x_reduced, omitted_indices) =
        drop_collinear_columns(&x_lsdv, &col_is_dummy, Some(0))?;
    let omitted_indices = if omitted_indices.is_empty() {
        None
    } else {
        Some(omitted_indices)
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: time_id.to_vec(),
                xtreg_fe_style: false,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: true,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: endog.clone(),
        exog: x_reduced,
        config,
    };

    let result = ols.fit()?;

    let kept_indices: Vec<usize> = (0..k + n_dummies)
        .filter(|i| !omitted_indices.as_ref().map(|o| o.contains(i)).unwrap_or(false))
        .collect();
    let const_pos = kept_indices.iter().position(|&x| x == 0);
    let (const_coef, const_std_err) = match const_pos {
        Some(pos) => (Some(result.betas[pos]), Some(result.stds[pos])),
        None => (None, None),
    };

    let k_slope = if constant && k > 0 { k - 1 } else { k };
    let n_report = (1 + k_slope).min(result.betas.len());
    let n_full = result.betas.len();
    let df_model_slope = k_slope;
    let df_residual = if cov_type == "cluster" {
        n_times.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };

    let use_cluster_df = cov_type == "cluster";
    use statrs::distribution::{ContinuousCDF, StudentsT};
    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel LSDV StudentsT: {}", e))?;
    let t_crit = t_dist.inverse_cdf(0.975);

    let (pvalues_full, conf_left_full, conf_right_full) = if use_cluster_df {
        let mut pv = Array1::zeros(n_full);
        let mut cl = Array1::zeros(n_full);
        let mut cr = Array1::zeros(n_full);
        for i in 0..n_full {
            let t = result.tvalues[i];
            pv[i] = 2.0 * (1.0 - t_dist.cdf(t.abs()));
            cl[i] = result.betas[i] - t_crit * result.stds[i];
            cr[i] = result.betas[i] + t_crit * result.stds[i];
        }
        (pv, cl, cr)
    } else {
        (
            result.pvalues.clone(),
            result.conf_int_left.clone(),
            result.conf_int_right.clone(),
        )
    };
    let cov_slope = result.cov_beta.slice(ndarray::s![..n_report, ..n_report]).to_owned();
    let df_total = df_model_slope + df_residual;

    let (fvalue, f_p_value) = if df_model_slope > 0 {
        use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
        use faer::{Side, linalg::solvers::Solve};
        use statrs::distribution::{ContinuousCDF, FisherSnedecor};

        let beta_s = result.betas.slice(ndarray::s![1..n_report]);
        let v_s = cov_slope.slice(ndarray::s![1..n_report, 1..n_report]);
        let wald = if beta_s.len() > 0 {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            match v_faer.as_ref().llt(Side::Lower) {
                Ok(llt) => {
                    let x = llt.solve(beta_faer.as_ref());
                    beta_s.dot(&x.as_ref().into_ndarray())
                }
                Err(_) => {
                    // Cov not PD (e.g. cluster-robust with few clusters); fallback
                    0.0
                }
            }
        } else {
            0.0
        };
        let f = (wald / df_model_slope as f64).max(0.0);
        let df1 = (df_model_slope as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel LSDV FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (0.0, 1.0)
    };

    let ms_total = result.ss_total / df_total as f64;

    Ok(super::PanelOLSResult {
        const_coef,
        const_std_err,
        fe_stats: None,
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model: df_model_slope,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas: result.betas.clone(),
        stds: result.stds.clone(),
        tvalues: result.tvalues.clone(),
        pvalues: pvalues_full,
        conf_int_left: conf_left_full,
        conf_int_right: conf_right_full,
        cov_beta: result.cov_beta.clone(),
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

/// Panel LSDV with entity + time dummies (Two-Way Fixed Effects)
/// y_it = α + X_it'β + Σ_{i=2}^n γ_i D_i + Σ_{t=2}^T λ_t D_t + u_it
pub fn fit_panel_lsdv_twoway(
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
        return Err("Panel LSDV (Two-Way): lengths must match".to_string());
    }

    let mut eids: Vec<usize> = entity_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    eids.sort_unstable();
    let mut tids: Vec<usize> = time_id.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    tids.sort_unstable();
    let n_entities = eids.len();
    let n_times = tids.len();
    if n_entities < 2 || n_times < 2 {
        return Err("Panel LSDV (Two-Way): need at least 2 entities and 2 time periods".to_string());
    }

    let eid_to_idx: HashMap<usize, usize> = eids.iter().enumerate().map(|(i, &e)| (e, i)).collect();
    let tid_to_idx: HashMap<usize, usize> = tids.iter().enumerate().map(|(i, &t)| (t, i)).collect();

    let k = exog.ncols();
    let n_entity_dummies = n_entities - 1;
    let n_time_dummies = n_times - 1;
    let n_dummies = n_entity_dummies + n_time_dummies;

    let mut x_data = Vec::with_capacity(n * (k + n_dummies));
    for i in 0..n {
        for c in 0..k {
            x_data.push(exog[[i, c]]);
        }
        let eid = entity_id[i];
        let eidx = *eid_to_idx.get(&eid).unwrap_or(&0);
        for d in 0..n_entity_dummies {
            x_data.push(if eidx == d + 1 { 1.0 } else { 0.0 });
        }
        let tid = time_id[i];
        let tidx = *tid_to_idx.get(&tid).unwrap_or(&0);
        for d in 0..n_time_dummies {
            x_data.push(if tidx == d + 1 { 1.0 } else { 0.0 });
        }
    }

    let x_lsdv = Array2::from_shape_vec((n, k + n_dummies), x_data)
        .map_err(|e| format!("Panel LSDV (Two-Way): shape {:?}", e))?;

    let col_is_dummy: Vec<bool> = (0..k + n_dummies).map(|j| j >= k).collect();
    let (x_reduced, omitted_indices) =
        drop_collinear_columns(&x_lsdv, &col_is_dummy, Some(0))?;
    let omitted_indices = if omitted_indices.is_empty() {
        None
    } else {
        Some(omitted_indices)
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: entity_id.to_vec(),
                xtreg_fe_style: false,
            })
        } else {
            None
        }
    });
    let config = OLSConfig {
        constant: true,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: endog.clone(),
        exog: x_reduced,
        config,
    };

    let result = ols.fit()?;

    let kept_indices: Vec<usize> = (0..k + n_dummies)
        .filter(|i| !omitted_indices.as_ref().map(|o| o.contains(i)).unwrap_or(false))
        .collect();
    let const_pos = kept_indices.iter().position(|&x| x == 0);
    let (const_coef, const_std_err) = match const_pos {
        Some(pos) => (Some(result.betas[pos]), Some(result.stds[pos])),
        None => (None, None),
    };

    let k_slope = if constant && k > 0 { k - 1 } else { k };
    let n_report = (1 + k_slope).min(result.betas.len());
    let n_full = result.betas.len();
    let df_model_slope = k_slope;
    let df_residual = if cov_type == "cluster" {
        n_entities.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };

    let use_cluster_df = cov_type == "cluster";
    use statrs::distribution::{ContinuousCDF, StudentsT};
    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df)
        .map_err(|e| format!("Panel LSDV StudentsT: {}", e))?;
    let t_crit = t_dist.inverse_cdf(0.975);

    let (pvalues_full, conf_left_full, conf_right_full) = if use_cluster_df {
        let mut pv = Array1::zeros(n_full);
        let mut cl = Array1::zeros(n_full);
        let mut cr = Array1::zeros(n_full);
        for i in 0..n_full {
            let t = result.tvalues[i];
            pv[i] = 2.0 * (1.0 - t_dist.cdf(t.abs()));
            cl[i] = result.betas[i] - t_crit * result.stds[i];
            cr[i] = result.betas[i] + t_crit * result.stds[i];
        }
        (pv, cl, cr)
    } else {
        (
            result.pvalues.clone(),
            result.conf_int_left.clone(),
            result.conf_int_right.clone(),
        )
    };
    let cov_slope = result.cov_beta.slice(ndarray::s![..n_report, ..n_report]).to_owned();
    let df_total = df_model_slope + df_residual;

    let (fvalue, f_p_value) = if df_model_slope > 0 {
        use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
        use faer::{Side, linalg::solvers::Solve};
        use statrs::distribution::{ContinuousCDF, FisherSnedecor};

        let beta_s = result.betas.slice(ndarray::s![1..n_report]);
        let v_s = cov_slope.slice(ndarray::s![1..n_report, 1..n_report]);
        let wald = if beta_s.len() > 0 {
            let v_faer = v_s.view().into_faer().to_owned();
            let beta_faer = beta_s.view().into_faer_col().to_owned();
            match v_faer.as_ref().llt(Side::Lower) {
                Ok(llt) => {
                    let x = llt.solve(beta_faer.as_ref());
                    beta_s.dot(&x.as_ref().into_ndarray())
                }
                Err(_) => 0.0,
            }
        } else {
            0.0
        };
        let f = (wald / df_model_slope as f64).max(0.0);
        let df1 = (df_model_slope as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("Panel LSDV FisherSnedecor: {}", e))?;
        (f, 1.0 - dist.cdf(f))
    } else {
        (0.0, 1.0)
    };

    let ms_total = result.ss_total / df_total as f64;

    Ok(super::PanelOLSResult {
        const_coef,
        const_std_err,
        fe_stats: None,
        num_observation: result.num_observation,
        num_entities: n_entities,
        num_time_periods: n_times,
        ss_model: result.ss_model,
        ss_residual: result.ss_residual,
        ss_total: result.ss_total,
        df_model: df_model_slope,
        df_residual,
        df_total,
        ms_model: result.ms_model,
        ms_residual: result.ms_residual,
        ms_total,
        covariance_type: result.covariance_type,
        r2: result.r2,
        r2_adjusted: result.r2_adjusted,
        r2_within: Some(result.r2),
        fvalue,
        f_p_value,
        betas: result.betas.clone(),
        stds: result.stds.clone(),
        tvalues: result.tvalues.clone(),
        pvalues: pvalues_full,
        conf_int_left: conf_left_full,
        conf_int_right: conf_right_full,
        cov_beta: result.cov_beta.clone(),
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
