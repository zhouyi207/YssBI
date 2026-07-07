//! Prais-Winsten and Cochrane-Orcutt regression for AR(1) errors
//!
//! Stata: prais y x1 x2 [, corc]
//! - Prais-Winsten: preserves first observation via √(1-ρ²) transform
//! - Cochrane-Orcutt (corc): drops first observation

use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use crate::ts::serial_correlation::durbin_watson;
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};

/// Transform method: Prais-Winsten (keep t=1) or Cochrane-Orcutt (drop t=1)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PraisTransform {
    PraisWinsten,
    CochraneOrcutt,
}

/// ρ estimation method (default: regress = OLS of u_t on u_{t-1})
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RhoType {
    Regress,
}

pub struct PraisConfig {
    pub constant: bool,
    pub transform: PraisTransform,
    pub rhotype: RhoType,
    pub max_iter: usize,
    pub tol: f64,
}

impl Default for PraisConfig {
    fn default() -> Self {
        Self {
            constant: true,
            transform: PraisTransform::PraisWinsten,
            rhotype: RhoType::Regress,
            max_iter: 100,
            tol: 1e-6,
        }
    }
}

pub struct Prais {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub config: PraisConfig,
}

#[derive(Debug)]
pub struct PraisModel {
    pub params: Array1<f64>,
    pub rho: f64,
}

#[derive(Debug)]
pub struct PraisResult {
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
    pub model: PraisModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,
    pub rho: f64,
    pub dw_original: f64,
    pub dw_transformed: f64,
    pub iterations: usize,
    /// Iteration log: "iteration N: rho = X.XXXX" for each step
    pub iteration_log: Vec<String>,
}

/// Estimate ρ from residuals using rhotype(regress): u_t = ρ u_{t-1} + e_t
fn estimate_rho_regress(residuals: &[f64]) -> f64 {
    let n = residuals.len();
    if n < 2 {
        return 0.0;
    }
    let sum_uu: f64 = (1..n).map(|t| residuals[t] * residuals[t - 1]).sum();
    let sum_u2: f64 = (0..n - 1).map(|t| residuals[t] * residuals[t]).sum();
    if sum_u2 <= 1e-20 {
        return 0.0;
    }
    let rho = sum_uu / sum_u2;
    rho.clamp(-0.999, 0.999)
}

