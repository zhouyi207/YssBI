//! IV:2SLS (Instrumental Variables Two-Stage Least Squares)
//!
//! Stata ivregress 2sls: depvar [varlist1] (varlist2 = varlistiv)
//! - varlist1: exogenous variables (in both stages)
//! - varlist2: endogenous variables (instrumented in stage 1)
//! - varlistiv: instruments (stage 1 only)
//!
//! Stage 1: Regress each endogenous on Z = [exog, instruments] → endog_hat
//! Stage 2: Regress Y on X = [exog, endog_hat] → β. VCE uses structural residuals u = y - X_struct*β.

use crate::regression::covariance::{CovParams, compute_cov_beta};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal, StudentsT},
    statistics::Statistics,
};

/// 2SLS 配置，与 OLS 一致（constant, cov_type, cov_params）
pub struct IV2SLSConfig {
    pub constant: bool,
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
    /// Stata small: if true, use ESS/(n-k) for σ²; if false, use ESS/n (Stata default).
    pub small: bool,
}

/// IV:2SLS 输入
/// - endog: y (n,)
/// - exog: exogenous variables (n × k_exog)，不含 constant
/// - endog_reg: endogenous variables (n × k_endog)
/// - instruments: instruments (n × k_iv)
///
/// 识别条件: k_iv >= k_endog
pub struct IV2SLS {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub endog_reg: Array2<f64>,
    pub instruments: Array2<f64>,
    pub config: IV2SLSConfig,
    /// 内生变量名称，用于 first_stage 输出
    pub endog_names: Option<Vec<String>>,
    /// Z 矩阵变量名 [const?, exog..., instruments...]，用于 first_stage 系数标签
    pub z_var_names: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct IV2SLSModel {
    pub params: Array1<f64>,
}

/// estat firststage 汇总（Stata estat firststage）
/// - 单内生：R², Adj R², Partial R², F, Prob>F, Min eigenvalue
/// - 多内生：Shea partial R², Shea adj partial R², Min eigenvalue
#[derive(Debug, Clone)]
pub struct FirstStageSummary {
    /// Included instruments (X1 excluding constant): exogenous regressors in both structural and first stage
    pub k_included_instruments: usize,
    /// Excluded instruments (X2): only in first stage
    pub k_excluded_instruments: usize,
    /// Endogenous regressors (instrumented)
    pub k_endogenous_regressors: usize,
    /// 单内生时为 Some；多内生时为 None
    pub r2: Option<f64>,
    pub r2_adjusted: Option<f64>,
    pub partial_r2: Option<f64>,
    pub f_stat: Option<f64>,
    pub f_p_value: Option<f64>,
    pub f_df1: Option<usize>,
    pub f_df2: Option<usize>,
    /// 多内生时每变量一个
    pub shea_partial_r2: Vec<f64>,
    pub shea_adj_partial_r2: Vec<f64>,
    pub min_eigenvalue: f64,
    /// 仅 nonrobust 且 k_endog<=2 时提供；否则 None
    pub min_eigenvalue_cv: Option<StockYogoCriticalValues>,
    /// 当 min_eigenvalue_cv 为 None 时的原因："robust" | "k_endog_gt_2"
    pub min_eigenvalue_cv_note: Option<String>,
}

/// Stock-Yogo 2SLS relative bias 临界值（5%, 10%, 20%, 30%）
/// 在 k2 < k1+2 时 Stock-Yogo 未提供，整行为 None
#[derive(Debug, Clone)]
pub struct StockYogoBiasRow {
    pub pct_5: f64,
    pub pct_10: f64,
    pub pct_20: f64,
    pub pct_30: f64,
}

/// Stock-Yogo 2SLS size of nominal 5% Wald test 临界值（10%, 15%, 20%, 25%）
#[derive(Debug, Clone)]
pub struct StockYogoSizeRow {
    pub pct_10: f64,
    pub pct_15: f64,
    pub pct_20: f64,
    pub pct_25: f64,
}

/// Stock-Yogo 弱工具变量临界值（Stata 表）
#[derive(Debug, Clone)]
pub struct StockYogoCriticalValues {
    pub bias: Option<StockYogoBiasRow>,
    pub size: StockYogoSizeRow,
}

/// 第一阶段单方程结果（每个内生变量对 Z = [exog, instruments] 的回归）
#[derive(Debug, Clone)]
pub struct FirstStageResult {
    pub endog_name: String,
    /// 自变量名称（const, exog..., instruments...）
    pub var_names: Vec<String>,
    pub betas: Vec<f64>,
    pub stds: Vec<f64>,
    pub tvalues: Vec<f64>,
    pub pvalues: Vec<f64>,
    pub conf_int_left: Vec<f64>,
    pub conf_int_right: Vec<f64>,
    pub r2: f64,
    pub r2_adjusted: f64,
}

#[derive(Debug)]
pub struct IV2SLSResult {
    pub num_observation: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
    pub r2: f64,
    pub r2_adjusted: f64,
    /// Wald chi2 for joint significance (2SLS uses asymptotic inference, not F)
    pub wald_chi2: f64,
    pub wald_chi2_p_value: f64,

    pub model: IV2SLSModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    /// z-statistics (2SLS uses asymptotic normal inference, not t)
    pub zvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,

    /// 第一阶段回归结果（每个内生变量）
    pub first_stage: Vec<FirstStageResult>,
    /// estat firststage 汇总
    pub first_stage_summary: FirstStageSummary,

    /// 过度识别检验（estat overid），仅当 k_iv > k_endog 时计算（排除的工具变量数 > 内生变量数）
    pub overid: Option<OveridTest>,
    /// 用于诊断：k_iv = 排除的工具变量数，k_endog = 内生变量数
    pub overid_k_iv: usize,
    pub overid_k_endog: usize,

