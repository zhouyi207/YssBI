use super::critical_values::{
    stock_yogo_cv_1_endog, stock_yogo_cv_2_endog, stock_yogo_cv_liml_1_endog,
    stock_yogo_cv_liml_2_endog,
};
use super::{design::PreparedIvDesign, model::IvModel};
use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::causal::iv::FirstStageSummary;
use yss_sci_contract::regression::OlsCovariance;

use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor},
    statistics::Statistics,
};
use yss_sci_linalg::Mat;
use yss_sci_linalg::{MatrixExt, Solve};

/// When true, use LIML Stock-Yogo size critical values (bias=None). When false, use 2SLS.
pub(crate) fn compute_first_stage_summary(
    model: &IvModel,
    design: &PreparedIvDesign,
    for_liml: bool,
) -> Result<FirstStageSummary, String> {
    let z = &design.z;
    let ztz_inv_nd = &design.ztz_inverse;
    let endog_hat = &design.endog_hat;
    let endog_reg = &model.endog_reg;
    let instruments = &model.instruments;
    let has_constant = model.options.constant;
    let covariance = &model.options.covariance;
    let small = model.small;
    let n = z.nrows();
    let k_z = z.ncols();
    let k_exog = model.exog.ncols();
    let k_iv = instruments.ncols();
    let k_endog = endog_reg.ncols();
    let k1 = if has_constant { k_exog + 1 } else { k_exog };
    let df_z = n - k_z;

    // X1 is the included-instrument prefix of the already prepared design.
    let x1 = z.subcols(0, k1).to_owned();

    let x1tx1 = x1.transpose() * x1.as_ref();
    let x1tx1_inv_nd = x1tx1
        .checked_cholesky()
        .map_err(|_| "IV2SLS firststage: X1'X1 not pd".to_string())?
        .solve(&Mat::identity(x1tx1.nrows(), x1tx1.nrows()));
    // Apply projections to the required columns, without materializing an N-by-N matrix.
    let mx1_y = endog_reg.as_ref()
        - (x1.as_ref() * (x1tx1_inv_nd.as_ref() * (x1.transpose() * endog_reg.as_ref()))).as_ref();
    let mx1_x2 = instruments.as_ref()
        - (x1.as_ref() * (x1tx1_inv_nd.as_ref() * (x1.transpose() * instruments.as_ref())))
            .as_ref();
    let mz_y = endog_reg.as_ref() - endog_hat.as_ref();

    // Σ_VV = (1/(N-k_z)) Y' M_Z Y
    let sigma_vv = (mz_y.transpose() * mz_y.as_ref()) / yss_sci_linalg::Scale(df_z as f64);

    // Instrument-explained variation after removing included exogenous columns.
    let x2_mx1_x2 = mx1_x2.transpose() * mx1_x2.as_ref();
    let x2_mx1_x2_inv = x2_mx1_x2
        .checked_cholesky()
        .map_err(|_| "IV2SLS firststage: X2'M_X1 X2 not pd".to_string())?
        .solve(&Mat::identity(x2_mx1_x2.nrows(), x2_mx1_x2.nrows()));
    let cross = mx1_x2.transpose() * mx1_y.as_ref();
    let inner = cross.transpose() * x2_mx1_x2_inv.as_ref() * cross.as_ref();
    // Normalizing by excluded instruments makes the single-endogenous statistic equal F.
    let inner_scaled = inner / yss_sci_linalg::Scale(k_iv.max(1) as f64);

    // Generalized minimum eigenvalue of (inner/k_iv, Sigma_VV).
    let min_eigenvalue_from_cd = if k_endog == 1 {
        if sigma_vv[(0, 0)] > 1e-300 {
            inner_scaled[(0, 0)] / sigma_vv[(0, 0)]
        } else {
            0.0
        }
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

    let min_eigenvalue = min_eigenvalue_from_cd;
    let is_robust = is_robust_covariance(covariance);
    let min_eigenvalue_cv = if !is_robust {
        if for_liml {
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
        // Single endog: R2, Adj R2, Partial R2, F
        let y_col = endog_reg.col(0).to_owned();
        let y_hat = endog_hat.col(0).to_owned();
        let y_mean = y_col.iter().mean();
        let ss_tot = y_col.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>();
        let ss_resid = y_col
            .iter()
            .zip(y_hat.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>();
        let r2 = if ss_tot > 1e-300 {
            1.0 - ss_resid / ss_tot
        } else {
            0.0
        };
        let r2_adj = if ss_tot == 0.0 {
            None
        } else {
            Some(1.0 - (ss_resid / df_z as f64) / (ss_tot / (n - 1) as f64))
        };

        // Partial R2: regress M_X1*Y on M_X1*X2
        let my = mx1_y.col(0).to_owned();
        let mx2t_my = mx1_x2.transpose() * my.as_ref();
        let xi = x2_mx1_x2_inv.as_ref() * mx2t_my.as_ref();
        let fitted = mx1_x2.as_ref() * xi.as_ref();
        let ss_resid_partial: f64 = my
            .iter()
            .zip(fitted.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        let ss_tot_partial: f64 = my.iter().map(|v| v.powi(2)).sum();
        let partial_r2 = if ss_tot_partial > 1e-300 {
            1.0 - ss_resid_partial / ss_tot_partial
        } else {
            0.0
        };

        // F: H0: π2=0. Nonrobust: F = (R2_full - R2_r)/(1-R2_full) * (n-k_z)/k_iv
        // Robust: Wald/k_iv for F-like
        let (f_stat, f_p_value, f_df1, f_df2) = if is_robust {
            let first_stage_resid = mz_y.col(0).to_owned();
            let sigma2_df = if small { df_z } else { n };
            let cov_gamma = compute_cov_beta(
                z,
                ztz_inv_nd,
                &first_stage_resid,
                sigma2_df,
                has_constant.then_some(0),
                covariance,
            )?;
            let gamma = design.first_stage_coefficients.col(0).to_owned();
            let gamma2 = gamma.subrows(k1, gamma.nrows() - k1).to_owned();
            let cov_gamma2 = cov_gamma
                .submatrix(k1, k1, cov_gamma.nrows() - k1, cov_gamma.ncols() - k1)
                .to_owned();
            let cov_gamma2_inv = cov_gamma2
                .as_ref()
                .to_owned()
                .checked_cholesky()
                .map_err(|_| "IV2SLS firststage: cov_gamma2 not pd".to_string())?
                .solve(&Mat::identity(cov_gamma2.nrows(), cov_gamma2.nrows()));
            let wald = gamma2.transpose() * (cov_gamma2_inv.as_ref() * gamma2.as_ref()).as_ref();
            let chi2 = ChiSquared::new(k_iv as f64).map_err(|e| format!("{}", e))?;
            let f_p = chi2.sf(wald);
            (wald / k_iv as f64, f_p, k_iv, df_z)
        } else {
            let ssr_r: f64 = my.iter().map(|value| value.powi(2)).sum();
            let ssr_u = ss_resid;
            let f_val = if ssr_u > 1e-300 {
                ((ssr_r - ssr_u) / k_iv as f64) / (ssr_u / df_z as f64)
            } else {
                0.0
            };
            let f_dist =
                FisherSnedecor::new(k_iv as f64, df_z as f64).map_err(|e| format!("{}", e))?;
            let f_p = crate::distribution::fisher_snedecor_sf(&f_dist, f_val);
            (f_val, f_p, k_iv, df_z)
        };

        (
            Some(r2),
            r2_adj,
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
            let y1 = endog_reg.col(j).to_owned();
            let y1_hat = endog_hat.col(j).to_owned();
            let (y0, y0_hat) = if k_endog > 1 {
                let mut y0_data = Vec::with_capacity(n * (k_endog - 1));
                let mut y0_hat_data = Vec::with_capacity(n * (k_endog - 1));
                for i in 0..n {
                    for jj in 0..k_endog {
                        if jj != j {
                            y0_data.push(endog_reg[(i, jj)]);
                            y0_hat_data.push(endog_hat[(i, jj)]);
                        }
                    }
                }
                let y0_mat =
                    yss_sci_linalg::MatRef::from_row_major_slice(&(y0_data), n, k_endog - 1)
                        .to_owned();
                let y0_hat_mat =
                    yss_sci_linalg::MatRef::from_row_major_slice(&(y0_hat_data), n, k_endog - 1)
                        .to_owned();
                (Some(y0_mat), Some(y0_hat_mat))
            } else {
                (None, None)
            };

            let w = if let Some(ref y0) = y0 {
                let mut w = Mat::zeros(n, k1 + y0.ncols());
                for i in 0..n {
                    for c in 0..k1 {
                        w[(i, c)] = x1[(i, c)];
                    }
                    for c in 0..y0.ncols() {
                        w[(i, k1 + c)] = y0[(i, c)];
                    }
                }
                w
            } else {
                x1.clone()
            };
            let wtw = w.transpose() * w.as_ref();
            let wtw_inv = wtw
                .as_ref()
                .to_owned()
                .checked_cholesky()
                .map_err(|_| "IV2SLS firststage: W'W not pd".to_string())?
                .solve(&Mat::identity(wtw.nrows(), wtw.nrows()));
            let wtw_inv_nd = wtw_inv.as_ref().to_owned();

            let y1_tilde = &y1
                - &(w.as_ref()
                    * (wtw_inv_nd.as_ref() * (w.transpose() * y1.as_ref()).as_ref()).as_ref());
            let y1_hat_tilde = if let Some(ref y0h) = y0_hat {
                let mut w_hat = Mat::zeros(n, k1 + y0h.ncols());
                for i in 0..n {
                    for c in 0..k1 {
                        w_hat[(i, c)] = x1[(i, c)];
                    }
                    for c in 0..y0h.ncols() {
                        w_hat[(i, k1 + c)] = y0h[(i, c)];
                    }
                }
                let w_hat_t_w_hat = w_hat.transpose() * w_hat.as_ref();
                let w_hat_t_w_hat_inv = w_hat_t_w_hat
                    .as_ref()
                    .to_owned()
                    .checked_cholesky()
                    .map_err(|_| "IV2SLS firststage: W_hat'W_hat not pd".to_string())?
                    .solve(&Mat::identity(w_hat_t_w_hat.nrows(), w_hat_t_w_hat.nrows()));
                let proj = w_hat.as_ref()
                    * (w_hat_t_w_hat_inv.as_ref() * (w_hat.transpose() * y1_hat.as_ref()));
                &y1_hat - &proj
            } else {
                &y1_hat
                    - &(x1.as_ref()
                        * (x1tx1_inv_nd.as_ref() * (x1.transpose() * y1_hat.as_ref()).as_ref())
                            .as_ref())
            };

            let ss_tot = y1_tilde.iter().map(|v| v.powi(2)).sum::<f64>();
            let ss_resid = y1_tilde
                .iter()
                .zip(y1_hat_tilde.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>();
            let r2_s = if ss_tot > 1e-300 {
                1.0 - ss_resid / ss_tot
            } else {
                0.0
            };
            let r2_s_adj = if has_constant {
                1.0 - (1.0 - r2_s) * (n - 1) as f64 / (n - k_z + 1) as f64
            } else {
                1.0 - (1.0 - r2_s) * (n - 1) as f64 / (n - k_z) as f64
            };
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

pub(super) fn is_robust_covariance(covariance: &OlsCovariance) -> bool {
    matches!(
        covariance,
        OlsCovariance::Hc0
            | OlsCovariance::Hc1
            | OlsCovariance::Hc2
            | OlsCovariance::Hc3
            | OlsCovariance::Cluster { .. }
            | OlsCovariance::Hac { .. }
            | OlsCovariance::Newey { .. }
    )
}
