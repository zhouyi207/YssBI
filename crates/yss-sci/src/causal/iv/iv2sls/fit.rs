use super::{design::PreparedIvDesign, types::IV2SLS};
use crate::causal::iv::IvEstimate;
use crate::regression::{covariance::compute_cov_beta, design::covariance_rows};
use statrs::{
    distribution::{ChiSquared, ContinuousCDF, Normal},
    statistics::Statistics,
};
use yss_sci_contract::{
    causal::iv::InstrumentalVariableStatistics, regression::fit::RegressionCoefficientStatistics,
};
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

impl IV2SLS {
    pub fn fit(&self) -> Result<IvEstimate, String> {
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
        let x_matrix = x.as_ref().to_owned();
        let y_vector = self.endog.as_ref().to_owned();
        let xtx = x_matrix.transpose() * x_matrix.as_ref();
        let xty = x_matrix.transpose() * y_vector.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: X'X is not positive definite (stage 2). Check for collinearity."
                    .to_string()
            })?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas_vector = xtx_inv.as_ref() * xty.as_ref();
        let betas_nd = betas_vector.as_ref().to_owned();

        // ESS and VCE must use structural residuals: u = y - X_struct * β
        // where X_struct = [exog, endog] (actual endogenous, not endog_hat).
        // Stata: ESS = y'y - 2β'X'y + β'X'Xβ, σ² = ESS/(n-k), VCE = σ² (X'P_Z X)^{-1}.
        let u_structural: Col<f64> = &self.endog - &(x_struct.as_ref() * betas_nd.as_ref());

        let y_mean = y_vector.iter().mean();
        let ss_total = if self.options.constant {
            y_vector.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            y_vector.iter().map(|v| v.powi(2)).sum::<f64>()
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
            &x_matrix,
            &xtx_inv,
            &u_structural,
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;

        let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());
        // 2SLS uses asymptotic inference: z = coef/se ~ N(0,1), not t
        let z_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| if *se > 1e-300 { b / se } else { 0.0 })
            .collect();

        let std_normal = Normal::new(0.0, 1.0).map_err(|e| format!("IV2SLS: {}", e))?;
        let p_values: Vec<f64> = z_values
            .iter()
            .map(|&z| crate::distribution::normal_two_sided_p(z))
            .collect();

        let z_crit = std_normal.inverse_cdf(0.975);
        let ci_lower = &betas_nd - yss_sci_linalg::Scale(z_crit) * &std_err;
        let ci_upper = &betas_nd + yss_sci_linalg::Scale(z_crit) * &std_err;

        // Wald chi2 for joint significance (2SLS uses chi2, not F). Stata Methods: "If c=1 and small is not
        // specified, a Wald statistic W of the joint significance of the k−1 parameters of β except the
        // constant term is calculated; W ∼ χ²(k−1)." W = β_s' V_s^{-1} β_s. Use solve(V_s, β_s) for stability.
        let k = betas_nd.nrows();
        let (wald_chi2, wald_p) = {
            let (beta_s, v_s, df_wald) = if self.options.constant && k > 1 {
                // Exclude constant (index 0). Our X = [const, exog, endog_hat], so const is always first.
                let beta_s = betas_nd.subrows(1, betas_nd.nrows() - 1).to_owned();
                let v_s = cov_beta
                    .submatrix(1, 1, cov_beta.nrows() - 1, cov_beta.ncols() - 1)
                    .to_owned();
                (beta_s, v_s, k - 1)
            } else {
                let beta_s = betas_nd.clone();
                let v_s = cov_beta.clone();
                (beta_s, v_s, k)
            };
            let v_s_matrix = v_s.as_ref().to_owned();
            let beta_s_vector = beta_s.as_ref().to_owned();
            // Solve V_s * x = beta_s => x = V_s^{-1} * beta_s; then wald = beta_s' * x (more stable than explicit inverse)
            let x = v_s_matrix
                .as_ref()
                .checked_cholesky()
                .map_err(|_| "IV2SLS: V_s not pd for Wald".to_string())?
                .solve(&beta_s_vector.as_ref());
            let x_nd = x.as_ref();
            let wald = beta_s.transpose() * x_nd.as_ref();
            let chi2_dist =
                ChiSquared::new(df_wald as f64).map_err(|e| format!("IV2SLS Wald: {}", e))?;
            let wald_p = chi2_dist.sf(wald);
            (wald, wald_p)
        };

        Ok(IvEstimate {
            fitted: &self.endog - &u_structural,
            residuals: u_structural,
            betas: betas_nd,
            inference: RegressionCoefficientStatistics {
                covariance: covariance_rows(&cov_beta),
                standard_errors: std_err.iter().copied().collect(),
                statistic_values: z_values,
                p_values,
                confidence_interval_lower: ci_lower.iter().copied().collect(),
                confidence_interval_upper: ci_upper.iter().copied().collect(),
            },
            statistics: InstrumentalVariableStatistics {
                covariance_type,
                wald_chi2,
                wald_p_value: wald_p,
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
