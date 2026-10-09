//! Limited-information maximum likelihood using the shared IV design.
use super::{
    IvEstimate, IvModel,
    design::{PreparedIvDesign, regressor_design},
    estimate::{coefficient_inference, goodness_of_fit, model_test},
};
use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::causal::iv::InstrumentalVariableStatistics;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

impl IvModel {
    pub fn fit_liml(&self) -> Result<IvEstimate, String> {
        let n = self.endog.nrows();
        let k_endog = self.endog_reg.ncols();
        let included = self.exog.ncols() + usize::from(self.options.constant);
        let PreparedIvDesign {
            z,
            ztz_inverse,
            endog_hat,
            ..
        } = self.design()?;
        let included_design = z.subcols(0, included);
        let projected_x =
            regressor_design(included_design, &endog_hat.col_iter().collect::<Vec<_>>());
        let x = regressor_design(
            included_design,
            &self.endog_reg.col_iter().collect::<Vec<_>>(),
        );
        let (rank, cond_no) = matrix_rank(x.as_ref()).map_err(|error| error.to_string())?;
        if rank == 0 || rank < x.ncols() {
            return Err("Design matrix is rank deficient".into());
        }
        if n <= rank {
            return Err("Insufficient residual degrees of freedom".into());
        }
        let df_residual = n - rank;

        let x1 = z.subcols(0, included);
        let y_tilde = Mat::from_fn(n, k_endog + 1, |row, column| {
            if column == 0 {
                self.endog[row]
            } else {
                self.endog_reg[(row, column - 1)]
            }
        });
        let x1tx1 = x1.transpose() * x1;
        let x1tx1_inverse = x1tx1
            .checked_cholesky()
            .map_err(|_| "IVLIML: X1'X1 not pd".to_string())?
            .solve(&Mat::identity(included, included));
        let response_hat = z.as_ref()
            * (ztz_inverse.as_ref() * (z.transpose() * self.endog.as_ref()).as_ref()).as_ref();
        let mz_y = Mat::from_fn(n, k_endog + 1, |row, column| {
            if column == 0 {
                self.endog[row] - response_hat[row]
            } else {
                self.endog_reg[(row, column - 1)] - endog_hat[(row, column - 1)]
            }
        });
        let x1ty = x1.transpose() * y_tilde.as_ref();
        let mx1_y = y_tilde.as_ref() - (x1 * (x1tx1_inverse.as_ref() * x1ty.as_ref())).as_ref();
        let ytmz = mz_y.transpose() * mz_y.as_ref();
        let ytmx1 = mx1_y.transpose() * mx1_y.as_ref();

        // Reverse the eigenproblem: MZ can be singular, while MX1 is the
        // positive-definite denominator. The largest reciprocal root gives kappa.
        let size = k_endog + 1;
        let lower = ytmx1
            .checked_cholesky()
            .map_err(|_| "IVLIML: Ỹ'MX1 Ỹ not positive definite".to_string())?
            .lower();
        let mut inverse_lower = Mat::identity(size, size);
        lower
            .as_ref()
            .solve_lower_triangular_in_place(inverse_lower.as_mut());
        let g = inverse_lower.as_ref() * ytmz.as_ref() * inverse_lower.transpose();
        let evd_g = yss_sci_linalg::SymmetricEigen::factor(g.as_ref())
            .map_err(|_| "IVLIML: EVD of G failed".to_string())?;
        let reciprocal_kappa = evd_g
            .values()
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if !reciprocal_kappa.is_finite() || reciprocal_kappa <= 0.0 {
            return Err("IVLIML: k-class root is undefined".into());
        }
        // MZ <= MX1 implies kappa >= 1; retain that boundary under roundoff.
        let kappa = reciprocal_kappa.recip().max(1.0);
        if !kappa.is_finite() {
            return Err("IVLIML: k-class root is nonfinite".into());
        }

        // X'(I-κMZ)X = (1-κ)X'X + κ Xhat'Xhat; Xhat=PZ X.
        let xtx = x.transpose() * x.as_ref();
        let projected_xtx = projected_x.transpose() * projected_x.as_ref();
        let k_class_cross: Mat<f64> =
            yss_sci_linalg::Scale(1.0 - kappa) * xtx + yss_sci_linalg::Scale(kappa) * projected_xtx;
        let xty = x.transpose() * self.endog.as_ref();
        let projected_xty = projected_x.transpose() * self.endog.as_ref();
        let k_class_response: Col<f64> =
            yss_sci_linalg::Scale(1.0 - kappa) * xty + yss_sci_linalg::Scale(kappa) * projected_xty;
        let k_class_inverse = k_class_cross
            .checked_cholesky()
            .map_err(|_| "IVLIML: X'(I−κMZ)X not pd".to_string())?
            .solve(&Mat::identity(rank, rank));
        let betas = k_class_inverse.as_ref() * k_class_response.as_ref();

        let residuals: Col<f64> = &self.endog - &(x.as_ref() * betas.as_ref());
        let (r2, adjusted_r2) = goodness_of_fit(
            self.endog.as_ref(),
            residuals.as_ref(),
            self.options.constant,
            df_residual,
        )?;
        let sigma2_df = if self.small { df_residual } else { n };
        // LIML uses the k-class bread with instrument-projected score rows,
        // while the residuals remain structural (Stata ivregress VCE formula).
        let covariance = compute_cov_beta(
            &projected_x,
            &k_class_inverse,
            residuals.as_ref(),
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;
        let inference =
            coefficient_inference(betas.as_ref(), &covariance, df_residual, self.small)?;
        let model_test = model_test(
            &betas,
            &covariance,
            self.options.constant,
            df_residual,
            self.small,
        )?;
        Ok(IvEstimate {
            fitted: &self.endog - &residuals,
            residuals,
            betas,
            inference,
            statistics: InstrumentalVariableStatistics {
                covariance_type: self.options.covariance.name().to_owned(),
                model_test,
                observations: n,
                df_residual,
                r2,
                adjusted_r2,
                condition_number: cond_no,
                kappa,
            },
        })
    }
}
