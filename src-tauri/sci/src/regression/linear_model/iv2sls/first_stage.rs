use super::critical_values::{
    stock_yogo_cv_1_endog, stock_yogo_cv_2_endog, stock_yogo_cv_liml_1_endog,
    stock_yogo_cv_liml_2_endog,
};
use super::types::FirstStageSummary;
use crate::regression::covariance::{CovParams, compute_cov_beta};
use crate::tools::{IntoFaer, IntoNdarray};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::Array2;
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor},
    statistics::Statistics,
};

/// When true, use LIML Stock-Yogo size critical values (bias=None). When false, use 2SLS.
pub(crate) fn compute_first_stage_summary(
    z: &Array2<f64>,
    endog_hat: &Array2<f64>,
    endog_reg: &Array2<f64>,
    exog: &Array2<f64>,
    instruments: &Array2<f64>,
    n: usize,
    k_z: usize,
    k_exog: usize,
    k_iv: usize,
    k_endog: usize,
    has_constant: bool,
    cov_type: &str,
    cov_params: Option<&CovParams>,
    small: bool,
    for_liml: bool,
) -> Result<FirstStageSummary, String> {
    let k1 = if has_constant { k_exog + 1 } else { k_exog };
    let df_z = n.saturating_sub(k_z);

    // X1 = [const?, exog]
    let mut x1_raw = Vec::with_capacity(n * k1);
    for i in 0..n {
        if has_constant {
            x1_raw.push(1.0);
        }
        for j in 0..k_exog {
            x1_raw.push(exog[[i, j]]);
        }
    }
    let x1 =
        Array2::from_shape_vec((n, k1), x1_raw).map_err(|e| format!("IV2SLS firststage: {}", e))?;

    let x1tx1 = x1.t().dot(&x1);
    let x1tx1_inv = x1tx1
        .view()
        .into_faer()
        .to_owned()
        .llt(Side::Lower)
        .map_err(|_| "IV2SLS firststage: X1'X1 not pd".to_string())?
        .solve(Mat::identity(x1tx1.nrows(), x1tx1.ncols()));
    let x1tx1_inv_nd = x1tx1_inv.as_ref().into_ndarray().to_owned();

    // M_X1 = I - X1(X1'X1)^{-1}X1'
    let px1 = x1.dot(&x1tx1_inv_nd).dot(&x1.t());
    let mut mx1_y_data = Vec::with_capacity(n * k_endog);
    for i in 0..n {
        for j in 0..k_endog {
            let y_val = endog_reg[[i, j]];
            let py_val: f64 = (0..n).map(|ii| px1[[i, ii]] * endog_reg[[ii, j]]).sum();
            mx1_y_data.push(y_val - py_val);
        }
    }
    let mx1_y = Array2::from_shape_vec((n, k_endog), mx1_y_data)
        .map_err(|e| format!("IV2SLS firststage: {}", e))?;
    let mut mx1_x2_data = Vec::with_capacity(n * k_iv);
    for i in 0..n {
        for j in 0..k_iv {
            let x2_val = instruments[[i, j]];
            let px_val: f64 = (0..n).map(|ii| px1[[i, ii]] * instruments[[ii, j]]).sum();
            mx1_x2_data.push(x2_val - px_val);
        }
    }
    let mx1_x2 = Array2::from_shape_vec((n, k_iv), mx1_x2_data)
        .map_err(|e| format!("IV2SLS firststage: {}", e))?;

    // M_Z for Cragg-Donald
    let ztz = z.t().dot(z);
    let ztz_inv = ztz
        .view()
        .into_faer()
        .to_owned()
        .llt(Side::Lower)
        .map_err(|_| "IV2SLS firststage: Z'Z not pd".to_string())?
        .solve(Mat::identity(ztz.nrows(), ztz.ncols()));
    let ztz_inv_nd = ztz_inv.as_ref().into_ndarray().to_owned();
    let pz = z.dot(&ztz_inv_nd).dot(&z.t());
    let mut mz_y_data = Vec::with_capacity(n * k_endog);
    for i in 0..n {
        for j in 0..k_endog {
            let y_val = endog_reg[[i, j]];
            let py_val: f64 = (0..n).map(|ii| pz[[i, ii]] * endog_reg[[ii, j]]).sum();
            mz_y_data.push(y_val - py_val);
        }
    }
    let mz_y = Array2::from_shape_vec((n, k_endog), mz_y_data)
        .map_err(|e| format!("IV2SLS firststage: {}", e))?;

    // Σ_VV = (1/(N-k_z)) Y' M_Z Y
    let sigma_vv = mz_y.t().dot(&mz_y) / df_z.max(1) as f64;

    // Min eigenvalue: Cragg-Donald. G = (1/k_z) Σ_VV^{-1/2} Y' M_X1' X2 (X2' M_X1 X2)^{-1} X2' M_X1 Y Σ_VV^{-1/2}
    let x2_mx1 = mx1_x2.clone();
    let x2_mx1_x2 = x2_mx1.t().dot(&x2_mx1);
    let x2_mx1_x2_inv = x2_mx1_x2
        .view()
        .into_faer()
        .to_owned()
        .llt(Side::Lower)
        .map_err(|_| "IV2SLS firststage: X2'M_X1 X2 not pd".to_string())?
        .solve(Mat::identity(x2_mx1_x2.nrows(), x2_mx1_x2.ncols()));
    let x2_mx1_x2_inv_nd = x2_mx1_x2_inv.as_ref().into_ndarray().to_owned();

    let mut y_mx1_data = Vec::with_capacity(n * k_endog);
    for i in 0..n {
        for j in 0..k_endog {
            let y_val = endog_reg[[i, j]];
            let py_val: f64 = (0..n).map(|ii| px1[[i, ii]] * endog_reg[[ii, j]]).sum();
            y_mx1_data.push(y_val - py_val);
        }
    }
    let y_mx1 = Array2::from_shape_vec((n, k_endog), y_mx1_data)
        .map_err(|e| format!("IV2SLS firststage: {}", e))?;
    let inner = (y_mx1.t().dot(&x2_mx1))
        .dot(&x2_mx1_x2_inv_nd)
        .dot(&x2_mx1.t().dot(&y_mx1));
    // Cragg-Donald uses k_iv (excluded instruments), not k_z. For k_endog=1, min_eig = F stat.
    // F = (inner/k_iv) / sigma_vv => min_eig = inner/(k_iv*sigma_vv). Wrong: inner/(k_z*sigma_vv) gave F/2 when k_z=1+k_iv.
    let inner_scaled = inner / k_iv.max(1) as f64;

    // Min eigenvalue: Cragg-Donald = min eig of Σ_VV^{-1} * (1/k_iv) * inner. For k_endog=1, equals F stat.
    let min_eigenvalue_from_cd = if k_endog == 1 {
        if sigma_vv[[0, 0]] > 1e-300 {
            inner_scaled[[0, 0]] / sigma_vv[[0, 0]]
        } else {
            0.0
        }
    } else {
        let sigma_inv = sigma_vv
            .view()
            .into_faer()
            .to_owned()
            .llt(Side::Lower)
            .map_err(|_| "IV2SLS firststage: sigma_vv not pd".to_string())?
            .solve(Mat::identity(sigma_vv.nrows(), sigma_vv.ncols()));
        let g_mat = sigma_inv
            .as_ref()
            .into_ndarray()
            .to_owned()
            .dot(&inner_scaled);
        let g_faer = g_mat.view().into_faer();
        let evd = faer::linalg::solvers::SelfAdjointEigen::new(g_faer, Side::Lower)
            .map_err(|_| "IV2SLS firststage: EVD failed".to_string())?;
        let s_col = evd.S().column_vector();
        s_col.iter().fold(f64::INFINITY, |a, &b| a.min(b))
    };

    let min_eigenvalue = min_eigenvalue_from_cd;
    let is_robust = is_robust_cov_type(cov_type);
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
        None
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
        let y_col = endog_reg.column(0).into_owned();
        let y_hat = endog_hat.column(0).into_owned();
        let ss_tot = y_col
            .iter()
            .map(|v| (v - y_col.iter().mean()).powi(2))
            .sum::<f64>();
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
        let r2_adj = if df_z > 0 && n > 1 {
            1.0 - (ss_resid / df_z as f64) / (ss_tot / (n - 1) as f64)
        } else {
            r2
        };

        // Partial R2: regress M_X1*Y on M_X1*X2
        let my = mx1_y.column(0).into_owned();
        let mx2 = mx1_x2;
        let mx2t_mx2 = mx2.t().dot(&mx2);
        let mx2t_my = mx2.t().dot(&my);
        let mx2t_mx2_inv = mx2t_mx2
            .view()
            .into_faer()
            .to_owned()
            .llt(Side::Lower)
            .map_err(|_| "IV2SLS firststage: M_X2'M_X2 not pd".to_string())?
            .solve(Mat::identity(mx2t_mx2.nrows(), mx2t_mx2.ncols()));
        let xi = mx2t_mx2_inv
            .as_ref()
            .into_ndarray()
            .to_owned()
            .dot(&mx2t_my);
        let fitted = mx2.dot(&xi);
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
            let first_stage_resid = &y_col
                - &z.dot(&{
                    let zty = z.t().dot(&y_col);
                    ztz_inv_nd.dot(&zty)
                });
            let sigma2_df = if small { df_z } else { n };
            let cov_gamma = compute_cov_beta(
                z,
                &ztz_inv_nd,
                &first_stage_resid,
                sigma2_df,
                cov_type,
                cov_params,
            )?;
            let gamma = ztz_inv_nd.dot(&z.t().dot(&y_col));
            let gamma2 = gamma.slice(ndarray::s![k1..]).into_owned();
            let cov_gamma2 = cov_gamma.slice(ndarray::s![k1.., k1..]).into_owned();
            let cov_gamma2_inv = cov_gamma2
                .view()
                .into_faer()
                .to_owned()
                .llt(Side::Lower)
                .map_err(|_| "IV2SLS firststage: cov_gamma2 not pd".to_string())?
                .solve(Mat::identity(cov_gamma2.nrows(), cov_gamma2.ncols()));
            let wald = gamma2.dot(
                &cov_gamma2_inv
                    .as_ref()
                    .into_ndarray()
                    .to_owned()
                    .dot(&gamma2),
            );
            let chi2 = ChiSquared::new(k_iv as f64).map_err(|e| format!("{}", e))?;
            let f_p = 1.0 - chi2.cdf(wald);
            (wald / k_iv as f64, f_p, k_iv, df_z)
        } else {
            let ssr_r: f64 = y_col
                .iter()
                .zip(x1.dot(&x1tx1_inv_nd.dot(&x1.t().dot(&y_col))).iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            let ssr_u = ss_resid;
            let f_val = if ssr_u > 1e-300 && df_z > 0 {
                ((ssr_r - ssr_u) / k_iv as f64) / (ssr_u / df_z as f64)
            } else {
                0.0
            };
            let f_dist =
                FisherSnedecor::new(k_iv as f64, df_z as f64).map_err(|e| format!("{}", e))?;
            let f_p = 1.0 - f_dist.cdf(f_val);
            (f_val, f_p, k_iv, df_z)
        };

        (
            Some(r2),
            Some(r2_adj),
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
            let y1 = endog_reg.column(j).into_owned();
            let y1_hat = endog_hat.column(j).into_owned();
            let (y0, y0_hat) = if k_endog > 1 {
                let mut y0_data = Vec::with_capacity(n * (k_endog - 1));
                let mut y0_hat_data = Vec::with_capacity(n * (k_endog - 1));
                for jj in 0..k_endog {
                    if jj != j {
                        for i in 0..n {
                            y0_data.push(endog_reg[[i, jj]]);
                            y0_hat_data.push(endog_hat[[i, jj]]);
                        }
                    }
                }
                let y0_mat = Array2::from_shape_vec((n, k_endog - 1), y0_data)
                    .map_err(|e| format!("IV2SLS firststage: {}", e))?;
                let y0_hat_mat = Array2::from_shape_vec((n, k_endog - 1), y0_hat_data)
                    .map_err(|e| format!("IV2SLS firststage: {}", e))?;
                (Some(y0_mat), Some(y0_hat_mat))
            } else {
                (None, None)
            };

            let w = if let Some(ref y0) = y0 {
                let mut w = Array2::zeros((n, k1 + y0.ncols()));
                for i in 0..n {
                    for c in 0..k1 {
                        w[[i, c]] = x1[[i, c]];
                    }
                    for c in 0..y0.ncols() {
                        w[[i, k1 + c]] = y0[[i, c]];
                    }
                }
                w
            } else {
                x1.clone()
            };
            let wtw = w.t().dot(&w);
            let wtw_inv = wtw
                .view()
                .into_faer()
                .to_owned()
                .llt(Side::Lower)
                .map_err(|_| "IV2SLS firststage: W'W not pd".to_string())?
                .solve(Mat::identity(wtw.nrows(), wtw.ncols()));
            let wtw_inv_nd = wtw_inv.as_ref().into_ndarray().to_owned();

            let y1_tilde = &y1 - &w.dot(&wtw_inv_nd.dot(&w.t().dot(&y1)));
            let y1_hat_tilde = if let Some(ref y0h) = y0_hat {
                let mut w_hat = Array2::zeros((n, k1 + y0h.ncols()));
                for i in 0..n {
                    for c in 0..k1 {
                        w_hat[[i, c]] = x1[[i, c]];
                    }
                    for c in 0..y0h.ncols() {
                        w_hat[[i, k1 + c]] = y0h[[i, c]];
                    }
                }
                let w_hat_t_w_hat = w_hat.t().dot(&w_hat);
                let w_hat_t_w_hat_inv = w_hat_t_w_hat
                    .view()
                    .into_faer()
                    .to_owned()
                    .llt(Side::Lower)
                    .map_err(|_| "IV2SLS firststage: W_hat'W_hat not pd".to_string())?
                    .solve(Mat::identity(w_hat_t_w_hat.nrows(), w_hat_t_w_hat.ncols()));
                let proj = w_hat.dot(
                    &w_hat_t_w_hat_inv
                        .as_ref()
                        .into_ndarray()
                        .to_owned()
                        .dot(&w_hat.t().dot(&y1_hat)),
                );
                &y1_hat - &proj
            } else {
                &y1_hat - &x1.dot(&x1tx1_inv_nd.dot(&x1.t().dot(&y1_hat)))
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

pub(super) fn is_robust_cov_type(cov_type: &str) -> bool {
    matches!(
        cov_type,
        "HC0" | "HC1" | "HC2" | "HC3" | "cluster" | "HAC" | "newey"
    )
}
