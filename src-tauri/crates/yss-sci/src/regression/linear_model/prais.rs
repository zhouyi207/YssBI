//! Prais-Winsten and Cochrane-Orcutt regression for AR(1) errors
//!
//! Stata: prais y x1 x2 [, corc]
//! - Prais-Winsten: preserves first observation via √(1-ρ²) transform
//! - Cochrane-Orcutt (corc): drops first observation

use crate::ts::serial_correlation::durbin_watson;
use statrs::distribution::{ContinuousCDF, FisherSnedecor, StudentsT};
use yss_sci_linalg::matrix_rank;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

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
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub config: PraisConfig,
}

#[derive(Debug)]
pub struct PraisModel {
    pub params: Col<f64>,
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
    pub betas: Col<f64>,
    pub stds: Col<f64>,
    pub tvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,
    pub conf_int_right: Col<f64>,
    pub cov_beta: Mat<f64>,
    pub cond_no: f64,
    pub rho: f64,
    pub dw_original: f64,
    pub dw_transformed: f64,
    pub iterations: usize,
    /// Iteration log: "iteration N: rho = X.XXXX" for each step
    pub iteration_log: Vec<String>,
}

/// Estimate ρ from residuals using rhotype(regress): u_t = ρ u_{t-1} + e_t
fn estimate_rho_regress(residuals: &[f64]) -> Result<f64, String> {
    let n = residuals.len();
    if n < 2 {
        return Ok(0.0);
    }
    let sum_uu: f64 = (1..n).map(|t| residuals[t] * residuals[t - 1]).sum();
    let sum_u2: f64 = (0..n - 1).map(|t| residuals[t] * residuals[t]).sum();
    if sum_u2 <= 1e-20 {
        return Ok(0.0);
    }
    let rho = sum_uu / sum_u2;
    if !rho.is_finite() {
        return Err("Prais: nonfinite AR(1) estimate".into());
    }
    // Keep the fitted AR(1) process stationary, including boundary estimates.
    Ok(rho.clamp(-0.999, 0.999))
}

impl Prais {
    pub fn fit(&self) -> Result<PraisResult, String> {
        let n = self.endog.nrows();
        let k = self.exog.ncols();
        if n < 3 {
            return Err("Prais: need at least 3 observations".to_string());
        }
        if k == 0 {
            return Err("Prais: need at least one regressor".to_string());
        }

        if self.config.max_iter == 0 || !self.config.tol.is_finite() || self.config.tol <= 0.0 {
            return Err("Prais: max_iter and tolerance must be positive and finite".into());
        }
        if self.exog.nrows() != n || self.endog.iter().any(|v| !v.is_finite()) {
            return Err("Prais: invalid response or design dimensions".into());
        }
        let y_nd = &self.endog;
        let x_nd = &self.exog;
        let y = y_nd.as_ref().to_owned();
        let x = x_nd.as_ref().to_owned();

        // Initial OLS
        let xtx = x.transpose() * x.as_ref();
        let xty = x.transpose() * y.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| {
                "Prais: X'X is singular (rank-deficient). Check for multicollinearity.".to_string()
            })?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas_init = xtx_inv.as_ref() * xty.as_ref();
        let y_hat_init = x.as_ref() * betas_init.as_ref();
        let u_init: Vec<f64> = y
            .iter()
            .zip(y_hat_init.as_ref().iter())
            .map(|(a, b)| a - b)
            .collect();

        let dw_original = durbin_watson(&u_init);

        let mut residuals: Vec<f64> = u_init;
        let mut betas: Col<f64>;
        let mut rho = 0.0;
        let mut iterations = 0;
        let mut xtx_inv_s;
        let mut cond_no: f64;
        let mut iteration_log: Vec<String> = Vec::new();

