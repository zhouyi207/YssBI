use super::{design::PreparedIvDesign, model::IvModel};
use crate::causal::iv::{
    IvEstimate,
    estimate::{coefficient_inference, model_test},
};
use crate::regression::covariance::compute_cov_beta;
use statrs::statistics::Statistics;
use yss_sci_contract::causal::iv::InstrumentalVariableStatistics;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

impl IvModel {
    pub fn fit_2sls(&self) -> Result<IvEstimate, String> {
        let n = self.endog.nrows();
        let PreparedIvDesign { x, x_struct, .. } = self.design()?;
        let (rank, cond_no) = matrix_rank(x.as_ref()).map_err(|e| e.to_string())?;
        if rank == 0 || rank < x.ncols() {
            return Err("Design matrix is rank deficient".to_string());
        }
        if n <= rank {
            return Err("Insufficient residual degrees of freedom".to_string());
        }
        let df_residual = n - rank;
        let df_model = if self.options.constant {
            rank - 1
        } else {
            rank
        };
        let df_total = df_residual + df_model;

        let covariance_type = self.options.covariance.name().to_owned();

        // OLS on second stage: β = (X'X)^{-1} X'y
        let xtx = x.transpose() * x.as_ref();
        let xty = x.transpose() * self.endog.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: X'X is not positive definite (stage 2). Check for collinearity."
                    .to_string()
            })?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas = xtx_inv.as_ref() * xty.as_ref();

        // ESS and VCE must use structural residuals: u = y - X_struct * β
        // where X_struct = [exog, endog] (actual endogenous, not endog_hat).
        // Stata: ESS = y'y - 2β'X'y + β'X'Xβ, σ² = ESS/(n-k), VCE = σ² (X'P_Z X)^{-1}.
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas.as_ref());

        let y_mean = self.endog.iter().mean();
        let ss_total = if self.options.constant {
            self.endog.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            self.endog.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = u_structural.transpose() * u_structural.as_ref();
        let r2 = if ss_total > 1e-300 {
            1.0 - ss_residual / ss_total
        } else {
            0.0
        };

        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = if ms_total > 1e-300 {
            1.0 - ms_residual / ms_total
        } else {
            0.0
        };

        // Stata: s² = ESS/(n-k) if small, else ESS/n. Affects VCE and robust scale.
        let sigma2_df = if self.small { df_residual } else { n };

        let cov_beta = compute_cov_beta(
            &x,
            &xtx_inv,
            &u_structural,
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;

        let inference = coefficient_inference(&betas, &cov_beta, df_residual, self.small)?;
        let model_test = model_test(
            &betas,
            &cov_beta,
            self.options.constant,
            df_residual,
            self.small,
        )?;

        Ok(IvEstimate {
            fitted: &self.endog - &u_structural,
            residuals: u_structural,
            betas,
            inference,
            statistics: InstrumentalVariableStatistics {
                covariance_type,
                model_test,
                observations: n,
                df_residual,
                r2,
                adjusted_r2: r2_adjusted,
                condition_number: cond_no,
                kappa: 1.0,
            },
        })
    }
}
