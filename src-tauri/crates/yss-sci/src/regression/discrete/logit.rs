//! Binary Logit model via IRLS (Iteratively Reweighted Least Squares)
//!
//! IRLS is mathematically equivalent to Newton-Raphson for logistic regression.
//! Each iteration solves a weighted least squares problem.

use faer::{Col, Mat};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use yss_linalg::{MatrixExt, Solve};

const MAX_ITER: usize = 100;
const TOL: f64 = 1e-8;
const EPS: f64 = 1e-7; // clamp p to [EPS, 1-EPS] for numerical stability

/// Logistic function: σ(z) = 1/(1+exp(-z))
#[inline]
fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    }
}

pub struct LogitConfig {
    pub constant: bool,
}

impl Default for LogitConfig {
    fn default() -> Self {
        Self { constant: true }
    }
}

pub struct Logit {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub config: LogitConfig,
}

#[derive(Debug)]
pub struct LogitModel {
    pub params: Col<f64>,
}

#[derive(Debug)]
pub struct LogitResult {
    pub num_observation: usize,
    pub model: LogitModel,
    pub betas: Col<f64>,
    pub stds: Col<f64>,
    pub zvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,
    pub conf_int_right: Col<f64>,
    pub cov_beta: Mat<f64>,
    pub log_likelihood: f64,
    pub ll_null: f64,
    pub pseudo_r2: f64,
    pub lr_chi2: f64,
    pub lr_p_value: f64,
    pub aic: f64,
    pub bic: f64,
    pub iterations: usize,
    pub converged: bool,
}

impl Logit {
    /// Fit the logit model via IRLS.
    pub fn fit(&self) -> Result<LogitResult, String> {
        let n = self.endog.nrows();
        let k = self.exog.ncols();

        if n != self.exog.nrows() {
            return Err(format!(
                "Logit: endog len {} != exog rows {}",
                n,
                self.exog.nrows()
            ));
        }

        // Validate binary outcome
        for (i, &yi) in self.endog.iter().enumerate() {
            if yi != 0.0 && yi != 1.0 {
                return Err(format!(
                    "Logit: endog must be 0/1, got {} at observation {}",
                    yi,
                    i + 1
                ));
            }
        }

        let mut beta = Col::<f64>::zeros(k);

        for iter in 0..MAX_ITER {
            // η = Xβ
            let eta = self.exog.as_ref() * beta.as_ref();

            // p = σ(η), clamped for numerical stability
            let p: Col<f64> = eta.map(|&e| {
                let s = sigmoid(e);
                s.clamp(EPS, 1.0 - EPS)
            });

            // w_i = p_i(1-p_i)
            let w: Col<f64> = p.map(|&pi| (pi * (1.0 - pi)).max(1e-10));

            // z = η + W^{-1}(y - p) = η + (y-p)/w
            let z: Col<f64> = Col::from_fn(n, |i| eta[i] + (self.endog[i] - p[i]) / w[i]);

            // WLS: β_new = (X'WX)^{-1} X'Wz
            // Weight X and z by sqrt(w): Xw = X * sqrt(w), zw = z * sqrt(w)
            // Then (Xw'Xw)^{-1} Xw'zw = (X'WX)^{-1} X'Wz
            let sqrt_w: Col<f64> = w.map(|&wi| wi.sqrt());

            let mut xw = self.exog.clone();
            for (i, mut row) in xw.row_iter_mut().enumerate() {
                row *= faer::Scale(sqrt_w[i]);
            }
            let zw: Col<f64> = z
                .iter()
                .zip(sqrt_w.iter())
                .map(|(zi, sw)| zi * sw)
                .collect();

            let xw_matrix = xw.as_ref().to_owned();
            let zw_vector = zw.as_ref().to_owned();

            let xtx = xw_matrix.transpose() * xw_matrix.as_ref();
            let xtz = xw_matrix.transpose() * zw_vector.as_ref();

            let xtx_inv = xtx
                .checked_cholesky()
                .map_err(|_| {
                    "Logit: X'WX not positive definite (check for separation or collinearity)"
                        .to_string()
                })?
                .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));

            let beta_new = xtx_inv.as_ref() * xtz.as_ref();
            let beta_new_nd = beta_new.as_ref().to_owned();

            // Check convergence
            let diff: f64 = beta
                .iter()
                .zip(beta_new_nd.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);

            beta = beta_new_nd;