        loop {
            let rho_old = rho;
            rho = estimate_rho_regress(&residuals)?;

            iteration_log.push(format!("Prais iteration {}: rho = {:.4}", iterations, rho));

            let scale = (1.0 - rho * rho).sqrt();
            if scale <= 1e-10 {
                return Err("Prais: ρ too close to ±1, transformation unstable".to_string());
            }

            let drop_first = self.config.transform == PraisTransform::CochraneOrcutt;
            let n_star = n - usize::from(drop_first);
            let transform = |i: usize, value: f64, previous: f64| {
                if i == 0 && !drop_first {
                    scale * value
                } else {
                    value - rho * previous
                }
            };
            let y_star_vector = Col::from_fn(n_star, |i| {
                let t = i + usize::from(drop_first);
                transform(i, y_nd[t], y_nd[t.saturating_sub(1)])
            });
            let x_star_matrix = Mat::from_fn(n_star, k, |i, j| {
                let t = i + usize::from(drop_first);
                transform(i, x_nd[(t, j)], x_nd[(t.saturating_sub(1), j)])
            });

            let (rank, cond_no_val) =
                matrix_rank(x_star_matrix.as_ref()).map_err(|e| e.to_string())?;
            if rank == 0 || rank < x_star_matrix.ncols() {
                return Err("Prais: transformed design is rank deficient".to_string());
            }
            if n_star <= rank {
                return Err("Prais: insufficient residual degrees of freedom".to_string());
            }
            cond_no = cond_no_val;

            let xtx_s = x_star_matrix.transpose() * x_star_matrix.as_ref();
            let xty_s = x_star_matrix.transpose() * y_star_vector.as_ref();

            xtx_inv_s = xtx_s
                .checked_cholesky()
                .map_err(|_| "Prais: transformed X'X is singular".to_string())?
                .solve(&Mat::identity(xtx_s.nrows(), xtx_s.nrows()));
            betas = (xtx_inv_s.as_ref() * xty_s.as_ref()).as_ref().to_owned();

            let y_hat_star = x_star_matrix.as_ref() * betas.as_ref();
            let res_trans: Vec<f64> = y_star_vector
                .iter()
                .zip(y_hat_star.as_ref().iter())
                .map(|(a, b)| a - b)
                .collect();
            let dw_transformed = durbin_watson(&res_trans);

            iterations += 1;

            let converged = (rho - rho_old).abs() < self.config.tol;
            if !converged && iterations >= self.config.max_iter {
                return Err(format!(
                    "Prais: did not converge after {iterations} iterations"
                ));
            }

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
                let intercept = self.config.constant.then(|| {
                    Col::from_fn(n_star, |i| {
                        if i == 0 && self.config.transform == PraisTransform::PraisWinsten {
                            scale
                        } else {
                            1.0 - rho
                        }
                    })
                });
                let ss_total = super::transformed_total_ss(&y_star_vector, intercept.as_ref());
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
                let xtx_inv_nd = xtx_inv_s.as_ref().to_owned();
                let cov_beta = yss_sci_linalg::Scale(ms_residual) * &xtx_inv_nd;
                let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());
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
                let ci_lower = &betas - &std_err.map(|&v| t_crit * v);
                let ci_upper = &betas + &std_err.map(|&v| t_crit * v);

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
                    tvalues: (t_values).into_iter().collect::<Col<f64>>(),
                    pvalues: (p_values).into_iter().collect::<Col<f64>>(),
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
            let y_hat = x.as_ref() * betas.as_ref();
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
    use yss_sci_linalg::Col;

    #[test]
    fn test_prais_basic() {
        let n = 30;
        let y: Vec<f64> = (0..n)
            .map(|i| 10.0 + 0.5 * i as f64 + (i as f64 * 0.3).sin())
            .collect();
        let x: Vec<f64> = (0..n).map(|i| i as f64 * 1.2 + 2.0).collect();
        let mut exog = Vec::with_capacity(n * 2);
        for &value in &x {
            exog.push(1.0);
            exog.push(value);
        }
        let endog = (y).into_iter().collect::<Col<f64>>();
        let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog), n, 2).to_owned();

        let mut prais = Prais {
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
        prais.endog = prais.endog.map(|v| v + 100.0);
        let shifted = prais.fit().unwrap();
        assert!((shifted.r2 - r.r2).abs() < 1e-8);
        assert_eq!(estimate_rho_regress(&[1.0, 2.0, 4.0]).unwrap(), 0.999);
        assert!(estimate_rho_regress(&[f64::INFINITY, 1.0]).is_err());
        prais.config.max_iter = 1;
        assert!(prais.fit().unwrap_err().contains("did not converge"));
        prais.config.max_iter = 0;
        assert!(prais.fit().is_err());
        prais.config.max_iter = 50;
        prais.config.tol = f64::NAN;
        assert!(prais.fit().is_err());
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
        let endog = (y).into_iter().collect::<Col<f64>>();
        let exog = yss_sci_linalg::MatRef::from_row_major_slice(&(exog), n, 2).to_owned();

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
