//! Limited-information maximum likelihood using the shared IV design.
use super::{
    IvEstimate, IvModel,
    design::PreparedIvDesign,
    estimate::{coefficient_inference, model_test},
};
use crate::regression::covariance::compute_cov_beta;
use statrs::statistics::Statistics;
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
            x: projected_x,
            x_struct: x,
            ..
        } = self.design()?;
        let (rank, cond_no) = matrix_rank(x.as_ref()).map_err(|error| error.to_string())?;
        if rank == 0 || rank < x.ncols() {
            return Err("Design matrix is rank deficient".into());
        }
        if n <= rank {
            return Err("Insufficient residual degrees of freedom".into());
        }
        let df_residual = n - rank;
        let df_model = rank - usize::from(self.options.constant);
        let df_total = df_residual + df_model;

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
        let yty = y_tilde.transpose() * y_tilde.as_ref();
        let zty = z.transpose() * y_tilde.as_ref();
        let ytmz: Mat<f64> =
            &yty - &((zty.transpose() * ztz_inverse.as_ref()).as_ref() * zty.as_ref());
        let x1ty = x1.transpose() * y_tilde.as_ref();
        let ytmx1: Mat<f64> =
            &yty - &((x1ty.transpose() * x1tx1_inverse.as_ref()).as_ref() * x1ty.as_ref());

        // The smaller generalized eigenvalue is the LIML k-class parameter.
        let evd = yss_sci_linalg::SymmetricEigen::factor(ytmz.as_ref())
            .map_err(|_| "IVLIML: EVD of Ỹ'MZ Ỹ failed".to_string())?;
        let values = evd.values();
        let vectors = evd.vectors();
        let size = k_endog + 1;
        let mut inverse_sqrt = Mat::zeros(size, size);
        for i in 0..size {
            if values[i] > 1e-12 {
                inverse_sqrt[(i, i)] = 1.0 / values[i].sqrt();
            }
        }
        let whitening = (vectors.as_ref() * inverse_sqrt.as_ref()).as_ref() * vectors.transpose();
        let g = whitening.as_ref() * ytmx1.as_ref() * whitening.as_ref();
        let evd_g = yss_sci_linalg::SymmetricEigen::factor(g.as_ref())
            .map_err(|_| "IVLIML: EVD of G failed".to_string())?;
        let kappa = evd_g
            .values()
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .max(0.0);

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
        let ss_residual = residuals.transpose() * residuals.as_ref();
        let y_mean = self.endog.iter().mean();
        let ss_total = if self.options.constant {
            self.endog.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            self.endog.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let r2 = if ss_total > 1e-300 {
            1.0 - ss_residual / ss_total
        } else {
            0.0
        };
        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let adjusted_r2 = if ms_total > 1e-300 {
            1.0 - ms_residual / ms_total
        } else {
            0.0
        };
        let sigma2_df = if self.small { df_residual } else { n };
        // LIML uses the k-class bread with instrument-projected score rows,
        // while the residuals remain structural (Stata ivregress VCE formula).
        let covariance = compute_cov_beta(
            &projected_x,
            &k_class_inverse,
            &residuals,
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;
        let inference = coefficient_inference(&betas, &covariance, df_residual, self.small)?;
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
