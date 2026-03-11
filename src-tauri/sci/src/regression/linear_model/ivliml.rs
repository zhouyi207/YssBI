//! IV:LIML (Limited Information Maximum Likelihood)
//!
//! Stata ivregress liml: depvar [varlist1] (varlist2 = varlistiv)
//! κ-class estimator with κ = minimum eigenvalue of (Ỹ'MZ Ỹ)^{-1/2} Ỹ'MX1 Ỹ (Ỹ'MZ Ỹ)^{-1/2}
//! β̂ = {X'(I − κMZ)X}^{-1} X'(I − κMZ)y

use crate::regression::covariance::{compute_cov_beta, CovParams};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor, Normal};
use statrs::statistics::Statistics;

/// LIML 配置（与 2SLS 一致）
pub struct IVLIMLConfig {
    pub constant: bool,
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
    pub small: bool,
}

/// IV:LIML 输入（与 IV2SLS 相同结构）
pub struct IVLIML {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub endog_reg: Array2<f64>,
    pub instruments: Array2<f64>,
    pub config: IVLIMLConfig,
    pub endog_names: Option<Vec<String>>,
    pub z_var_names: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct IVLIMLModel {
    pub params: Array1<f64>,
}

/// LIML 结果（与 IV2SLS 类似，复用 FirstStageResult/FirstStageSummary 等）
#[derive(Debug)]
pub struct IVLIMLResult {
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
    pub wald_chi2: f64,
    pub wald_chi2_p_value: f64,

    pub model: IVLIMLModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub zvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,

    /// κ used in LIML
    pub kappa: f64,

    /// 第一阶段回归（与 2SLS 相同）
    pub first_stage: Vec<super::iv2sls::FirstStageResult>,
    pub first_stage_summary: super::iv2sls::FirstStageSummary,

    pub overid_k_iv: usize,
    pub overid_k_endog: usize,