    /// 传统豪斯曼检验（hausman iv ols, constant sigmamore），仅 nonrobust VCE
    pub hausman: Option<HausmanTest>,
    /// Durbin-Wu-Hausman 内生性检验（estat endogenous），仅 nonrobust VCE
    pub endogenous: Option<EndogenousTest>,
}

/// 豪斯曼检验结果（Stata hausman iv ols, constant sigmamore）
/// 仅当 nonrobust VCE 时计算。H0: 内生变量可视为外生（OLS 与 IV 一致）
#[derive(Debug, Clone)]
pub struct HausmanTest {
    pub stat: f64,
    pub p_value: f64,
    pub df: usize,
}

/// 内生性检验结果（Stata estat endogenous）
/// Durbin (1954) score test 与 Wu-Hausman (Wu 1974; Hausman 1978) test
/// 仅当 nonrobust VCE 时计算。H0: 被检内生变量可视为外生
#[derive(Debug, Clone)]
pub struct EndogenousTest {
    pub durbin_stat: f64,
    pub durbin_p_value: f64,
    pub wu_stat: f64,
    pub wu_p_value: f64,
    pub df: usize,
    pub wu_df_denom: usize,
}

/// 过度识别检验结果（Stata estat overid）
/// - 同方差（nonrobust）：Sargan、Basmann
/// - 稳健 VCE（HC0/HC1/HC2/HC3/cluster/HAC/newey）：Wooldridge (1995) robust score test
#[derive(Debug, Clone)]
pub struct OveridTest {
    /// "sargan_basmann" | "wooldridge"
    pub test_type: String,
    /// Sargan/Basmann（同方差时有效）
    pub sargan_stat: Option<f64>,
    pub sargan_p_value: Option<f64>,
    pub basmann_stat: Option<f64>,
    pub basmann_p_value: Option<f64>,
    /// Wooldridge score（稳健 VCE 时有效，Stata estat overid）
    pub wooldridge_stat: Option<f64>,
    pub wooldridge_p_value: Option<f64>,
    pub df: usize,
}

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

fn is_robust_cov_type(cov_type: &str) -> bool {
    matches!(
        cov_type,
        "HC0" | "HC1" | "HC2" | "HC3" | "cluster" | "HAC" | "newey"
    )
}

/// Stock-Yogo (2005) 临界值，1 内生变量。k2=排除工具数。与 Stata ivreg2/estat firststage 一致。
/// 来源: livreg2.do s_ivbias*, s_ivsize*
/// bias 在 k2=1,2 时为 None（Stock-Yogo 未提供）
fn stock_yogo_cv_1_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let (bias, size) = match k2 {
        1 => (
            None,
            StockYogoSizeRow {
                pct_10: 16.38,
                pct_15: 8.96,
                pct_20: 6.66,
                pct_25: 5.53,
            },
        ),
        2 => (
            None,
            StockYogoSizeRow {
                pct_10: 19.93,
                pct_15: 11.59,
                pct_20: 8.75,
                pct_25: 7.25,
            },
        ),
        3 => (
            Some(StockYogoBiasRow {
                pct_5: 22.30,
                pct_10: 12.83,
                pct_20: 7.80,
                pct_30: 5.91,
            }),
            StockYogoSizeRow {
                pct_10: 22.30,
                pct_15: 12.83,
                pct_20: 9.54,
                pct_25: 7.80,
            },
        ),
        4 => (
            Some(StockYogoBiasRow {
                pct_5: 16.85,
                pct_10: 10.27,
                pct_20: 6.71,
                pct_30: 5.34,
            }),
            StockYogoSizeRow {
                pct_10: 24.58,
                pct_15: 13.96,
                pct_20: 10.26,
                pct_25: 8.31,
            },
        ),
        5 => (
            Some(StockYogoBiasRow {
                pct_5: 18.37,
                pct_10: 10.91,
                pct_20: 7.03,
                pct_30: 5.54,
            }),
            StockYogoSizeRow {
                pct_10: 26.87,
                pct_15: 15.09,
                pct_20: 10.98,
                pct_25: 8.84,
            },
        ),
        6 => (
            Some(StockYogoBiasRow {
                pct_5: 19.86,
                pct_10: 11.52,
                pct_20: 7.34,
                pct_30: 5.73,
            }),
            StockYogoSizeRow {
                pct_10: 29.18,
                pct_15: 16.23,
                pct_20: 11.72,
                pct_25: 9.38,
            },
        ),
        7 => (
            Some(StockYogoBiasRow {
                pct_5: 21.33,
                pct_10: 12.12,
                pct_20: 7.64,
                pct_30: 5.91,
            }),
            StockYogoSizeRow {
                pct_10: 31.50,
                pct_15: 17.38,
                pct_20: 12.48,
                pct_25: 9.93,
            },
        ),
        8 => (
            Some(StockYogoBiasRow {
                pct_5: 22.78,
                pct_10: 12.70,
                pct_20: 7.93,
                pct_30: 6.08,
            }),
            StockYogoSizeRow {
                pct_10: 33.84,
                pct_15: 18.54,
                pct_20: 13.24,
                pct_25: 10.50,
            },
        ),
        9 => (
            Some(StockYogoBiasRow {
                pct_5: 24.21,
                pct_10: 13.27,
                pct_20: 8.21,
                pct_30: 6.25,
            }),
            StockYogoSizeRow {
                pct_10: 36.19,
                pct_15: 19.71,
                pct_20: 14.01,
                pct_25: 11.07,
            },
        ),
        10 => (
            Some(StockYogoBiasRow {
                pct_5: 25.63,
                pct_10: 13.83,
                pct_20: 8.48,
                pct_30: 6.41,
            }),
            StockYogoSizeRow {
                pct_10: 38.54,
                pct_15: 20.88,
                pct_20: 14.78,
                pct_25: 11.65,
            },
        ),
        11 => (
            Some(StockYogoBiasRow {
                pct_5: 27.03,
                pct_10: 14.38,
                pct_20: 8.75,
                pct_30: 6.57,
            }),
            StockYogoSizeRow {
                pct_10: 40.90,
                pct_15: 22.06,
                pct_20: 15.56,
                pct_25: 12.23,
            },
        ),
        12 => (
            Some(StockYogoBiasRow {
                pct_5: 28.42,
                pct_10: 14.92,
                pct_20: 9.01,
                pct_30: 6.72,
            }),
            StockYogoSizeRow {
                pct_10: 43.27,
                pct_15: 23.24,
                pct_20: 16.35,
                pct_25: 12.82,
            },
        ),
        13 => (
            Some(StockYogoBiasRow {
                pct_5: 29.80,
                pct_10: 15.45,
                pct_20: 9.26,
                pct_30: 6.87,
            }),
            StockYogoSizeRow {
                pct_10: 45.64,
                pct_15: 24.42,
                pct_20: 17.14,
                pct_25: 13.41,
            },
        ),
        14 => (
            Some(StockYogoBiasRow {
                pct_5: 31.16,
                pct_10: 15.97,
                pct_20: 9.51,
                pct_30: 7.01,
            }),
            StockYogoSizeRow {
                pct_10: 48.01,
                pct_15: 25.61,
                pct_20: 17.93,
                pct_25: 14.00,
            },
        ),
        15 => (
            Some(StockYogoBiasRow {
                pct_5: 32.52,
                pct_10: 16.49,
                pct_20: 9.75,
                pct_30: 7.15,
            }),
            StockYogoSizeRow {
                pct_10: 50.39,
                pct_15: 26.80,
                pct_20: 18.72,
                pct_25: 14.60,
            },
        ),
        16 => (
            Some(StockYogoBiasRow {
                pct_5: 33.86,
                pct_10: 17.00,
                pct_20: 9.99,
                pct_30: 7.28,
            }),
            StockYogoSizeRow {
                pct_10: 52.77,
                pct_15: 27.99,
                pct_20: 19.51,
                pct_25: 15.19,
            },
        ),
        17 => (
            Some(StockYogoBiasRow {
                pct_5: 35.20,
                pct_10: 17.50,
                pct_20: 10.22,
                pct_30: 7.41,
            }),
            StockYogoSizeRow {
                pct_10: 55.15,
                pct_15: 29.19,
                pct_20: 20.31,
                pct_25: 15.79,
            },
        ),
        18 => (
            Some(StockYogoBiasRow {
                pct_5: 36.52,
                pct_10: 18.00,
                pct_20: 10.45,
                pct_30: 7.54,
            }),
            StockYogoSizeRow {
                pct_10: 57.53,
                pct_15: 30.38,
                pct_20: 21.10,
                pct_25: 16.39,
            },
        ),
        19 => (
            Some(StockYogoBiasRow {
                pct_5: 37.84,
                pct_10: 18.49,
                pct_20: 10.67,
                pct_30: 7.66,
            }),
            StockYogoSizeRow {
                pct_10: 59.92,
                pct_15: 31.58,
                pct_20: 21.90,
                pct_25: 16.99,
            },
        ),
        20 => (
            Some(StockYogoBiasRow {
                pct_5: 39.15,
                pct_10: 18.97,
                pct_20: 10.89,
                pct_30: 7.78,
            }),
            StockYogoSizeRow {
                pct_10: 62.30,
                pct_15: 32.77,
                pct_20: 22.70,
                pct_25: 17.60,
            },
        ),
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias, size })
}