            if diff < TOL {
                // Compute final quantities
                let eta_final = self.exog.as_ref() * beta.as_ref();
                let p_final: Col<f64> = eta_final.map(|&e| sigmoid(e).clamp(EPS, 1.0 - EPS));
                let w_final: Col<f64> = p_final.map(|&pi| (pi * (1.0 - pi)).max(1e-10));

                // Covariance: (X'WX)^{-1} at convergence
                let mut xw_final = self.exog.clone();
                for (i, mut row) in xw_final.row_iter_mut().enumerate() {
                    row *= faer::Scale(w_final[i].sqrt());
                }
                let xtx_final = xw_final.as_ref().to_owned();
                let xtx_f = xtx_final.transpose() * xtx_final.as_ref();
                let cov_beta = xtx_f
                    .checked_cholesky()
                    .map_err(|_| "Logit: failed to invert Hessian".to_string())?
                    .solve(&Mat::identity(xtx_f.nrows(), xtx_f.nrows()));
                let cov_beta_nd = cov_beta.as_ref().to_owned();

                let std_err = cov_beta_nd.diagonal().column_vector().map(|v| v.sqrt());

                let normal = Normal::new(0.0, 1.0).map_err(|e| format!("Logit: Normal: {}", e))?;
                let z_values: Vec<f64> = beta
                    .iter()
                    .zip(std_err.iter())
                    .map(|(b, se)| b / se)
                    .collect();
                let p_values: Vec<f64> = z_values
                    .iter()
                    .map(|&z| 2.0 * (1.0 - normal.cdf(z.abs())))
                    .collect();
                let z_crit = normal.inverse_cdf(0.975);
                let ci_lower = beta.clone() - faer::Scale(z_crit) * std_err.clone();
                let ci_upper = beta.clone() + faer::Scale(z_crit) * std_err.clone();

                // Log-likelihood: L = Σ [y*log(p) + (1-y)*log(1-p)]
                let ll: f64 = self
                    .endog
                    .iter()
                    .zip(p_final.iter())
                    .map(|(yi, pi)| {
                        let pi = pi.clamp(1e-300, 1.0 - 1e-300);
                        if *yi > 0.5 { pi.ln() } else { (1.0 - pi).ln() }
                    })
                    .sum();

                // Null model (constant only): p = mean(y)
                let y_mean = self.endog.iter().sum::<f64>() / self.endog.nrows() as f64;
                let p_null = y_mean.clamp(EPS, 1.0 - EPS);
                let ll_null: f64 = self
                    .endog
                    .iter()
                    .map(|yi| {
                        let p = if *yi > 0.5 { p_null } else { 1.0 - p_null };
                        p.ln()
                    })
                    .sum();

                let pseudo_r2 = 1.0 - ll / ll_null;
                let lr_chi2 = 2.0 * (ll - ll_null);
                let df_model = if self.config.constant {
                    k.saturating_sub(1)
                } else {
                    k
                };
                let lr_p_value = if df_model <= 0 {
                    1.0
                } else {
                    1.0 - ChiSquared::new(df_model as f64)
                        .map_err(|e| format!("Logit: ChiSquared: {}", e))?
                        .cdf(lr_chi2)
                };

                let aic = -2.0 * ll + 2.0 * k as f64;
                let bic = -2.0 * ll + k as f64 * (n as f64).ln();

                return Ok(LogitResult {
                    num_observation: n,
                    model: LogitModel {
                        params: beta.clone(),
                    },
                    betas: beta,
                    stds: std_err,
                    zvalues: (z_values).into_iter().collect::<Col<f64>>(),
                    pvalues: (p_values).into_iter().collect::<Col<f64>>(),
                    conf_int_left: ci_lower,
                    conf_int_right: ci_upper,
                    cov_beta: cov_beta_nd,
                    log_likelihood: ll,
                    ll_null,
                    pseudo_r2,
                    lr_chi2,
                    lr_p_value,
                    aic,
                    bic,
                    iterations: iter + 1,
                    converged: true,
                });
            }
        }

        Err(format!(
            "Logit: did not converge after {} iterations",
            MAX_ITER
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logit_constant_only() {
        // Constant-only: 50% y=0, 50% y=1 -> should give intercept ~0, p ~ 0.5
        let endog = (vec![
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
            1.0, 1.0, 1.0,
        ])
        .into_iter()
        .collect::<Col<f64>>();
        let exog = Mat::full(20, 1, 1.0);

        let logit = Logit {
            endog,
            exog,
            config: LogitConfig { constant: true },
        };
        let result = logit.fit().unwrap();

        assert!(result.converged);
        assert_eq!(result.betas.nrows(), 1);
        assert!((result.betas[0].abs() - 0.0).abs() < 0.01);
        assert!(result.pseudo_r2 >= 0.0 && result.pseudo_r2 < 0.01);
    }

    #[test]
    fn test_logit_with_covariate() {
        // y=1 when x>0.5, with overlap in middle
        let n = 80;
        let mut endog = Vec::with_capacity(n);
        let mut exog_raw = Vec::with_capacity(n * 2);
        for i in 0..n {
            let x = (i as f64) / (n as f64);
            let y = if x < 0.35 {
                0.0
            } else if x > 0.65 {
                1.0
            } else {
                if (i - 28) % 3 == 0 { 1.0 } else { 0.0 }
            };
            endog.push(y);
            exog_raw.push(1.0);
            exog_raw.push(x);
        }
        let endog = (endog).into_iter().collect::<Col<f64>>();
        let exog = faer::MatRef::from_row_major_slice(&(exog_raw), n, 2).to_owned();

        let logit = Logit {
            endog,
            exog,
            config: LogitConfig { constant: true },
        };
        let result = logit.fit().unwrap();

        assert!(result.converged);
        assert_eq!(result.betas.nrows(), 2);
        assert!(result.betas[1] > 0.0);
    }
}
