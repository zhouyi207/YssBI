use super::critical_values::{
    stock_yogo_cv_1_endog, stock_yogo_cv_2_endog, stock_yogo_cv_liml_1_endog,
    stock_yogo_cv_liml_2_endog,
};
use super::{
    design::{PreparedIvDesign, prepare_design, project_endogenous},
    estimate::{coefficient_inference, goodness_of_fit, model_test},
};
use crate::error::{computation_failed, invalid_input};
use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::causal::iv::{
    FirstStageResult, FirstStageSummary, InstrumentalVariableDesign, InstrumentalVariableKind,
    InstrumentalVariableModelTest,
};
use yss_sci_contract::regression::OlsOptions;
use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};

use yss_sci_linalg::{Col, ColRef, Mat, MatRef};
use yss_sci_linalg::{MatrixExt, Solve};

/// Analyze instrument relevance without fitting or reconstructing a structural response.
/// `kind` selects the estimator's Stock–Yogo critical table; equations use OLS inference.
pub fn analyze(
    data: &InstrumentalVariableDesign,
    options: &OlsOptions,
    kind: InstrumentalVariableKind,
) -> Result<(Vec<FirstStageResult>, FirstStageSummary), SciError> {
    let op = SciOperationCode::InstrumentalVariables;
    let n = data.endogenous.first().map_or(0, Vec::len);
    if n == 0
        || data.instruments.len() < data.endogenous.len()
        || data
            .exogenous
            .iter()
            .chain(&data.endogenous)
            .chain(&data.instruments)
            .any(|column| column.len() != n)
    {
        return Err(invalid_input(op, ScientificInputViolation::ShapeMismatch));
    }
    if data
        .exogenous
        .iter()
        .chain(&data.endogenous)
        .chain(&data.instruments)
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(invalid_input(op, ScientificInputViolation::NonFiniteInput));
    }
    compute_first_stage(data, options, kind).map_err(|_| computation_failed(op))
}

fn compute_first_stage(
    data: &InstrumentalVariableDesign,
    options: &OlsOptions,
    kind: InstrumentalVariableKind,
) -> Result<(Vec<FirstStageResult>, FirstStageSummary), String> {
    let n = data.endogenous[0].len();
    let k_z = data.exogenous.len() + data.instruments.len() + usize::from(options.constant);
    let df_z = n
        .checked_sub(k_z)
        .filter(|&degrees| degrees > 0)
        .ok_or("IV firststage: insufficient residual degrees of freedom")?;
    let design = prepare_design(
        &borrowed_columns(&data.exogenous),
        &borrowed_columns(&data.endogenous),
        &borrowed_columns(&data.instruments),
        options.constant,
    )?;
    let residuals = Mat::from_fn(n, data.endogenous.len(), |row, column| {
        data.endogenous[column][row] - design.endog_hat[(row, column)]
    });
    let mut equations = Vec::with_capacity(data.endogenous.len());
    for (j, column) in data.endogenous.iter().enumerate() {
        let gamma = design.first_stage_coefficients.col(j);
        let resid = residuals.col(j);
        let (r2, r2_adjusted) =
            goodness_of_fit(ColRef::from_slice(column), resid, options.constant, df_z)?;
        let covariance = compute_cov_beta(
            &design.z,
            &design.ztz_inverse,
            resid,
            df_z,
            options.constant.then_some(0),
            &options.covariance,
        )?;
        let inference = coefficient_inference(gamma, &covariance, df_z, true)?;
        equations.push(FirstStageResult {
            endog_name: format!("endog_{}", j + 1),
            var_names: (0..k_z).map(|i| format!("z{}", i + 1)).collect(),
            betas: gamma.iter().copied().collect(),
            inference,
            df_residual: df_z,
            r2,
            r2_adjusted,
        });
    }
    let summary =
        compute_first_stage_summary(data, options, kind, &design, residuals.as_ref(), &equations)?;
    Ok((equations, summary))
}

fn borrowed_columns(columns: &[Vec<f64>]) -> Vec<ColRef<'_, f64>> {
    columns
        .iter()
        .map(|column| ColRef::from_slice(column))
        .collect()
}

