//! Panel LSDV (Least Squares Dummy Variables) estimator
//!
//! Entity LSDV: y_it = α + x_it'β + Σ_i D_i·γ_i + ε_it
//! Time LSDV:   y_it = α + x_it'β + Σ_t D_t·γ_t + ε_it
//! Regress y on [1, X, dummies] with one category as reference.
//! Gives same slope coefficients β as within estimator.

use crate::regression::collinearity::drop_collinear_columns;
use crate::regression::linear::OLS;
use std::collections::BTreeMap;
use yss_sci_contract::regression::CovParams;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

/// Panel LSDV estimator (Stata areg style)
/// exog: [1, x1, x2, ...] with constant in column 0
pub fn fit_panel_lsdv(
    endog: &Col<f64>,
    exog: &Mat<f64>,
    entity_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelFit, String> {
    fit_panel_lsdv_dummies(
        endog,
        exog,
        entity_id,
        Dummies::Entity,
        constant,
        cov_type,
        cov_params,
    )
}

/// Panel LSDV with time dummies (Time Fixed Effects)
/// y_it = α + X_it'β + Σ_{t=2}^T γ_t D_t + u_it
pub fn fit_panel_lsdv_time(
    endog: &Col<f64>,
    exog: &Mat<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelFit, String> {
    fit_panel_lsdv_dummies(
        endog,
        exog,
        entity_id,
        Dummies::Time(time_id),
        constant,
        cov_type,
        cov_params,
    )
}

/// Panel LSDV with entity + time dummies (Two-Way Fixed Effects)
/// y_it = α + X_it'β + Σ_{i=2}^n γ_i D_i + Σ_{t=2}^T λ_t D_t + u_it
pub fn fit_panel_lsdv_twoway(
    endog: &Col<f64>,
    exog: &Mat<f64>,
    entity_id: &[usize],
    time_id: &[usize],
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelFit, String> {
    fit_panel_lsdv_dummies(
        endog,
        exog,
        entity_id,
        Dummies::TwoWay(time_id),
        constant,
        cov_type,
        cov_params,
    )
}

/// Effects represented by dummy columns; each dimension drops its first level.
#[derive(Clone, Copy)]
enum Dummies<'a> {
    Entity,
    Time(&'a [usize]),
    TwoWay(&'a [usize]),
}

/// Positions of the sorted distinct ids.
fn levels(ids: &[usize]) -> BTreeMap<usize, usize> {
    let mut distinct: Vec<usize> = ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    distinct.sort_unstable();
    distinct
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect()
}

fn fit_panel_lsdv_dummies(
    endog: &Col<f64>,
    exog: &Mat<f64>,
    entity_id: &[usize],
    dummies: Dummies<'_>,
    constant: bool,
    cov_type: &str,
    cov_params: Option<CovParams>,
) -> Result<super::PanelFit, String> {
    let (label, family, dimensions, cluster_dimension) = match dummies {
        Dummies::Entity => ("Panel LSDV", "panel_lsdv", vec![entity_id], entity_id),
        Dummies::Time(time_id) => (
            "Panel LSDV (Time)",
            "panel_lsdv_time",
            vec![time_id],
            time_id,
        ),
        Dummies::TwoWay(time_id) => (
            "Panel LSDV (Two-Way)",
            "panel_lsdv_twoway",
            vec![entity_id, time_id],
            entity_id,
        ),
    };
    let n = endog.nrows();
    if exog.nrows() != n || entity_id.len() != n || dimensions.iter().any(|ids| ids.len() != n) {
        return Err(format!("{label}: lengths must match"));
    }
    let n_entities = levels(entity_id).len();
    let dimension_levels: Vec<BTreeMap<usize, usize>> =
        dimensions.iter().map(|ids| levels(ids)).collect();
    if dimension_levels.iter().any(|level| level.len() < 2) {
        return Err(format!(
            "{label}: every absorbed dimension needs at least 2 levels"
        ));
    }

    let k = exog.ncols();
    let n_dummies = dimension_levels
        .iter()
        .map(|level| level.len() - 1)
        .sum::<usize>();

    let mut x_data = Vec::with_capacity(n * (k + n_dummies));
    for i in 0..n {
        for c in 0..k {
            x_data.push(exog[(i, c)]);
        }
        for (ids, level) in dimensions.iter().zip(&dimension_levels) {
            let idx = *level.get(&ids[i]).unwrap_or(&0);
            for d in 0..level.len() - 1 {
                x_data.push(if idx == d + 1 { 1.0 } else { 0.0 });
            }
        }
    }

    let x_lsdv =
        yss_sci_linalg::MatRef::from_row_major_slice(&(x_data), n, k + n_dummies).to_owned();

    // Dummy columns follow the exog columns; the intercept stays at col 0.
    let col_is_dummy: Vec<bool> = (0..k + n_dummies).map(|j| j >= k).collect();
    let (x_reduced, omitted_indices) = drop_collinear_columns(&x_lsdv, &col_is_dummy, Some(0))?;
    let omitted_indices = if omitted_indices.is_empty() {
        None
    } else {
        Some(omitted_indices)
    };

    let cov_params = cov_params.or_else(|| {
        if cov_type == "cluster" {
            Some(CovParams::Cluster {
                cluster_id: cluster_dimension.to_vec(),
                xtreg_fe_style: false,
            })
        } else {
            None
        }
    });
    let config = yss_sci_contract::regression::OlsOptions::from_covariance_parts(
        true,
        cov_type,
        (cov_params).as_ref(),
    )
    .map_err(|error| error.to_string())?;

    let ols = OLS {
        endog: endog.clone(),
        exog: x_reduced,
        config,
    };

    let result = ols.fit()?;

    let kept_indices: Vec<usize> = (0..k + n_dummies)
        .filter(|i| {
            !omitted_indices
                .as_ref()
                .map(|o| o.contains(i))
                .unwrap_or(false)
        })
        .collect();
    let const_pos = kept_indices.iter().position(|&x| x == 0);
    let (const_coef, const_std_err) = match const_pos {
        Some(pos) => (Some(result.betas[pos]), Some(result.stds[pos])),
        None => (None, None),
    };

    let k_slope = if constant && k > 0 { k - 1 } else { k };
    let n_report = (1 + k_slope).min(result.betas.nrows());
    let n_full = result.betas.nrows();
    let df_model_slope = k_slope;
    let df_residual = if cov_type == "cluster" {
        levels(cluster_dimension).len().saturating_sub(1).max(1)
    } else {
        result.df_residual
    };

    let use_cluster_df = cov_type == "cluster";
    use statrs::distribution::StudentsT;
    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(0.0, 1.0, t_df).map_err(|e| format!("{label} StudentsT: {e}"))?;
    let t_crit = crate::inference::intervals::critical(0.95, Some(t_dist.freedom()))
        .map_err(|error| format!("{label} t critical value: {error:?}"))?;

    let (pvalues_full, conf_left_full, conf_right_full) = if use_cluster_df {
        let mut pv = Col::zeros(n_full);
        let mut cl = Col::zeros(n_full);
        let mut cr = Col::zeros(n_full);
        for i in 0..n_full {
            let t = result.tvalues[i];
            pv[i] = crate::distribution::student_t_probability(
                &t_dist,
                t,
                yss_sci_contract::hypothesis::Alternative::TwoSided,
            );
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
    let cov_slope = result
        .cov_beta
        .submatrix(0, 0, n_report, n_report)
        .to_owned();
    let df_total = df_model_slope + df_residual;

    let (fvalue, f_p_value) = if df_model_slope > 0 {
        use statrs::distribution::FisherSnedecor;

        let beta_s = result.betas.subrows(1, n_report - 1);
        let v_s = cov_slope.submatrix(1, 1, n_report - 1, n_report - 1);
        let wald = if beta_s.nrows() > 0 {
            let v_matrix = v_s.as_ref().to_owned();
            let beta_vector = beta_s.as_ref().to_owned();
            match v_matrix.as_ref().checked_cholesky() {
                Ok(llt) => {
                    let x = llt.solve(&beta_vector.as_ref());
                    beta_s.transpose() * x.as_ref()
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
        let dist =
            FisherSnedecor::new(df1, df2).map_err(|e| format!("{label} FisherSnedecor: {e}"))?;
        (f, crate::distribution::fisher_snedecor_sf(&dist, f))
    } else {
        (0.0, 1.0)
    };

    let ms_total = result.ss_total / df_total as f64;

    Ok(super::PanelFit {
        parameter_names: vec![],
        parameter_categories: vec![],
        response_name: "response".into(),
        omitted_terms: vec![],
        estimation: yss_sci_contract::panel::PanelEstimationSample {
            space: "original".into(),
            constant: ols.config.constant,
            response: ols.endog.iter().copied().collect(),
            design: (0..ols.exog.ncols())
                .map(|j| ols.exog.col(j).iter().copied().collect())
                .collect(),
            coefficients: result.betas.iter().copied().collect(),
            fitted: result.fitted.iter().copied().collect(),
            residuals: result.residuals.iter().copied().collect(),
            source_rows: vec![],
        },
        family: family.into(),
        coefficients: (result.betas.clone()).iter().copied().collect(),
        inference: super::RegressionCoefficientStatistics {
            covariance: super::covariance_rows(&(result.cov_beta.clone())),
            standard_errors: (result.stds.clone()).iter().copied().collect(),
            statistic_values: (result.tvalues.clone()).iter().copied().collect(),
            p_values: (pvalues_full).iter().copied().collect(),
            confidence_interval_lower: (conf_left_full).iter().copied().collect(),
            confidence_interval_upper: (conf_right_full).iter().copied().collect(),
        },
        statistics: super::PanelModelStatistics {
            observations: result.num_observation,
            entities: n_entities,
            time_periods: match dummies {
                Dummies::Entity => 0,
                Dummies::Time(time_id) | Dummies::TwoWay(time_id) => levels(time_id).len(),
            },
            within_r2: Some(result.r2),
            linear: super::LinearRegressionStatistics {
                r2: result.r2,
                adjusted_r2: result.r2_adjusted,
                f_statistic: fvalue,
                f_p_value,
                df_model: df_model_slope,
                df_residual,
                df_total,
                ss_model: result.ss_model,
                ss_residual: result.ss_residual,
                ss_total: result.ss_total,
                ms_model: result.ms_model,
                ms_residual: result.ms_residual,
                ms_total,
                covariance_type: result.covariance_type,
                condition_number: result.cond_no,
            },
        },
        recovered_constant: const_coef,
        recovered_constant_standard_error: const_std_err,
        effects_statistics: None,
        omitted_indices,
        estimator_statistics: super::PanelEstimatorStatistics::LeastSquares,
        covariance_nonrobust: None,
    })
}