    /// Overidentification test (estat overid): Anderson-Rubin chi2, Basmann F. Only when k_iv > k_endog and nonrobust.
    pub overid: Option<LimlOveridTest>,
}

/// LIML overidentification test (Stata estat overid)
/// Anderson-Rubin (1950) chi2, Basmann F
#[derive(Debug, Clone)]
pub struct LimlOveridTest {
    pub anderson_rubin_stat: f64,
    pub anderson_rubin_p_value: f64,
    pub basmann_stat: f64,
    pub basmann_p_value: f64,
    pub df: usize,
    pub df_denom: usize,
}

fn is_robust_cov_type(cov_type: &str) -> bool {
    matches!(
        cov_type,
        "HC0" | "HC1" | "HC2" | "HC3" | "cluster" | "HAC" | "newey"
    )
}

impl IVLIML {
    pub fn fit(&self) -> Result<IVLIMLResult, String> {
        let n = self.endog.len();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IVLIML: underidentified — {} instruments < {} endogenous.",
                k_iv, k_endog
            ));
        }

        let k_z = if self.config.constant {
            k_exog + k_iv + 1
        } else {
            k_exog + k_iv
        };
        let k1 = if self.config.constant { k_exog + 1 } else { k_exog };

        // Z = [const?, exog, instruments]
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
            .map_err(|e| format!("IVLIML: failed to build Z: {}", e))?;

        // X1 = [const?, exog]
        let mut x1_raw = Vec::with_capacity(n * k1);
        for i in 0..n {
            if self.config.constant {
                x1_raw.push(1.0);
            }
            for j in 0..k_exog {
                x1_raw.push(self.exog[[i, j]]);
            }
        }
        let x1 = Array2::from_shape_vec((n, k1), x1_raw)
            .map_err(|e| format!("IVLIML: failed to build X1: {}", e))?;

        // X = [const?, exog, endog_reg] (structural)
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
                x_raw.push(self.endog_reg[[i, j]]);
            }
        }
        let x = Array2::from_shape_vec((n, k_x), x_raw)
            .map_err(|e| format!("IVLIML: failed to build X: {}", e))?;

        // Ỹ = [y Y] (n × (p+1))
        let mut y_tilde_raw = Vec::with_capacity(n * (k_endog + 1));
        for i in 0..n {
            y_tilde_raw.push(self.endog[i]);
            for j in 0..k_endog {
                y_tilde_raw.push(self.endog_reg[[i, j]]);
            }
        }
        let y_tilde = Array2::from_shape_vec((n, k_endog + 1), y_tilde_raw)
            .map_err(|e| format!("IVLIML: failed to build Ỹ: {}", e))?;

        let z_faer = z.view().into_faer().to_owned();
        let x1_faer = x1.view().into_faer().to_owned();
        let y_tilde_faer = y_tilde.view().into_faer().to_owned();
        let x_faer = x.view().into_faer().to_owned();
        let y_faer = self.endog.view().into_faer_col().to_owned();

        // Z'Z, (Z'Z)^{-1}
        let ztz = z_faer.transpose() * z_faer.as_ref();
        let ztz_inv = ztz
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "IVLIML: Z'Z not pd".to_string())?
            .solve(Mat::identity(ztz.nrows(), ztz.ncols()));
        let ztz_inv_nd = ztz_inv.as_ref().into_ndarray().to_owned();

        // X1'X1, (X1'X1)^{-1}
        let x1tx1 = x1_faer.transpose() * x1_faer.as_ref();
        let x1tx1_inv = x1tx1
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "IVLIML: X1'X1 not pd".to_string())?
            .solve(Mat::identity(x1tx1.nrows(), x1tx1.ncols()));
        let x1tx1_inv_nd = x1tx1_inv.as_ref().into_ndarray().to_owned();

        // Ỹ'MZ Ỹ = Ỹ'Ỹ - Ỹ'Z(Z'Z)^{-1}Z'Ỹ
        let yty = y_tilde_faer.transpose() * y_tilde_faer.as_ref();
        let zty = z_faer.transpose() * y_tilde_faer.as_ref();
        let ytmz = yty.as_ref().into_ndarray().to_owned();
        let zty_nd = zty.as_ref().into_ndarray().to_owned();
        let ytmz_nd: Array2<f64> = &ytmz - &zty_nd.t().dot(&ztz_inv_nd).dot(&zty_nd);

        // Ỹ'MX1 Ỹ = Ỹ'Ỹ - Ỹ'X1(X1'X1)^{-1}X1'Ỹ
        let x1ty = x1_faer.transpose() * y_tilde_faer.as_ref();
        let x1ty_nd = x1ty.as_ref().into_ndarray().to_owned();
        let ytmx1_nd: Array2<f64> = &ytmz - &x1ty_nd.t().dot(&x1tx1_inv_nd).dot(&x1ty_nd);

        // G = (Ỹ'MZ Ỹ)^{-1/2} Ỹ'MX1 Ỹ (Ỹ'MZ Ỹ)^{-1/2}
        let evd = faer::linalg::solvers::SelfAdjointEigen::new(
            ytmz_nd.view().into_faer(),
            Side::Lower,
        )
        .map_err(|_| "IVLIML: EVD of Ỹ'MZ Ỹ failed".to_string())?;
        let s_col = evd.S().column_vector();
        let u = evd.U();
        let size = k_endog + 1;
        let mut lambda_inv_sqrt = Mat::zeros(size, size);
        for i in 0..size {
            let si = s_col[i];
            if si > 1e-12 {
                lambda_inv_sqrt.as_mut()[(i, i)] = 1.0 / si.sqrt();
            }
        }
        let ytmz_inv_sqrt = u.as_ref() * lambda_inv_sqrt.as_ref() * u.transpose();
        let g = ytmz_inv_sqrt.as_ref() * (ytmx1_nd.view().into_faer().to_owned() * ytmz_inv_sqrt.as_ref());

        // κ = minimum eigenvalue of G
        let g_nd = g.as_ref().into_ndarray().to_owned();
        let evd_g = faer::linalg::solvers::SelfAdjointEigen::new(
            g_nd.view().into_faer(),
            Side::Lower,
        )
        .map_err(|_| "IVLIML: EVD of G failed".to_string())?;
        let s_g = evd_g.S().column_vector();
        let kappa = s_g
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(0.0);

        // β̂ = {X'(I − κMZ)X}^{-1} X'(I − κMZ)y
        // X'(I−κMZ)X = (1-κ)X'X + κ X'Z(Z'Z)^{-1}Z'X
        // X'(I−κMZ)y = (1-κ)X'y + κ X'Z(Z'Z)^{-1}Z'y
        let xtx = x_faer.transpose() * x_faer.as_ref();
        let xty = x_faer.transpose() * y_faer.as_ref();
        let xtz = x_faer.transpose() * z_faer.as_ref();
        let ztx = z_faer.transpose() * x_faer.as_ref();
        let zty_y = z_faer.transpose() * y_faer.as_ref();

        let xtx_nd = xtx.as_ref().into_ndarray().to_owned();
        let xty_nd = xty.as_ref().into_ndarray().to_owned();
        let xtz_nd = xtz.as_ref().into_ndarray().to_owned();
        let ztx_nd = ztx.as_ref().into_ndarray().to_owned();
        let zty_y_nd = zty_y.as_ref().into_ndarray().to_owned();

        let xt_ikmz_x_nd: Array2<f64> = (1.0 - kappa) * &xtx_nd
            + kappa * xtz_nd.dot(&ztz_inv_nd).dot(&ztx_nd);
        let xt_ikmz_y_nd: Array1<f64> = (1.0 - kappa) * &xty_nd
            + kappa * xtz_nd.dot(&ztz_inv_nd).dot(&zty_y_nd);

        let xt_ikmz_x_faer = xt_ikmz_x_nd.view().into_faer().to_owned();
        let xt_ikmz_y_faer = xt_ikmz_y_nd.view().into_faer_col().to_owned();
        let xt_ikmz_x_inv = xt_ikmz_x_faer
            .as_ref()
            .llt(Side::Lower)
            .map_err(|_| "IVLIML: X'(I−κMZ)X not pd".to_string())?
            .solve(Mat::identity(xt_ikmz_x_nd.nrows(), xt_ikmz_x_nd.ncols()));
        let betas_faer = xt_ikmz_x_inv.as_ref() * xt_ikmz_y_faer.as_ref();
        let betas_nd = betas_faer.as_ref().into_ndarray().to_owned();

        let (rank, cond_no) = matrix_rank(x.view().into_faer().to_owned());
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let u_structural: Array1<f64> = &self.endog - &x.dot(&betas_nd);
        let ss_residual = u_structural.dot(&u_structural);
        let y_mean = self.endog.iter().mean();
        let ss_total = if self.config.constant {
            self.endog.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            self.endog.iter().map(|v| v.powi(2)).sum::<f64>()
        };
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

        let sigma2_df = if self.config.small { df_residual } else { n };
        let xt_ikmz_x_inv_nd = xt_ikmz_x_inv.as_ref().into_ndarray().to_owned();
        let x_nd = x_faer.as_ref().into_ndarray().to_owned();

        let cov_beta = compute_cov_beta(
            &x_nd,
            &xt_ikmz_x_inv_nd,
            &u_structural,
            sigma2_df,
            &self.config.cov_type,
            self.config.cov_params.as_ref(),
        )?;

        let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);
        let z_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| if *se > 1e-300 { b / se } else { 0.0 })
            .collect();
        let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("IVLIML: {}", e))?;
        let p_values: Vec<f64> = z_values
            .iter()
            .map(|&z| 2.0 * (1.0 - std_normal.cdf(z.abs())))
            .collect();
        let z_crit = std_normal.inverse_cdf(0.975);
        let ci_lower = &betas_nd - z_crit * &std_err;
        let ci_upper = &betas_nd + z_crit * &std_err;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        let k = betas_nd.len();
        let (wald_chi2, wald_p) = {
            let (beta_s, v_s, df_wald) = if self.config.constant && k > 1 {
                let beta_s = betas_nd.slice(ndarray::s![1..]).to_owned();
                let v_s = cov_beta.slice(ndarray::s![1.., 1..]).to_owned();
                (beta_s, v_s, k - 1)
            } else {
                (betas_nd.clone(), cov_beta.clone(), k)
            };
            let v_s_faer = v_s.view().into_faer().to_owned();
            let beta_s_faer = beta_s.view().into_faer_col().to_owned();
            let x_sol = v_s_faer
                .as_ref()
                .llt(Side::Lower)
                .map_err(|_| "IVLIML: V_s not pd for Wald".to_string())?
                .solve(beta_s_faer.as_ref());
            let wald = beta_s.dot(&x_sol.as_ref().into_ndarray());
            let chi2_dist = ChiSquared::new(df_wald as f64).map_err(|e| format!("IVLIML Wald: {}", e))?;
            (wald, 1.0 - chi2_dist.cdf(wald))
        };

        let ztz_inv_nd = ztz_inv.as_ref().into_ndarray().to_owned();
        let df_z = n.saturating_sub(k_z);
        let mut endog_hat = Array2::zeros((n, k_endog));
        let mut first_stage: Vec<super::iv2sls::FirstStageResult> = Vec::with_capacity(k_endog);
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
            let resid = &endog_col - &hat_arr;
            let ss_resid = resid.iter().map(|v| v.powi(2)).sum::<f64>();
            let y_mean_j = endog_col.iter().mean();
            let ss_tot = endog_col.iter().map(|v| (v - y_mean_j).powi(2)).sum::<f64>();
            let r2_j = if ss_tot > 1e-300 { 1.0 - ss_resid / ss_tot } else { 0.0 };
            let sigma2_j = if df_z > 0 { (ss_resid / df_z as f64).max(1e-300) } else { 1e-300 };
            let cov_gamma = sigma2_j * &ztz_inv_nd;
            let stds: Vec<f64> = (0..k_z).map(|i| cov_gamma[[i, i]].sqrt()).collect();
            let gamma_nd = gamma.as_ref().into_ndarray().to_owned();
            let t_dist = statrs::distribution::StudentsT::new(0.0, 1.0, df_z as f64)
                .unwrap_or(statrs::distribution::StudentsT::new(0.0, 1.0, 1.0).unwrap());
            let t_values: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] / stds[i]).collect();
            let p_values: Vec<f64> = t_values.iter().map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs()))).collect();
            let t_crit = t_dist.inverse_cdf(0.975);
            let ci_left: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] - t_crit * stds[i]).collect();
            let ci_right: Vec<f64> = (0..k_z).map(|i| gamma_nd[i] + t_crit * stds[i]).collect();
            let name = self.endog_names.as_ref().and_then(|n| n.get(j).cloned()).unwrap_or_else(|| format!("endog_{}", j + 1));
            let var_names: Vec<String> = (0..k_z)
                .map(|i| self.z_var_names.as_ref().and_then(|v| v.get(i).cloned()).unwrap_or_else(|| format!("z{}", i + 1)))
                .collect();
            let ms_resid = if df_z > 0 { ss_resid / df_z as f64 } else { 0.0 };
            let ms_tot = if n > 1 { ss_tot / (n - 1) as f64 } else { 0.0 };
            let r2_adj = if ms_tot > 1e-300 { 1.0 - ms_resid / ms_tot } else { 0.0 };
            first_stage.push(super::iv2sls::FirstStageResult {
                endog_name: name,
                var_names,
                betas: gamma_nd.to_vec(),
                stds,
                tvalues: t_values,
                pvalues: p_values,
                conf_int_left: ci_left,
                conf_int_right: ci_right,
                r2: r2_j,
                r2_adjusted: r2_adj,
            });
        }

        let first_stage_summary = super::iv2sls::compute_first_stage_summary(
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
            true, // for_liml: use LIML Stock-Yogo size critical values
        )?;

        // Overidentification test (estat overid): Anderson-Rubin chi2, Basmann F.
        // Only when nonrobust VCE. With robust (vce(robust)), Stata does not compute overid.
        let overid = if k_iv > k_endog && !is_robust_cov_type(&covariance_type) {
            let df_overid = k_iv - k_endog;
            let df_denom = n.saturating_sub(k_z);
            let uu = u_structural.dot(&u_structural);
            if df_denom > 0 && uu > 1e-300 {
                let ztu = z.t().dot(&u_structural);
                let ztz_inv_ztu = ztz_inv_nd.dot(&ztu);
                let u_pz_u = ztu.dot(&ztz_inv_ztu);
                let sargan_stat = n as f64 * u_pz_u / uu;
                let basmann_chi2 = if (n as f64 - sargan_stat).abs() > 1e-10 {
                    sargan_stat * (n as f64 - k_z as f64) / (n as f64 - sargan_stat)
                } else {
                    sargan_stat
                };
                let chi2_dist = ChiSquared::new(df_overid as f64)
                    .map_err(|e| format!("IVLIML overid ChiSquared: {}", e))?;
                let ar_p = 1.0 - chi2_dist.cdf(sargan_stat);
                let basmann_f_stat = basmann_chi2 / (df_overid as f64);
                let f_dist = FisherSnedecor::new(df_overid as f64, df_denom as f64)
                    .map_err(|e| format!("IVLIML overid FisherSnedecor: {}", e))?;
                let basmann_p = 1.0 - f_dist.cdf(basmann_f_stat);

                Some(LimlOveridTest {
                    anderson_rubin_stat: sargan_stat,
                    anderson_rubin_p_value: ar_p,
                    basmann_stat: basmann_f_stat,
                    basmann_p_value: basmann_p,
                    df: df_overid,
                    df_denom,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(IVLIMLResult {
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
            model: IVLIMLModel { params: betas_nd.clone() },
            betas: betas_nd,
            stds: std_err,
            zvalues: Array1::from_vec(z_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
            kappa,
            first_stage,
            first_stage_summary,
            overid_k_iv: k_iv,
            overid_k_endog: k_endog,
            overid,
        })
    }
}