/// Stock-Yogo (2005) 临界值，2 内生变量。k2=排除工具数。与 Stata ivreg2/estat firststage 一致。
/// 来源: livreg2.do s_ivbias*, s_ivsize* (K1=2 列)
/// bias 在 k2=2,3 时为 None（Stock-Yogo 未提供）
fn stock_yogo_cv_2_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let (bias, size) = match k2 {
        2 => (
            None,
            StockYogoSizeRow {
                pct_10: 7.03,
                pct_15: 4.58,
                pct_20: 3.95,
                pct_25: 3.63,
            },
        ),
        3 => (
            None,
            StockYogoSizeRow {
                pct_10: 13.43,
                pct_15: 8.18,
                pct_20: 6.40,
                pct_25: 5.45,
            },
        ),
        4 => (
            Some(StockYogoBiasRow {
                pct_5: 11.04,
                pct_10: 7.56,
                pct_20: 5.57,
                pct_30: 4.73,
            }),
            StockYogoSizeRow {
                pct_10: 16.87,
                pct_15: 9.93,
                pct_20: 7.54,
                pct_25: 6.28,
            },
        ),
        5 => (
            Some(StockYogoBiasRow {
                pct_5: 12.16,
                pct_10: 8.18,
                pct_20: 5.91,
                pct_30: 4.96,
            }),
            StockYogoSizeRow {
                pct_10: 19.45,
                pct_15: 11.22,
                pct_20: 8.38,
                pct_25: 6.89,
            },
        ),
        6 => (
            Some(StockYogoBiasRow {
                pct_5: 13.27,
                pct_10: 8.79,
                pct_20: 6.23,
                pct_30: 5.18,
            }),
            StockYogoSizeRow {
                pct_10: 21.68,
                pct_15: 12.33,
                pct_20: 9.10,
                pct_25: 7.42,
            },
        ),
        7 => (
            Some(StockYogoBiasRow {
                pct_5: 14.36,
                pct_10: 9.39,
                pct_20: 6.54,
                pct_30: 5.39,
            }),
            StockYogoSizeRow {
                pct_10: 23.72,
                pct_15: 13.34,
                pct_20: 9.77,
                pct_25: 7.91,
            },
        ),
        8 => (
            Some(StockYogoBiasRow {
                pct_5: 15.45,
                pct_10: 9.98,
                pct_20: 6.84,
                pct_30: 5.59,
            }),
            StockYogoSizeRow {
                pct_10: 25.64,
                pct_15: 14.31,
                pct_20: 10.41,
                pct_25: 8.39,
            },
        ),
        9 => (
            Some(StockYogoBiasRow {
                pct_5: 16.53,
                pct_10: 10.56,
                pct_20: 7.13,
                pct_30: 5.78,
            }),
            StockYogoSizeRow {
                pct_10: 27.51,
                pct_15: 15.24,
                pct_20: 11.03,
                pct_25: 8.85,
            },
        ),
        10 => (
            Some(StockYogoBiasRow {
                pct_5: 17.60,
                pct_10: 11.13,
                pct_20: 7.41,
                pct_30: 5.97,
            }),
            StockYogoSizeRow {
                pct_10: 29.32,
                pct_15: 16.16,
                pct_20: 11.65,
                pct_25: 9.31,
            },
        ),
        11 => (
            Some(StockYogoBiasRow {
                pct_5: 18.66,
                pct_10: 11.70,
                pct_20: 7.69,
                pct_30: 6.15,
            }),
            StockYogoSizeRow {
                pct_10: 31.11,
                pct_15: 17.06,
                pct_20: 12.25,
                pct_25: 9.77,
            },
        ),
        12 => (
            Some(StockYogoBiasRow {
                pct_5: 19.72,
                pct_10: 12.26,
                pct_20: 7.96,
                pct_30: 6.33,
            }),
            StockYogoSizeRow {
                pct_10: 32.88,
                pct_15: 17.95,
                pct_20: 12.86,
                pct_25: 10.22,
            },
        ),
        13 => (
            Some(StockYogoBiasRow {
                pct_5: 20.77,
                pct_10: 12.81,
                pct_20: 8.23,
                pct_30: 6.50,
            }),
            StockYogoSizeRow {
                pct_10: 34.62,
                pct_15: 18.84,
                pct_20: 13.45,
                pct_25: 10.68,
            },
        ),
        14 => (
            Some(StockYogoBiasRow {
                pct_5: 21.81,
                pct_10: 13.36,
                pct_20: 8.49,
                pct_30: 6.67,
            }),
            StockYogoSizeRow {
                pct_10: 36.36,
                pct_15: 19.72,
                pct_20: 14.05,
                pct_25: 11.13,
            },
        ),
        15 => (
            Some(StockYogoBiasRow {
                pct_5: 22.84,
                pct_10: 13.90,
                pct_20: 8.75,
                pct_30: 6.83,
            }),
            StockYogoSizeRow {
                pct_10: 38.08,
                pct_15: 20.60,
                pct_20: 14.65,
                pct_25: 11.58,
            },
        ),
        16 => (
            Some(StockYogoBiasRow {
                pct_5: 23.87,
                pct_10: 14.44,
                pct_20: 9.00,
                pct_30: 6.99,
            }),
            StockYogoSizeRow {
                pct_10: 39.80,
                pct_15: 21.48,
                pct_20: 15.24,
                pct_25: 12.03,
            },
        ),
        17 => (
            Some(StockYogoBiasRow {
                pct_5: 24.89,
                pct_10: 14.97,
                pct_20: 9.25,
                pct_30: 7.15,
            }),
            StockYogoSizeRow {
                pct_10: 41.51,
                pct_15: 22.35,
                pct_20: 15.83,
                pct_25: 12.49,
            },
        ),
        18 => (
            Some(StockYogoBiasRow {
                pct_5: 25.91,
                pct_10: 15.50,
                pct_20: 9.49,
                pct_30: 7.30,
            }),
            StockYogoSizeRow {
                pct_10: 43.22,
                pct_15: 23.22,
                pct_20: 16.42,
                pct_25: 12.94,
            },
        ),
        19 => (
            Some(StockYogoBiasRow {
                pct_5: 26.92,
                pct_10: 16.02,
                pct_20: 9.73,
                pct_30: 7.45,
            }),
            StockYogoSizeRow {
                pct_10: 44.92,
                pct_15: 24.09,
                pct_20: 17.02,
                pct_25: 13.39,
            },
        ),
        20 => (
            Some(StockYogoBiasRow {
                pct_5: 27.93,
                pct_10: 16.54,
                pct_20: 9.97,
                pct_30: 7.60,
            }),
            StockYogoSizeRow {
                pct_10: 46.62,
                pct_15: 24.96,
                pct_20: 17.61,
                pct_25: 13.84,
            },
        ),
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias, size })
}