impl Prais {
    pub fn fit(&self) -> Result<PraisResult, String> {
        let n = self.endog.len();
        let k = self.exog.ncols();
        if n < 3 {
            return Err("Prais: need at least 3 observations".to_string());
        }
        if k == 0 {
            return Err("Prais: need at least one regressor".to_string());
        }

        let y_nd = &self.endog;
        let x_nd = &self.exog;
        let y = y_nd.view().into_faer_col().to_owned();
        let x = x_nd.view().into_faer().to_owned();

        // Initial OLS
        let xtx = x.as_ref().transpose() * x.as_ref();
        let xty = x.as_ref().transpose() * y.as_ref();
        let xtx_inv = xtx
            .llt(Side::Lower)
            .map_err(|_| {
                "Prais: X'X is singular (rank-deficient). Check for multicollinearity.".to_string()
            })?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let betas_init = xtx_inv.as_ref() * xty.as_ref();
        let y_hat_init = x.as_ref() * betas_init.as_ref();
        let u_init: Vec<f64> = y
            .iter()
            .zip(y_hat_init.as_ref().iter())
            .map(|(a, b)| a - b)
            .collect();

        let dw_original = durbin_watson(&u_init);

        let mut residuals: Vec<f64> = u_init;
        let mut betas: Array1<f64>;
        let mut rho = 0.0;
        let mut iterations = 0;
        let mut xtx_inv_s;
        let mut cond_no: f64;
        let mut iteration_log: Vec<String> = Vec::new();

        loop {
            let rho_old = rho;
            rho = estimate_rho_regress(&residuals);

            iteration_log.push(format!("Prais iteration {}: rho = {:.4}", iterations, rho));

            let scale = (1.0 - rho * rho).sqrt();
            if scale <= 1e-10 {
                return Err("Prais: ρ too close to ±1, transformation unstable".to_string());
            }

            let (y_star, x_star) = if self.config.transform == PraisTransform::CochraneOrcutt {
                let mut y_star = Vec::with_capacity(n - 1);
                let mut x_star = Vec::with_capacity((n - 1) * k);
                for t in 1..n {
                    y_star.push(y_nd[t] - rho * y_nd[t - 1]);
                    for j in 0..k {
                        x_star.push(x_nd[[t, j]] - rho * x_nd[[t - 1, j]]);
                    }
                }
                (
                    Array1::from_vec(y_star),
                    Array2::from_shape_vec((n - 1, k), x_star)
                        .map_err(|e| format!("Prais: {}", e))?,
                )
            } else {
                let mut y_star = Vec::with_capacity(n);
                let mut x_star = Vec::with_capacity(n * k);
                y_star.push(scale * y_nd[0]);
                for j in 0..k {
                    x_star.push(scale * x_nd[[0, j]]);
                }
                for t in 1..n {
                    y_star.push(y_nd[t] - rho * y_nd[t - 1]);
                    for j in 0..k {
                        x_star.push(x_nd[[t, j]] - rho * x_nd[[t - 1, j]]);
                    }
                }
                (
                    Array1::from_vec(y_star),
                    Array2::from_shape_vec((n, k), x_star).map_err(|e| format!("Prais: {}", e))?,
                )
            };

            let n_star = y_star.len();
            let x_star_faer = x_star.view().into_faer().to_owned();
            let y_star_faer = y_star.view().into_faer_col().to_owned();

            let (rank, cond_no_val) = matrix_rank(x_star_faer.as_ref().to_owned());
            cond_no = cond_no_val;

            let xtx_s = x_star_faer.as_ref().transpose() * x_star_faer.as_ref();
            let xty_s = x_star_faer.as_ref().transpose() * y_star_faer.as_ref();

            xtx_inv_s = xtx_s
                .llt(Side::Lower)
                .map_err(|_| "Prais: transformed X'X is singular".to_string())?
                .solve(Mat::identity(xtx_s.nrows(), xtx_s.ncols()));
            betas = (xtx_inv_s.as_ref() * xty_s.as_ref())
                .as_ref()
                .into_ndarray()
                .to_owned();

            let y_hat_star = x_star_faer.as_ref() * betas.view().into_faer_col();
            let res_trans: Vec<f64> = y_star_faer
                .iter()
                .zip(y_hat_star.as_ref().iter())
                .map(|(a, b)| a - b)
                .collect();
            let dw_transformed = durbin_watson(&res_trans);

            iterations += 1;

            let converged =
                (rho - rho_old).abs() < self.config.tol || iterations >= self.config.max_iter;

            if converged {
                let df_residual = n_star - rank;
                let df_model = if self.config.constant {
                    rank.saturating_sub(1)
                } else {
                    rank
                };
                let df_total = df_residual + df_model;

                // All statistics based on ρ-transformed variables (Stata convention)
                let ss_residual: f64 = res_trans.iter().map(|r| r * r).sum();
                let y_star_mean = y_star_faer.iter().mean();
                let ss_total: f64 = if self.config.constant {
                    y_star_faer.iter().map(|v| (v - y_star_mean).powi(2)).sum()
                } else {
                    y_star_faer.iter().map(|v| v.powi(2)).sum()
                };
                let ss_model = ss_total - ss_residual;
                let r2 = 1.0 - ss_residual / ss_total;
                let ms_model = ss_model / df_model.max(1) as f64;
                let ms_residual = ss_residual / df_residual as f64;
                let ms_total = ss_total / df_total as f64;
                let r2_adjusted = 1.0 - ms_residual / ms_total;
                let f = ms_model / ms_residual;

                let dist_f = FisherSnedecor::new(df_model as f64, df_residual as f64)
                    .map_err(|e| format!("Prais: {}", e))?;
                let f_p_value = 1.0 - dist_f.cdf(f);

                // cov(β) = σ² (X*'X*)⁻¹, σ² = ms_residual
                let xtx_inv_nd = xtx_inv_s.as_ref().into_ndarray().to_owned();
                let cov_beta = ms_residual * &xtx_inv_nd;
                let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);
                let t_values: Vec<f64> = betas
                    .iter()
                    .zip(std_err.iter())
                    .map(|(b, se)| b / se)
                    .collect();
                let t_dist = StudentsT::new(0.0, 1.0, df_residual as f64)
                    .map_err(|e| format!("Prais: {}", e))?;
                let p_values: Vec<f64> = t_values
                    .iter()
                    .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
                    .collect();
                let t_crit = t_dist.inverse_cdf(0.975);
                let ci_lower = &betas - &(std_err.mapv(|v| t_crit * v));
                let ci_upper = &betas + &(std_err.mapv(|v| t_crit * v));

                let method = match self.config.transform {
                    PraisTransform::PraisWinsten => "Prais-Winsten",
                    PraisTransform::CochraneOrcutt => "Cochrane-Orcutt",
                };

                return Ok(PraisResult {
                    num_observation: n_star,
                    ss_model,
                    ss_residual,
                    ss_total,
                    df_model,
                    df_residual,
                    df_total,
                    ms_model,
                    ms_residual,
                    ms_total,
                    covariance_type: format!("{} AR(1)", method),
                    r2,
                    r2_adjusted,
                    fvalue: f,
                    f_p_value,
                    model: PraisModel {
                        params: betas.clone(),
                        rho,
                    },
                    betas: betas.clone(),
                    stds: std_err,
                    tvalues: Array1::from_vec(t_values),
                    pvalues: Array1::from_vec(p_values),
                    conf_int_left: ci_lower,
                    conf_int_right: ci_upper,
                    cov_beta,
                    cond_no,
                    rho,
                    dw_original,
                    dw_transformed,
                    iterations,
                    iteration_log,
                });
            }

            // Update residuals for next iteration: ŷ = Xβ on original data
            let y_hat = x.as_ref() * betas.view().into_faer_col();
            residuals = y
                .iter()
                .zip(y_hat.as_ref().iter())
                .map(|(a, b)| a - b)
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, Array2};

