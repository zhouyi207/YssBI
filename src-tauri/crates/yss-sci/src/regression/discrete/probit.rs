//! Binary Probit model via IRLS (Iteratively Reweighted Least Squares)
//!
//! P(y=1|x) = Φ(x'β) where Φ is the standard normal CDF.
//! IRLS uses weight w = φ(η)² / [Φ(η)(1-Φ(η))] and working response z = η + (y-p)/w.

use faer::{Col, Mat};
use statrs::distribution::{ChiSquared, Continuous, ContinuousCDF, Normal};
use yss_linalg::{MatrixExt, Solve};

const MAX_ITER: usize = 100;
const TOL: f64 = 1e-8;
const EPS: f64 = 1e-7;

pub struct ProbitConfig {
    pub constant: bool,
}

impl Default for ProbitConfig {
    fn default() -> Self {
        Self { constant: true }
    }
}

pub struct Probit {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub config: ProbitConfig,
}

#[derive(Debug)]
pub struct ProbitModel {
    pub params: Col<f64>,
}

#[derive(Debug)]
pub struct ProbitResult {
    pub num_observation: usize,
    pub model: ProbitModel,
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

impl Probit {
    pub fn fit(&self) -> Result<ProbitResult, String> {
        let n = self.endog.nrows();
        let k = self.exog.ncols();

        if n != self.exog.nrows() {
            return Err(format!(
                "Probit: endog len {} != exog rows {}",
                n,
                self.exog.nrows()
            ));
        }

        for (i, &yi) in self.endog.iter().enumerate() {
            if yi != 0.0 && yi != 1.0 {
                return Err(format!(
                    "Probit: endog must be 0/1, got {} at observation {}",
                    yi,
                    i + 1
                ));
            }
        }

        let normal = Normal::new(0.0, 1.0).map_err(|e| format!("Probit: Normal: {}", e))?;
        let mut beta = Col::<f64>::zeros(k);

        for iter in 0..MAX_ITER {
            let eta = self.exog.as_ref() * beta.as_ref();

            // p = Φ(η), φ = PDF
            let p: Col<f64> = eta.map(|&e| {
                let phi_cdf = normal.cdf(e);
                phi_cdf.clamp(EPS, 1.0 - EPS)
            });
            let phi: Col<f64> = eta.map(|&e| normal.pdf(e));

            // w = φ² / [Φ(1-Φ)]
            let w: Col<f64> = Col::from_fn(n, |i| {
                let pi = p[i];
                let phii = phi[i];
                (phii * phii / (pi * (1.0 - pi))).max(1e-10)
            });

            // z = η + (y-p)/w
            let z: Col<f64> = Col::from_fn(n, |i| eta[i] + (self.endog[i] - p[i]) / w[i]);

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
                .map_err(|_| "Probit: X'WX not positive definite".to_string())?
                .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));

            let beta_new = xtx_inv.as_ref() * xtz.as_ref();
            let beta_new_nd = beta_new.as_ref().to_owned();

            let diff: f64 = beta
                .iter()
                .zip(beta_new_nd.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);

            beta = beta_new_nd;

            if diff < TOL {
                let eta_final = self.exog.as_ref() * beta.as_ref();
                let p_final: Col<f64> = eta_final.map(|&e| normal.cdf(e).clamp(EPS, 1.0 - EPS));
                let phi_final: Col<f64> = eta_final.map(|&e| normal.pdf(e));
                let w_final: Col<f64> = Col::from_fn(n, |i| {
                    let pi = p_final[i];
                    let phii = phi_final[i];
                    (phii * phii / (pi * (1.0 - pi))).max(1e-10)
                });

                let mut xw_final = self.exog.clone();
                for (i, mut row) in xw_final.row_iter_mut().enumerate() {
                    row *= faer::Scale(w_final[i].sqrt());
                }
                let xtx_final = xw_final.as_ref().to_owned();
                let xtx_f = xtx_final.transpose() * xtx_final.as_ref();
                let cov_beta = xtx_f
                    .checked_cholesky()
                    .map_err(|_| "Probit: failed to invert Hessian".to_string())?
                    .solve(&Mat::identity(xtx_f.nrows(), xtx_f.nrows()));
                let cov_beta_nd = cov_beta.as_ref().to_owned();

                let std_err = cov_beta_nd.diagonal().column_vector().map(|v| v.sqrt());

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

                let ll: f64 = self
                    .endog
                    .iter()
                    .zip(p_final.iter())
                    .map(|(yi, pi)| {
                        let pi = pi.clamp(1e-300, 1.0 - 1e-300);
                        if *yi > 0.5 { pi.ln() } else { (1.0 - pi).ln() }
                    })
                    .sum();

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
                        .map_err(|e| format!("Probit: ChiSquared: {}", e))?
                        .cdf(lr_chi2)
                };

                let aic = -2.0 * ll + 2.0 * k as f64;
                let bic = -2.0 * ll + k as f64 * (n as f64).ln();

                return Ok(ProbitResult {
                    num_observation: n,
                    model: ProbitModel {
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
            "Probit: did not converge after {} iterations",
            MAX_ITER
        ))
    }
}
