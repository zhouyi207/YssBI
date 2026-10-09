use super::{design::regressor_design, model::IvModel};
use crate::causal::iv::{
    IvEstimate,
    estimate::{coefficient_inference, goodness_of_fit, model_test},
};
use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::causal::iv::InstrumentalVariableStatistics;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

impl IvModel {
    pub fn fit_2sls(&self) -> Result<IvEstimate, String> {
        let n = self.endog.nrows();
        let (x, x_struct) = {
            let design = self.design()?;
            let included = self.exog.ncols() + usize::from(self.options.constant);
            let included_design = design.z.subcols(0, included);
            (
                regressor_design(
                    included_design,
                    &design.endog_hat.col_iter().collect::<Vec<_>>(),
                ),
                regressor_design(
                    included_design,
                    &self.endog_reg.col_iter().collect::<Vec<_>>(),
                ),
            )
        };
        let (rank, cond_no) = matrix_rank(x.as_ref()).map_err(|e| e.to_string())?;
        if rank == 0 || rank < x.ncols() {
            return Err("Design matrix is rank deficient".to_string());
        }
        if n <= rank {
            return Err("Insufficient residual degrees of freedom".to_string());
        }
        let df_residual = n - rank;
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

        let (r2, r2_adjusted) = goodness_of_fit(
            self.endog.as_ref(),
            u_structural.as_ref(),
            self.options.constant,
            df_residual,
        )?;

        // Stata: s² = ESS/(n-k) if small, else ESS/n. Affects VCE and robust scale.
        let sigma2_df = if self.small { df_residual } else { n };

        let cov_beta = compute_cov_beta(
            &x,
            &xtx_inv,
            u_structural.as_ref(),
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;

        let inference = coefficient_inference(betas.as_ref(), &cov_beta, df_residual, self.small)?;
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