    #[test]
    fn test_prais_basic() {
        let n = 30;
        let mut y: Vec<f64> = (0..n)
            .map(|i| 10.0 + 0.5 * i as f64 + (i as f64 * 0.3).sin())
            .collect();
        let x: Vec<f64> = (0..n).map(|i| i as f64 * 1.2 + 2.0).collect();
        let mut exog = Vec::with_capacity(n * 2);
        for i in 0..n {
            exog.push(1.0);
            exog.push(x[i]);
        }
        let endog = Array1::from_vec(y);
        let exog = Array2::from_shape_vec((n, 2), exog).unwrap();

        let prais = Prais {
            endog: endog.clone(),
            exog: exog.clone(),
            config: PraisConfig {
                constant: true,
                transform: PraisTransform::PraisWinsten,
                rhotype: RhoType::Regress,
                max_iter: 50,
                tol: 1e-6,
            },
        };
        let r = prais.fit().unwrap();
        assert!(r.rho.abs() < 1.0);
        assert!(r.iterations >= 1);
        assert_eq!(r.num_observation, n);
        assert!(r.r2 >= 0.0 && r.r2 <= 1.0);
    }

    #[test]
    fn test_prais_corc() {
        let n = 30;
        let y: Vec<f64> = (0..n).map(|i| 5.0 + 0.2 * i as f64).collect();
        let mut exog = Vec::with_capacity(n * 2);
        for i in 0..n {
            exog.push(1.0);
            exog.push(i as f64);
        }
        let endog = Array1::from_vec(y);
        let exog = Array2::from_shape_vec((n, 2), exog).unwrap();

        let prais = Prais {
            endog,
            exog,
            config: PraisConfig {
                constant: true,
                transform: PraisTransform::CochraneOrcutt,
                rhotype: RhoType::Regress,
                max_iter: 50,
                tol: 1e-6,
            },
        };
        let r = prais.fit().unwrap();
        assert_eq!(r.num_observation, n - 1);
        assert!(r.dw_transformed >= 0.0 && r.dw_transformed <= 4.0);
    }
}