/// Stock-Yogo (2005) LIML size of nominal 5% Wald test. k2=排除工具数。
/// 来源: ivreg2.ado cdsy type(limlsize10|15|20|25). LIML 无 bias 行。
fn stock_yogo_cv_liml_1_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let size = match k2 {
        1 => StockYogoSizeRow {
            pct_10: 16.38,
            pct_15: 8.96,
            pct_20: 6.66,
            pct_25: 5.53,
        },
        2 => StockYogoSizeRow {
            pct_10: 8.68,
            pct_15: 5.33,
            pct_20: 4.42,
            pct_25: 3.92,
        },
        3 => StockYogoSizeRow {
            pct_10: 6.46,
            pct_15: 4.36,
            pct_20: 3.69,
            pct_25: 3.32,
        },
        4 => StockYogoSizeRow {
            pct_10: 5.44,
            pct_15: 3.87,
            pct_20: 3.30,
            pct_25: 2.98,
        },
        5 => StockYogoSizeRow {
            pct_10: 4.84,
            pct_15: 3.56,
            pct_20: 3.05,
            pct_25: 2.77,
        },
        6 => StockYogoSizeRow {
            pct_10: 4.45,
            pct_15: 3.34,
            pct_20: 2.87,
            pct_25: 2.61,
        },
        7 => StockYogoSizeRow {
            pct_10: 4.18,
            pct_15: 3.18,
            pct_20: 2.73,
            pct_25: 2.49,
        },
        8 => StockYogoSizeRow {
            pct_10: 3.97,
            pct_15: 3.04,
            pct_20: 2.63,
            pct_25: 2.39,
        },
        9 => StockYogoSizeRow {
            pct_10: 3.81,
            pct_15: 2.93,
            pct_20: 2.54,
            pct_25: 2.32,
        },
        10 => StockYogoSizeRow {
            pct_10: 3.68,
            pct_15: 2.84,
            pct_20: 2.46,
            pct_25: 2.25,
        },
        11 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.76,
            pct_20: 2.40,
            pct_25: 2.19,
        },
        12 => StockYogoSizeRow {
            pct_10: 3.50,
            pct_15: 2.69,
            pct_20: 2.34,
            pct_25: 2.14,
        },
        13 => StockYogoSizeRow {
            pct_10: 3.42,
            pct_15: 2.63,
            pct_20: 2.29,
            pct_25: 2.10,
        },
        14 => StockYogoSizeRow {
            pct_10: 3.36,
            pct_15: 2.57,
            pct_20: 2.25,
            pct_25: 2.06,
        },
        15 => StockYogoSizeRow {
            pct_10: 3.31,
            pct_15: 2.52,
            pct_20: 2.21,
            pct_25: 2.03,
        },
        16 => StockYogoSizeRow {
            pct_10: 3.27,
            pct_15: 2.48,
            pct_20: 2.18,
            pct_25: 2.00,
        },
        17 => StockYogoSizeRow {
            pct_10: 3.24,
            pct_15: 2.44,
            pct_20: 2.14,
            pct_25: 1.97,
        },
        18 => StockYogoSizeRow {
            pct_10: 3.20,
            pct_15: 2.41,
            pct_20: 2.11,
            pct_25: 1.94,
        },
        19 => StockYogoSizeRow {
            pct_10: 3.18,
            pct_15: 2.37,
            pct_20: 2.09,
            pct_25: 1.92,
        },
        20 => StockYogoSizeRow {
            pct_10: 3.21,
            pct_15: 2.34,
            pct_20: 2.06,
            pct_25: 1.90,
        },
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias: None, size })
}

