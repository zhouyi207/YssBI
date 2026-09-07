use crate::regression::covariance::{CovParams, compute_cov_beta};

use ndarray::{Array1, Array2};
use num_traits::{One, Pow, Zero};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};
use yss_linalg::matrix_rank;
use yss_linalg::{MatMul, MatrixExt, Solve};

fn is_robust_cov_type(cov_type: &str) -> bool {
    matches!(
        cov_type,
        "HC0" | "HC1" | "HC2" | "HC3" | "cluster" | "HAC" | "newey" | "fixed scale"
    )
}

pub struct OLSConfig {
    pub constant: bool,
    /// Covariance type: "nonrobust", "HC0", "HC1", "HC2", "HC3", "fixed scale", "cluster", etc.
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
}

pub struct OLS {
    pub endog: Array1<f64>, // 因变量 y
    pub exog: Array2<f64>,  // 自变量 X (n × k)
    pub config: OLSConfig,
}

#[derive(Debug)]
pub struct OLSModel {
    pub params: Array1<f64>, // 估计的系数 β
}

#[derive(Debug)]
pub struct OLSResult {
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
    pub fvalue: f64,
    pub f_p_value: f64,

    pub model: OLSModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,  // 置信区间左侧
    pub conf_int_right: Array1<f64>, // 置信区间右侧

    /// 参数协方差矩阵 (k×k)，用于 Wald 假设检验
    pub cov_beta: Array2<f64>,

    /// Nonrobust VCE: σ² (X'X)⁻¹, always available for Hausman test
    pub cov_beta_nonrobust: Array2<f64>,

    // 矩阵是否病态（多重共线性）
    pub cond_no: f64,
}

impl OLS {
    pub fn fit(&self) -> Result<OLSResult, String> {
        let y = self.endog.view().to_owned();
        let x = self.exog.view().to_owned();

        let (rank, cond_no) = matrix_rank(x.view()).unwrap_or((0, f64::INFINITY));

        let num_obversion = x.nrows();
        let df_redidual = num_obversion - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_redidual + df_model;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        // 普通最小二乘
        let xtx = x.t().matmul(&x.view());
        let xty = x.t().matmul(&y.view());
        let xtx_inv = xtx
            .cholesky()
            .map_err(|_| "OLS: X'X matrix is not positive definite (likely rank-deficient or has multicollinearity). Check your input variables.".to_string())?
            .solve(&ndarray::Array2::<f64>::eye(xtx.nrows()));
        let betas = xtx_inv.view().matmul(&xty);
        let y_hat = x.view().matmul(&betas.view());

        let y_mean = y.iter().mean();

        let ss_total = if self.config.constant {
            y.iter().map(|v| (v - y_mean).pow(2)).sum::<f64>()
        } else {
            y.iter().map(|v| v.pow(2)).sum::<f64>()
        };
        let ss_residual = (&y.view() - &y_hat.view())
            .iter()
            .map(|v| v.pow(2))
            .sum::<f64>();
        let ss_model = ss_total - ss_residual;

        let r2 = 1.0 - ss_residual / ss_total;

        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_redidual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = 1.0 - ms_residual / ms_total;

        // 残差（需在 cov_beta 之前计算）
        let u = &y - &y_hat.view();

        let x_nd = x.view().to_owned();
        let xtx_inv_nd = xtx_inv.view().to_owned();
        let u_nd: Array1<f64> = u.view().to_owned();

        // 参数协方差矩阵（根据 cov_type）
        let cov_beta = compute_cov_beta(
            &x_nd,
            &xtx_inv_nd,
            &u_nd,
            df_redidual,
            &covariance_type,
            self.config.cov_params.as_ref(),
        )?;

        let cov_beta_nonrobust = ms_residual * &xtx_inv_nd;

        // F 统计量：robust VCE 时用 Wald，否则用经典 F
        let (f, f_p_value) = if df_model > 0 {
            if is_robust_cov_type(&covariance_type) {
                let betas_nd = betas.view().to_owned();
                // Wald = β_s' V_s^{-1} β_s，F = Wald / df_model
                let (beta_s, v_s) = if self.config.constant && rank > 1 {
                    (
                        betas_nd.slice(ndarray::s![1..]).into_owned(),
                        cov_beta.slice(ndarray::s![1.., 1..]).into_owned(),
                    )
                } else {
                    (betas_nd.clone(), cov_beta.clone())
                };
                let wald = if beta_s.len() > 0 {
                    let v_matrix = v_s.view().to_owned();
                    let beta_vector = beta_s.view().to_owned();
                    match v_matrix.view().cholesky() {
                        Ok(llt) => {
                            let x_sol = llt.solve(&beta_vector.view());
                            beta_s.dot(&x_sol.view())
                        }
                        Err(_) => 0.0, // cov 非正定（如 cluster 聚类少）时回退
                    }
                } else {
                    0.0
                };
                let f_val = (wald / df_model as f64).max(0.0);
                let df1 = (df_model as f64).max(1.0);
                let df2 = (df_redidual as f64).max(1.0);
                let dist = FisherSnedecor::new(df1, df2).map_err(|e| {
                    format!(
                        "OLS F-distribution: df_model={} df_residual={} {}",
                        df_model, df_redidual, e
                    )
                })?;
                (f_val, 1.0 - dist.cdf(f_val))
            } else {
                let f_val = (ms_model / ms_residual).max(0.0);
                let df1 = (df_model as f64).max(1.0);
                let df2 = (df_redidual as f64).max(1.0);
                let dist = FisherSnedecor::new(df1, df2).map_err(|e| {
                    format!(
                        "OLS F-distribution: df_model={} df_residual={} {}",
                        df_model, df_redidual, e
                    )
                })?;
                (f_val, 1.0 - dist.cdf(f_val))
            }
        } else {
            (0.0, 1.0)
        };

        // std err
        let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);

        // t value
        let betas_nd = betas.view().to_owned();
        let t_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| b / se)
            .collect();

        let t_df = (df_redidual as f64).max(1.0);
        let t_dist = StudentsT::new(f64::zero(), f64::one(), t_df)
            .map_err(|e| format!("OLS t-distribution: df_residual={} {}", df_redidual, e))?;
        let p_values: Vec<f64> = t_values
            .iter()
            .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
            .collect();

        let t_cirt = t_dist.inverse_cdf(0.975);
        let ci_lower = betas_nd.clone() - t_cirt * std_err.clone();
        let ci_upper = betas_nd.clone() + t_cirt * std_err.clone();

        Ok(OLSResult {
            num_observation: num_obversion,
            ss_model,
            ss_residual,
            ss_total,
            df_model,
            df_residual: df_redidual,
            df_total,
            ms_model,
            ms_residual,
            ms_total,
            covariance_type,
            r2,
            r2_adjusted,
            fvalue: f,
            f_p_value,
            model: OLSModel {
                params: betas_nd.clone(),
            },
            betas: betas_nd,
            stds: std_err,
            tvalues: Array1::from_vec(t_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cov_beta_nonrobust,
            cond_no,
        })
    }
}
