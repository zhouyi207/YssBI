//! Panel LSDV (Least Squares Dummy Variables) estimator
//!
//! y_it = α + x_it'β + Σ_i D_i·α_i + ε_it
//! Regress y on [1, X, entity dummies] with one entity as reference.
//! Gives same slope coefficients β as within estimator.

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
        constant: true,
        cov_type: cov_type.to_string(),
        cov_params,
    };

    let ols = OLS {
        endog: endog.clone(),
        exog: x_lsdv,
        config,
    };

    let result = ols.fit()?;

    let k_slope = if constant && k > 0 { k - 1 } else { k };
    let n_report = 1 + k_slope;
    let n_full = result.betas.len();
    let df_model_slope = k_slope;
    let df_residual = if cov_type == "cluster" {
        n_entities.saturating_sub(1).max(1)
    } else {
        result.df_residual
    };

    let use_cluster_df = cov_type == "cluster";
    use statrs::distribution::{ContinuousCDF, StudentsT};
    let t_dist = StudentsT::new(0.0, 1.0, df_residual as f64).unwrap();
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
            let x = v_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "Panel LSDV: cov not pd for Wald F".to_string())?
                .solve(beta_faer.as_ref());
            beta_s.dot(&x.as_ref().into_ndarray())
        } else {
            0.0
        };
        let f = wald / df_model_slope as f64;
        let dist = FisherSnedecor::new(df_model_slope as f64, df_residual as f64).unwrap();
        (f, 1.0 - dist.cdf(f))
    } else {
        (0.0, 1.0)
    };

    let ms_total = result.ss_total / df_total as f64;

    Ok(super::PanelOLSResult {
        const_coef: Some(result.betas[0]),
        const_std_err: Some(result.stds[0]),
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
        cond_no: result.cond_no,
    })
}