/// Stock-Yogo (2005) LIML size of nominal 5% Wald test，2 内生变量。
fn stock_yogo_cv_liml_2_endog(k2: usize) -> Option<StockYogoCriticalValues> {
    let size = match k2 {
        2 => StockYogoSizeRow {
            pct_10: 7.03,
            pct_15: 4.58,
            pct_20: 3.95,
            pct_25: 3.63,
        },
        3 => StockYogoSizeRow {
            pct_10: 5.44,
            pct_15: 3.81,
            pct_20: 3.32,
            pct_25: 3.09,
        },
        4 => StockYogoSizeRow {
            pct_10: 4.72,
            pct_15: 3.39,
            pct_20: 2.99,
            pct_25: 2.79,
        },
        5 => StockYogoSizeRow {
            pct_10: 4.32,
            pct_15: 3.13,
            pct_20: 2.78,
            pct_25: 2.60,
        },
        6 => StockYogoSizeRow {
            pct_10: 4.06,
            pct_15: 2.95,
            pct_20: 2.63,
            pct_25: 2.46,
        },
        7 => StockYogoSizeRow {
            pct_10: 3.90,
            pct_15: 2.83,
            pct_20: 2.52,
            pct_25: 2.35,
        },
        8 => StockYogoSizeRow {
            pct_10: 3.78,
            pct_15: 2.73,
            pct_20: 2.43,
            pct_25: 2.27,
        },
        9 => StockYogoSizeRow {
            pct_10: 3.70,
            pct_15: 2.66,
            pct_20: 2.36,
            pct_25: 2.20,
        },
        10 => StockYogoSizeRow {
            pct_10: 3.64,
            pct_15: 2.60,
            pct_20: 2.30,
            pct_25: 2.14,
        },
        11 => StockYogoSizeRow {
            pct_10: 3.60,
            pct_15: 2.55,
            pct_20: 2.25,
            pct_25: 2.09,
        },
        12 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.52,
            pct_20: 2.21,
            pct_25: 2.05,
        },
        13 => StockYogoSizeRow {
            pct_10: 3.56,
            pct_15: 2.48,
            pct_20: 2.17,
            pct_25: 2.02,
        },
        14 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.46,
            pct_20: 2.14,
            pct_25: 1.99,
        },
        15 => StockYogoSizeRow {
            pct_10: 3.54,
            pct_15: 2.44,
            pct_20: 2.11,
            pct_25: 1.96,
        },
        16 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.42,
            pct_20: 2.09,
            pct_25: 1.93,
        },
        17 => StockYogoSizeRow {
            pct_10: 3.55,
            pct_15: 2.41,
            pct_20: 2.07,
            pct_25: 1.91,
        },
        18 => StockYogoSizeRow {
            pct_10: 3.56,
            pct_15: 2.40,
            pct_20: 2.05,
            pct_25: 1.89,
        },
        19 => StockYogoSizeRow {
            pct_10: 3.57,
            pct_15: 2.39,
            pct_20: 2.03,
            pct_25: 1.87,
        },
        20 => StockYogoSizeRow {
            pct_10: 3.58,
            pct_15: 2.38,
            pct_20: 2.02,
            pct_25: 1.86,
        },
        _ => return None,
    };
    Some(StockYogoCriticalValues { bias: None, size })
}