fn compute_first_stage_summary(
    data: &InstrumentalVariableDesign,
    options: &OlsOptions,
    kind: InstrumentalVariableKind,
    design: &PreparedIvDesign,
    residuals: MatRef<'_, f64>,
    equations: &[FirstStageResult],
) -> Result<FirstStageSummary, String> {
    let z = &design.z;
    let endog_hat = &design.endog_hat;
    let has_constant = options.constant;
    let covariance = &options.covariance;
    let n = z.nrows();
    let k_z = z.ncols();
    let k_exog = data.exogenous.len();
    let k_iv = data.instruments.len();
    let k_endog = data.endogenous.len();
    let k1 = k_exog + usize::from(has_constant);
    let df_z = n - k_z;

    let x1 = z.subcols(0, k1);
    let x1tx1 = x1.transpose() * x1;
    let x1tx1_inverse = x1tx1
        .checked_cholesky()
        .map_err(|_| "IV2SLS firststage: X1'X1 not pd".to_string())?
        .solve(&Mat::identity(k1, k1));
    // Reuse the projection owner and turn fitted buffers into residualized columns.
    let endogenous_columns = borrowed_columns(&data.endogenous);
    let instrument_columns = borrowed_columns(&data.instruments);
    let (_, mut mx1_y) = project_endogenous(x1, &x1tx1_inverse, &endogenous_columns);
    let (_, mut mx1_x2) = project_endogenous(x1, &x1tx1_inverse, &instrument_columns);
    for row in 0..n {
        for column in 0..k_endog {
            mx1_y[(row, column)] = data.endogenous[column][row] - mx1_y[(row, column)];
        }
        for column in 0..k_iv {
            mx1_x2[(row, column)] = data.instruments[column][row] - mx1_x2[(row, column)];
        }
    }

    // Σ_VV = (1/(N-k_z)) Y' M_Z Y
    let sigma_vv = (residuals.transpose() * residuals) / yss_sci_linalg::Scale(df_z as f64);

    // Instrument-explained variation after removing included exogenous columns.
    let x2_mx1_x2 = mx1_x2.transpose() * mx1_x2.as_ref();
    let x2_mx1_x2_inv = x2_mx1_x2
        .checked_cholesky()
        .map_err(|_| "IV2SLS firststage: X2'M_X1 X2 not pd".to_string())?
        .solve(&Mat::identity(x2_mx1_x2.nrows(), x2_mx1_x2.nrows()));
    let cross = mx1_x2.transpose() * mx1_y.as_ref();
    let inner = cross.transpose() * x2_mx1_x2_inv.as_ref() * cross.as_ref();
    // The single-endogenous statistic equals the homoskedastic first-stage F.
    let inner_scaled = inner / yss_sci_linalg::Scale(k_iv as f64);

    // Generalized minimum eigenvalue of (inner/k_iv, Sigma_VV).
    let min_eigenvalue_from_cd = if k_endog == 1 {
        if !sigma_vv[(0, 0)].is_finite() || sigma_vv[(0, 0)] <= 0.0 {
            return Err("IV firststage: residual variance is undefined".into());
        }
        inner_scaled[(0, 0)] / sigma_vv[(0, 0)]
    } else {
        // Sigma^-1 * inner is generally nonsymmetric. Whiten the generalized
        // problem with Sigma = LL' before using the symmetric eigensolver.
        let lower = sigma_vv
            .checked_cholesky()
            .map_err(|_| "IV2SLS firststage: sigma_vv not pd".to_string())?
            .lower();
        let mut inverse_lower = Mat::identity(k_endog, k_endog);
        lower
            .as_ref()
            .solve_lower_triangular_in_place(inverse_lower.as_mut());
        let g_mat = inverse_lower.as_ref() * inner_scaled.as_ref() * inverse_lower.transpose();
        let g_matrix = g_mat.as_ref();
        let evd = yss_sci_linalg::SymmetricEigen::factor(g_matrix)
            .map_err(|_| "IV2SLS firststage: EVD failed".to_string())?;
        let s_col = evd.values();
        s_col.iter().fold(f64::INFINITY, |a, &b| a.min(b))
    };
    if !min_eigenvalue_from_cd.is_finite() {
        return Err("IV firststage: minimum eigenvalue is undefined".into());
    }

    let min_eigenvalue = min_eigenvalue_from_cd;
    let is_robust = covariance.is_robust();
    let min_eigenvalue_cv = if !is_robust {
        if kind == InstrumentalVariableKind::LimitedInformationMaximumLikelihood {
            if k_endog == 1 {
                stock_yogo_cv_liml_1_endog(k_iv)
            } else if k_endog == 2 {
                stock_yogo_cv_liml_2_endog(k_iv)
            } else {
                None
            }
        } else if k_endog == 1 {
            stock_yogo_cv_1_endog(k_iv)
        } else if k_endog == 2 {
            stock_yogo_cv_2_endog(k_iv)
        } else {
            None
        }
    } else {
        None
    };
    let min_eigenvalue_cv_note = if min_eigenvalue_cv.is_some() {
        None
    } else if is_robust {
        Some("robust".to_string())
    } else if k_endog >= 3 {
        Some("k_endog_gt_2".to_string())
    } else {
        Some("not_tabulated".to_string())
    };

    let (
        r2,
        r2_adj,
        partial_r2,
        f_stat,
        f_p_value,
        f_df1,
        f_df2,
        shea_partial_r2,
        shea_adj_partial_r2,
    ) = if k_endog == 1 {
        let equation = &equations[0];

        // Partial R2: regress M_X1*Y on M_X1*X2
        let my = mx1_y.col(0);
        let mx2t_my = mx1_x2.transpose() * my.as_ref();
        let xi = x2_mx1_x2_inv.as_ref() * mx2t_my.as_ref();
        let fitted = mx1_x2.as_ref() * xi.as_ref();
        let ss_resid_partial: f64 = my
            .iter()
            .zip(fitted.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        let ss_tot_partial: f64 = my.iter().map(|v| v.powi(2)).sum();
        if !ss_tot_partial.is_finite() || ss_tot_partial <= 0.0 {
            return Err("IV firststage: partial response variation is undefined".into());
        }
        let partial_r2 = 1.0 - ss_resid_partial / ss_tot_partial;
        if !partial_r2.is_finite() {
            return Err("IV firststage: partial R-squared is undefined".into());
        }

        // H0: excluded-instrument coefficients are zero. Reuse the equation's VCE.
        let gamma2 = Col::from_iter(equation.betas[k1..].iter().copied());
        let cov_gamma2 = Mat::from_fn(k_iv, k_iv, |r, c| {
            equation.inference.covariance[k1 + r][k1 + c]
        });
        let InstrumentalVariableModelTest::F {
            statistic: f_stat,
            p_value: f_p_value,
            df_numerator: f_df1,
            df_denominator: f_df2,
        } = model_test(&gamma2, &cov_gamma2, false, equation.df_residual, true)?
        else {
            return Err("IV firststage: unexpected joint-test reference".into());
        };

        (
            Some(equation.r2),
            Some(equation.r2_adjusted),
            Some(partial_r2),
            Some(f_stat),
            Some(f_p_value),
            Some(f_df1),
            Some(f_df2),
            vec![],
            vec![],
        )
    } else {
        // Multi endog: Shea's partial R2 for each
        let mut shea_partial = Vec::with_capacity(k_endog);
        let mut shea_adj = Vec::with_capacity(k_endog);
        for j in 0..k_endog {
            let y1 = ColRef::from_slice(&data.endogenous[j]);
            let y1_hat = endog_hat.col(j);
            // This branch has multiple endogenous columns; preserve row order directly.
            let w = Mat::from_fn(n, k1 + k_endog - 1, |row, column| {
                if column < k1 {
                    x1[(row, column)]
                } else {
                    let other = column - k1;
                    data.endogenous[other + usize::from(other >= j)][row]
                }
            });
            let w_hat = Mat::from_fn(n, k1 + k_endog - 1, |row, column| {
                if column < k1 {
                    x1[(row, column)]
                } else {
                    let other = column - k1;
                    endog_hat[(row, other + usize::from(other >= j))]
                }
            });
            let wtw = w.transpose() * w.as_ref();
            let wtw_inverse = wtw
                .checked_cholesky()
                .map_err(|_| "IV2SLS firststage: W'W not pd".to_string())?
                .solve(&Mat::identity(wtw.nrows(), wtw.nrows()));
            let y1_tilde =
                y1 - (w.as_ref() * (wtw_inverse.as_ref() * (w.transpose() * y1).as_ref())).as_ref();
            let w_hat_t_w_hat = w_hat.transpose() * w_hat.as_ref();
            let w_hat_inverse = w_hat_t_w_hat
                .checked_cholesky()
                .map_err(|_| "IV2SLS firststage: W_hat'W_hat not pd".to_string())?
                .solve(&Mat::identity(w_hat_t_w_hat.nrows(), w_hat_t_w_hat.nrows()));
            let y1_hat_tilde = y1_hat
                - (w_hat.as_ref()
                    * (w_hat_inverse.as_ref() * (w_hat.transpose() * y1_hat).as_ref()))
                .as_ref();

            let ss_tot = y1_tilde.iter().map(|v| v.powi(2)).sum::<f64>();
            let ss_resid = y1_tilde
                .iter()
                .zip(y1_hat_tilde.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>();
            if !ss_tot.is_finite() || ss_tot <= 0.0 {
                return Err("IV firststage: Shea response variation is undefined".into());
            }
            let r2_s = 1.0 - ss_resid / ss_tot;
            let r2_s_adj = if has_constant {
                1.0 - (1.0 - r2_s) * (n - 1) as f64 / (n - k_z + 1) as f64
            } else {
                1.0 - (1.0 - r2_s) * (n - 1) as f64 / (n - k_z) as f64
            };
            if !r2_s.is_finite() || !r2_s_adj.is_finite() {
                return Err("IV firststage: Shea R-squared is undefined".into());
            }
            shea_partial.push(r2_s);
            shea_adj.push(r2_s_adj);
        }
        (
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            shea_partial,
            shea_adj,
        )
    };

    Ok(FirstStageSummary {
        k_included_instruments: k_exog,
        k_excluded_instruments: k_iv,
        k_endogenous_regressors: k_endog,
        r2,
        r2_adjusted: r2_adj,
        partial_r2,
        f_stat,
        f_p_value,
        f_df1,
        f_df2,
        shea_partial_r2,
        shea_adj_partial_r2,
        min_eigenvalue,
        min_eigenvalue_cv,
        min_eigenvalue_cv_note,
    })
}