impl IV2SLS {
    pub fn fit(&self) -> Result<IV2SLSResult, String> {
        let n = self.endog.len();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IV2SLS: underidentified — {} instruments < {} endogenous. Need at least {} instruments.",
                k_iv, k_endog, k_endog
            ));
        }

        // Z = [exog, instruments] for stage 1 (with constant if config.constant)
        let k_z = if self.config.constant {
            k_exog + k_iv + 1
        } else {
            k_exog + k_iv
        };
        let mut z_raw = Vec::with_capacity(n * k_z);
        for i in 0..n {
            if self.config.constant {
                z_raw.push(1.0);
            }
            for j in 0..k_exog {
                z_raw.push(self.exog[[i, j]]);
            }
            for j in 0..k_iv {
                z_raw.push(self.instruments[[i, j]]);
            }
        }
        let z = Array2::from_shape_vec((n, k_z), z_raw)
            .map_err(|e| format!("IV2SLS: failed to build Z: {}", e))?;

        // Stage 1: endog_hat = Z * (Z'Z)^{-1} Z' * endog for each endogenous
        let z_faer = z.view().into_faer().to_owned();
        let ztz = z_faer.transpose() * z_faer.as_ref();
        let ztz_inv = ztz
            .llt(Side::Lower)
            .map_err(|_| {
                "IV2SLS: Z'Z is not positive definite (stage 1). Check instruments and exog for collinearity.".to_string()
            })?
            .solve(Mat::identity(ztz.nrows(), ztz.ncols()));

        let ztz_inv_nd = ztz_inv.as_ref().into_ndarray().to_owned();
        let df_z = n.saturating_sub(k_z);

        let mut endog_hat = Array2::zeros((n, k_endog));
        let mut first_stage: Vec<FirstStageResult> = Vec::with_capacity(k_endog);
        for j in 0..k_endog {
            let endog_col = self.endog_reg.column(j).into_owned();
            let endog_faer = endog_col.view().into_faer_col().to_owned();
            let zty = z_faer.transpose() * endog_faer.as_ref();
            let gamma = ztz_inv.as_ref() * zty;
            let hat = z_faer.as_ref() * gamma.as_ref();
            let hat_arr = hat.as_ref().into_ndarray().to_owned();
            for i in 0..n {
                endog_hat[[i, j]] = hat_arr[i];
            }

            // First-stage stats: resid, r2, cov_gamma, stds, t, p
            let resid = &endog_col - &hat_arr;
            let ss_resid = resid.iter().map(|v| v.powi(2)).sum::<f64>();
            let y_mean = endog_col.iter().mean();
            let ss_tot = endog_col.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>();
            let r2 = if ss_tot > 1e-300 {
                1.0 - ss_resid / ss_tot
            } else {
                0.0
            };
            let ms_resid = if df_z > 0 {
                ss_resid / df_z as f64
            } else {
                0.0
            };
            let ms_tot = if n > 1 { ss_tot / (n - 1) as f64 } else { 0.0 };
            let r2_adj = if ms_tot > 1e-300 {
                1.0 - ms_resid / ms_tot
            } else {
                0.0
            };

            let sigma2 = if df_z > 0 {
                (ss_resid / df_z as f64).max(1e-300)
            } else {
                1e-300
            };
            let cov_gamma = sigma2 * &ztz_inv_nd;
            let stds: Vec<f64> = (0..k_z).map(|i| cov_gamma[[i, i]].sqrt()).collect();
            let gamma_nd = gamma.as_ref().into_ndarray().to_owned();
            let t_dist = StudentsT::new(0.0, 1.0, df_z as f64)
                .unwrap_or(StudentsT::new(0.0, 1.0, 1.0).unwrap());
            let t_values: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] / stds[i]).collect();
            let p_values: Vec<f64> = t_values
                .iter()
                .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
                .collect();
            let t_crit = t_dist.inverse_cdf(0.975);
            let ci_left: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] - t_crit * stds[i]).collect();
            let ci_right: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] + t_crit * stds[i]).collect();

            let name = self
                .endog_names
                .as_ref()
                .and_then(|n| n.get(j))
                .cloned()
                .unwrap_or_else(|| format!("endog_{}", j + 1));
            let var_names: Vec<String> = (0..k_z)
                .map(|i| {
                    self.z_var_names
                        .as_ref()
                        .and_then(|v| v.get(i).cloned())
                        .unwrap_or_else(|| format!("z{}", i + 1))
                })
                .collect();
            first_stage.push(FirstStageResult {
                endog_name: name,
                var_names,
                betas: gamma_nd.to_vec(),
                stds,
                tvalues: t_values,
                pvalues: p_values,
                conf_int_left: ci_left,
                conf_int_right: ci_right,
                r2,
                r2_adjusted: r2_adj,
            });
        }

        // Stage 2: X = [exog, endog_hat] (with constant)
        let k_x = if self.config.constant {
            k_exog + k_endog + 1
        } else {
            k_exog + k_endog
        };
        let mut x_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.config.constant {
                x_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_raw.push(self.exog[[i, j]]);
            }
            for j in 0..k_endog {
                x_raw.push(endog_hat[[i, j]]);
            }
        }
        let x = Array2::from_shape_vec((n, k_x), x_raw)
            .map_err(|e| format!("IV2SLS: failed to build X: {}", e))?;

        let (rank, cond_no) = matrix_rank(x.view().into_faer().to_owned());
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        // OLS on second stage: β = (X'X)^{-1} X'y
        let x_faer = x.view().into_faer().to_owned();
        let y_faer = self.endog.view().into_faer_col().to_owned();
        let xtx = x_faer.transpose() * x_faer.as_ref();
        let xty = x_faer.transpose() * y_faer.as_ref();
        let xtx_inv = xtx
            .llt(Side::Lower)
            .map_err(|_| {
                "IV2SLS: X'X is not positive definite (stage 2). Check for collinearity."
                    .to_string()
            })?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let betas_faer = xtx_inv.as_ref() * xty;
        let betas_nd = betas_faer.as_ref().into_ndarray().to_owned();

        // ESS and VCE must use structural residuals: u = y - X_struct * β
        // where X_struct = [exog, endog] (actual endogenous, not endog_hat).
        // Stata: ESS = y'y - 2β'X'y + β'X'Xβ, σ² = ESS/(n-k), VCE = σ² (X'P_Z X)^{-1}.
        let mut x_struct_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.config.constant {
                x_struct_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_struct_raw.push(self.exog[[i, j]]);
            }
            for j in 0..k_endog {
                x_struct_raw.push(self.endog_reg[[i, j]]);
            }
        }
        let x_struct = Array2::from_shape_vec((n, k_x), x_struct_raw)
            .map_err(|e| format!("IV2SLS: failed to build X_struct: {}", e))?;
        let u_structural: Array1<f64> = &self.endog - &x_struct.dot(&betas_nd);

        let y_mean = y_faer.iter().mean();
        let ss_total = if self.config.constant {
            y_faer.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            y_faer.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = u_structural.dot(&u_structural);
        let ss_model = ss_total - ss_residual;
        let r2 = if ss_total > 1e-300 {
            1.0 - ss_residual / ss_total
        } else {
            0.0
        };

        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = if ms_total > 1e-300 {
            1.0 - ms_residual / ms_total
        } else {
            0.0
        };

        let x_nd = x_faer.as_ref().into_ndarray().to_owned();
        let xtx_inv_nd = xtx_inv.as_ref().into_ndarray().to_owned();

        // Stata: s² = ESS/(n-k) if small, else ESS/n. Affects VCE and robust scale.
        let sigma2_df = if self.config.small { df_residual } else { n };

        let cov_beta = compute_cov_beta(
            &x_nd,
            &xtx_inv_nd,
            &u_structural,
            sigma2_df,
            &covariance_type,
            self.config.cov_params.as_ref(),
        )?;

        let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);
        // 2SLS uses asymptotic inference: z = coef/se ~ N(0,1), not t
        let z_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| if *se > 1e-300 { b / se } else { 0.0 })
            .collect();

        let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("IV2SLS: {}", e))?;
        let p_values: Vec<f64> = z_values
            .iter()
            .map(|&z| 2.0 * (1.0 - std_normal.cdf(z.abs())))
            .collect();

        let z_crit = std_normal.inverse_cdf(0.975);
        let ci_lower = &betas_nd - z_crit * &std_err;
        let ci_upper = &betas_nd + z_crit * &std_err;

        // Wald chi2 for joint significance (2SLS uses chi2, not F). Stata Methods: "If c=1 and small is not
        // specified, a Wald statistic W of the joint significance of the k−1 parameters of β except the
        // constant term is calculated; W ∼ χ²(k−1)." W = β_s' V_s^{-1} β_s. Use solve(V_s, β_s) for stability.
        let k = betas_nd.len();
        let (wald_chi2, wald_p) = {
            let (beta_s, v_s, df_wald) = if self.config.constant && k > 1 {
                // Exclude constant (index 0). Our X = [const, exog, endog_hat], so const is always first.
                let beta_s = betas_nd.slice(ndarray::s![1..]).to_owned();
                let v_s = cov_beta.slice(ndarray::s![1.., 1..]).to_owned();
                (beta_s, v_s, k - 1)
            } else {
                let beta_s = betas_nd.clone();
                let v_s = cov_beta.clone();
                (beta_s, v_s, k)
            };
            let v_s_faer = v_s.view().into_faer().to_owned();
            let beta_s_faer = beta_s.view().into_faer_col().to_owned();
            // Solve V_s * x = beta_s => x = V_s^{-1} * beta_s; then wald = beta_s' * x (more stable than explicit inverse)
            let x = v_s_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "IV2SLS: V_s not pd for Wald".to_string())?
                .solve(beta_s_faer.as_ref());
            let x_nd = x.as_ref().into_ndarray();
            let wald = beta_s.dot(&x_nd);
            let chi2_dist =
                ChiSquared::new(df_wald as f64).map_err(|e| format!("IV2SLS Wald: {}", e))?;
            let wald_p = 1.0 - chi2_dist.cdf(wald);
            (wald, wald_p)
        };

        // estat firststage: First-stage regression summary statistics
        let first_stage_summary = compute_first_stage_summary(
            &z,
            &endog_hat,
            &self.endog_reg,
            &self.exog,
            &self.instruments,
            n,
            k_z,
            k_exog,
            k_iv,
            k_endog,
            self.config.constant,
            &covariance_type,
            self.config.cov_params.as_ref(),
            self.config.small,
            false, // for_liml
        )?;

        // Overidentification test (estat overid): Sargan/Basmann (homoskedastic) or Wooldridge (1995) robust score (robust VCE).
        // Stata: "If you used the 2SLS estimator and requested a robust VCE, Wooldridge's robust score test of
        // overidentifying restrictions is performed instead; without a robust VCE, Wooldridge's test statistic is identical to Sargan's."
        let overid = if k_iv > k_endog {
            let df_overid = k_iv - k_endog;
            let chi2_dist = ChiSquared::new(df_overid as f64)
                .map_err(|e| format!("IV2SLS overid ChiSquared: {}", e))?;

            let is_robust = is_robust_cov_type(&covariance_type);

            if is_robust {
                // Wooldridge (1995) robust score test. Stata Methods: Let Ŷ = endog_hat, Q = excluded instruments (m cols).
                // q̂_j = residuals from regressing jth column of Q on [X1, Ŷ]. k̂_ij = q̂_ij * û_i.
                // Regress 1 on [k̂_1,...,k̂_m]: W = N - RSS ~ χ²(m).
                let m = df_overid;
                let w_mat = &x; // W = [X1, Ŷ] = [const?, exog, endog_hat]
                let wtw = w_mat.t().dot(w_mat);
                let wtw_inv = wtw
                    .view()
                    .into_faer()
                    .to_owned()
                    .llt(Side::Lower)
                    .map_err(|_| "IV2SLS Wooldridge overid: W'W not positive definite".to_string())?
                    .solve(Mat::identity(wtw.nrows(), wtw.ncols()));
                let wtw_inv_nd = wtw_inv.as_ref().into_ndarray().to_owned();

                // Build K: n × m, columns k̂_j = (Q_j - W*γ_j) .* u, where γ_j = (W'W)^{-1} W' Q_j
                let mut k_mat = Array2::zeros((n, m));
                for j in 0..m {
                    let q_j = self.instruments.column(j).into_owned();
                    let wtq = w_mat.t().dot(&q_j);
                    let gamma_j = wtw_inv_nd.dot(&wtq);
                    let q_hat = w_mat.dot(&gamma_j); // fitted = W * γ
                    let q_resid = &q_j - &q_hat; // q̂_j = residuals
                    for i in 0..n {
                        k_mat[[i, j]] = q_resid[i] * u_structural[i];
                    }
                }

                // Regress 1 on K: 1 = K*θ + ε. RSS = (1 - K*θ)^2. W = N - RSS.
                let ones = Array1::from_elem(n, 1.0);
                let ktk = k_mat.t().dot(&k_mat);
                let kt1 = k_mat.t().dot(&ones);
                let ktk_inv = ktk
                    .view()
                    .into_faer()
                    .to_owned()
                    .llt(Side::Lower)
                    .map_err(|_| "IV2SLS Wooldridge overid: K'K not positive definite".to_string())?
                    .solve(Mat::identity(ktk.nrows(), ktk.ncols()));
                let theta = ktk_inv.as_ref().into_ndarray().to_owned().dot(&kt1);
                let fitted = k_mat.dot(&theta);
                let rss: f64 = ones
                    .iter()
                    .zip(fitted.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                let wooldridge_stat = n as f64 - rss;
                let wooldridge_p = 1.0 - chi2_dist.cdf(wooldridge_stat);
                Some(OveridTest {
                    test_type: "wooldridge".to_string(),
                    sargan_stat: None,
                    sargan_p_value: None,
                    basmann_stat: None,
                    basmann_p_value: None,
                    wooldridge_stat: Some(wooldridge_stat),
                    wooldridge_p_value: Some(wooldridge_p),
                    df: df_overid,
                })
            } else {
                // Sargan & Basmann (homoskedastic)
                let uu = u_structural.dot(&u_structural);
                if uu > 1e-300 {
                    let ztu = z.t().dot(&u_structural);
                    let ztz_inv_ztu = ztz_inv_nd.dot(&ztu);
                    let u_pz_u = ztu.dot(&ztz_inv_ztu);
                    let sargan_stat = n as f64 * u_pz_u / uu;
                    let basmann_stat = if (n as f64 - sargan_stat).abs() > 1e-10 {
                        sargan_stat * (n as f64 - k_z as f64) / (n as f64 - sargan_stat)
                    } else {
                        sargan_stat
                    };
                    let sargan_p = 1.0 - chi2_dist.cdf(sargan_stat);
                    let basmann_p = 1.0 - chi2_dist.cdf(basmann_stat);
                    Some(OveridTest {
                        test_type: "sargan_basmann".to_string(),
                        sargan_stat: Some(sargan_stat),
                        sargan_p_value: Some(sargan_p),
                        basmann_stat: Some(basmann_stat),
                        basmann_p_value: Some(basmann_p),
                        wooldridge_stat: None,
                        wooldridge_p_value: None,
                        df: df_overid,
                    })
                } else {
                    None
                }
            }
        } else {
            None
        };

        // Hausman tests (traditional + Durbin-Wu-Hausman): only for nonrobust VCE
        let (hausman, endogenous) = if !is_robust_cov_type(&covariance_type) {
            // OLS on y ~ X_struct (treating endog as exogenous): β_ols, u_ols
            let x_struct_tx = x_struct.t().dot(&x_struct);
            let x_struct_tx_inv: Option<faer::Mat<f64>> = x_struct_tx
                .view()
                .into_faer()
                .to_owned()
                .llt(Side::Lower)
                .ok()
                .map(|llt| llt.solve(Mat::identity(x_struct_tx.nrows(), x_struct_tx.ncols())));
            let (beta_ols, u_ols, sigma2_ols, xtx_struct_inv_nd) =
                if let Some(ref inv) = x_struct_tx_inv {
                    let inv_nd = inv.as_ref().into_ndarray().to_owned();
                    let xty_struct = x_struct.t().dot(&self.endog);
                    let beta_ols_nd = inv_nd.dot(&xty_struct);
                    let u_ols: Array1<f64> = &self.endog - &x_struct.dot(&beta_ols_nd);
                    let sigma2_ols = u_ols.dot(&u_ols) / df_residual as f64;
                    (beta_ols_nd, u_ols, sigma2_ols, inv_nd)
                } else {
                    (
                        Array1::<f64>::zeros(k_x),
                        Array1::<f64>::zeros(n),
                        0.0,
                        Array2::<f64>::zeros((k_x, k_x)),
                    )
                };

            // Traditional Hausman (sigmamore): H = (β_iv - β_ols)'(V_iv - V_ols)^{-1}(β_iv - β_ols)
            // V_iv = σ²_ols * (X̂'X̂)^{-1}, V_ols = σ²_ols * (X_struct'X_struct)^{-1}
            let hausman = if sigma2_ols > 1e-300 {
                let v_iv = sigma2_ols * &xtx_inv_nd; // X̂'X̂ from stage 2
                let v_ols = sigma2_ols * &xtx_struct_inv_nd;
                let v_diff: Array2<f64> = &v_iv - &v_ols;
                let diff_beta = &betas_nd - &beta_ols;
                let v_diff_faer = v_diff.view().into_faer().to_owned();
                let svd = v_diff_faer.as_ref().svd().ok();
                let (h_stat, h_df) = if let Some(svd) = svd {
                    let s = svd.S().column_vector();
                    let u = svd.U();
                    let v = svd.V();
                    let max_s = s.iter().cloned().fold(0.0f64, f64::max);
                    let tol = max_s * (k_x as f64) * f64::EPSILON;
                    let rank = s.iter().filter(|&&si| si > tol).count();
                    if rank == 0 {
                        (0.0, 0)
                    } else {
                        // H = diff' * V_diff^{-} * diff via SVD: V_diff = U S V', inv = V S^{-1} U' (Moore-Penrose)
                        let diff_col = diff_beta.view().into_faer_col().to_owned();
                        let ut_diff = u.get(.., ..k_x).transpose() * diff_col.as_ref();
                        let ut_diff_nd = ut_diff.as_ref().into_ndarray().to_owned();
                        let mut st_inv_ut_diff = Mat::zeros(k_x, 1);
                        for i in 0..k_x {
                            let si = s[i];
                            let val = if si > tol { ut_diff_nd[i] / si } else { 0.0 };
                            st_inv_ut_diff.as_mut()[(i, 0)] = val;
                        }
                        let vinv_diff = v.get(.., ..k_x) * st_inv_ut_diff.as_ref();
                        let h: f64 = diff_beta.dot(&vinv_diff.as_ref().into_ndarray().column(0));
                        (h.max(0.0), rank)
                    }
                } else {
                    (0.0, 0)
                };
                let chi2_h = ChiSquared::new(h_df as f64).ok();
                let p_val = chi2_h.map(|c| 1.0 - c.cdf(h_stat)).unwrap_or(f64::NAN);
                Some(HausmanTest {
                    stat: h_stat,
                    p_value: p_val,
                    df: h_df,
                })
            } else {
                None
            };

            // Durbin-Wu-Hausman (estat endogenous): D = num/(û'ₑ ûₑ/N), WH = (num/p1)/(denom/(N-k1-p-p1))
            // ûₗ = u_structural, ûₑ = u_ols; P_Z = Z(Z'Z)^{-1}Z'; P_{ZY1} = [Z Y1]([Z Y1]'[Z Y1])^{-1}[Z Y1]'
            // Testing all endog: Y1 = Y, [Z Y1] = [Z endog_reg]
            let endogenous = if sigma2_ols > 1e-300 && u_ols.dot(&u_ols) > 1e-300 {
                let p1 = k_endog;
                let k1 = if self.config.constant {
                    k_exog + 1
                } else {
                    k_exog
                };
                let wudf_denom = n
                    .saturating_sub(k1)
                    .saturating_sub(k_endog)
                    .saturating_sub(p1);

                // Build [Z Y1] = [Z, endog_reg] = [exog, instruments, endog_reg] with constant
                let mut zy1_raw = Vec::with_capacity(n * (k_z + k_endog));
                for i in 0..n {
                    for j in 0..k_z {
                        zy1_raw.push(z[[i, j]]);
                    }
                    for j in 0..k_endog {
                        zy1_raw.push(self.endog_reg[[i, j]]);
                    }
                }
                let zy1 = Array2::from_shape_vec((n, k_z + k_endog), zy1_raw)
                    .unwrap_or_else(|_| Array2::zeros((n, (k_z + k_endog).max(1))));
                let zy1_faer = zy1.view().into_faer().to_owned();
                let zy1t_zy1 = zy1_faer.transpose() * zy1_faer.as_ref();
                let zy1t_zy1_inv: Option<faer::Mat<f64>> = zy1t_zy1
                    .llt(Side::Lower)
                    .ok()
                    .map(|llt| llt.solve(Mat::identity(zy1t_zy1.nrows(), zy1t_zy1.ncols())));

                let (num, u_ols_sq) = if let Some(zy1_inv) = zy1t_zy1_inv {
                    let zy1_inv_nd = zy1_inv.as_ref().into_ndarray().to_owned();
                    let p_zy1_u_ols = zy1.dot(&zy1_inv_nd.dot(&zy1.t().dot(&u_ols)));
                    let p_z_u_iv = z.dot(&ztz_inv_nd.dot(&z.t().dot(&u_structural)));
                    let num = u_ols.dot(&p_zy1_u_ols) - u_structural.dot(&p_z_u_iv);
                    let u_ols_sq = u_ols.dot(&u_ols);
                    (num, u_ols_sq)
                } else {
                    (0.0, u_ols.dot(&u_ols))
                };

                let denom = u_ols_sq - num;
                let durbin_stat: f64 = if u_ols_sq > 1e-300 {
                    n as f64 * num / u_ols_sq
                } else {
                    0.0
                };
                let durbin_stat = durbin_stat.max(0.0);
                let chi2_d = ChiSquared::new(p1 as f64).ok();
                let durbin_p = chi2_d.map(|c| 1.0 - c.cdf(durbin_stat)).unwrap_or(f64::NAN);

                let wu_stat: f64 = if wudf_denom > 0 && denom > 1e-300 {
                    ((num / p1 as f64) / (denom / wudf_denom as f64)).max(0.0)
                } else {
                    0.0
                };
                let f_dist = FisherSnedecor::new(p1 as f64, wudf_denom as f64).ok();
                let wu_p = f_dist.map(|f| 1.0 - f.cdf(wu_stat)).unwrap_or(f64::NAN);

                Some(EndogenousTest {
                    durbin_stat,
                    durbin_p_value: durbin_p,
                    wu_stat,
                    wu_p_value: wu_p,
                    df: p1,
                    wu_df_denom: wudf_denom,
                })
            } else {
                None
            };

            (hausman, endogenous)
        } else {
            (None, None)
        };

        Ok(IV2SLSResult {
            num_observation: n,
            ss_model,
            ss_residual,
            ss_total,
            df_model,
            df_residual,
            df_total,
            ms_model,
            ms_residual,
            ms_total,
            covariance_type,
            r2,
            r2_adjusted,
            wald_chi2,
            wald_chi2_p_value: wald_p,
            model: IV2SLSModel {
                params: betas_nd.clone(),
            },
            betas: betas_nd,
            stds: std_err,
            zvalues: Array1::from_vec(z_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
            first_stage,
            first_stage_summary,
            overid,
            overid_k_iv: k_iv,
            overid_k_endog: k_endog,
            hausman,
            endogenous,
        })
    }
}
