//! IV:LIML (Limited Information Maximum Likelihood)
//!
//! Stata ivregress liml: depvar [varlist1] (varlist2 = varlistiv)
//! κ-class estimator with κ = minimum eigenvalue of (Ỹ'MZ Ỹ)^{-1/2} Ỹ'MX1 Ỹ (Ỹ'MZ Ỹ)^{-1/2}
//! β̂ = {X'(I − κMZ)X}^{-1} X'(I − κMZ)y

use crate::causal::iv::estimate::{coefficient_inference, model_test};
use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::causal::iv::InstrumentalVariableStatistics;
use yss_sci_contract::regression::OlsOptions;

use statrs::statistics::Statistics;
use yss_sci_linalg::matrix_rank;
use yss_sci_linalg::{Col, Mat};
use yss_sci_linalg::{MatrixExt, Solve};

/// IV:LIML 数值输入，复用共享 OLS 选项。
pub struct IVLIML {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub endog_reg: Mat<f64>,
    pub instruments: Mat<f64>,
    pub options: OlsOptions,
    /// Stata small: if true, use ESS/(n-k) for σ²; otherwise ESS/n.
    pub small: bool,
}

impl IVLIML {
    pub fn fit(&self) -> Result<crate::causal::iv::IvEstimate, String> {
        let n = self.endog.nrows();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IVLIML: underidentified — {} instruments < {} endogenous.",
                k_iv, k_endog
            ));
        }

        let k_z = if self.options.constant {
            k_exog + k_iv + 1
        } else {
            k_exog + k_iv
        };
        let k1 = if self.options.constant {
            k_exog + 1
        } else {
            k_exog
        };

        // Z = [const?, exog, instruments]
        let mut z_raw = Vec::with_capacity(n * k_z);
        for i in 0..n {
            if self.options.constant {
                z_raw.push(1.0);
            }
            for j in 0..k_exog {
                z_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_iv {
                z_raw.push(self.instruments[(i, j)]);
            }
        }
        let z = yss_sci_linalg::MatRef::from_row_major_slice(&(z_raw), n, k_z).to_owned();

        // X1 = [const?, exog]
        let mut x1_raw = Vec::with_capacity(n * k1);
        for i in 0..n {
            if self.options.constant {
                x1_raw.push(1.0);
            }
            for j in 0..k_exog {
                x1_raw.push(self.exog[(i, j)]);
            }
        }
        let x1 = yss_sci_linalg::MatRef::from_row_major_slice(&(x1_raw), n, k1).to_owned();

        // X = [const?, exog, endog_reg] (structural)
        let k_x = if self.options.constant {
            k_exog + k_endog + 1
        } else {
            k_exog + k_endog
        };
        let mut x_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.options.constant {
                x_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_endog {
                x_raw.push(self.endog_reg[(i, j)]);
            }
        }
        let x = yss_sci_linalg::MatRef::from_row_major_slice(&(x_raw), n, k_x).to_owned();

        // Ỹ = [y Y] (n × (p+1))
        let mut y_tilde_raw = Vec::with_capacity(n * (k_endog + 1));
        for i in 0..n {
            y_tilde_raw.push(self.endog[i]);
            for j in 0..k_endog {
                y_tilde_raw.push(self.endog_reg[(i, j)]);
            }
        }
        let y_tilde =
            yss_sci_linalg::MatRef::from_row_major_slice(&(y_tilde_raw), n, k_endog + 1).to_owned();

        let z_matrix = z.as_ref().to_owned();
        let x1_matrix = x1.as_ref().to_owned();
        let y_tilde_matrix = y_tilde.as_ref().to_owned();
        let x_matrix = x.as_ref().to_owned();
        let y_vector = self.endog.as_ref().to_owned();

        // Z'Z, (Z'Z)^{-1}
        let ztz = z_matrix.transpose() * z_matrix.as_ref();
        let ztz_inv = ztz
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "IVLIML: Z'Z not pd".to_string())?
            .solve(&Mat::identity(ztz.nrows(), ztz.nrows()));
        let ztz_inv_nd = ztz_inv.as_ref().to_owned();

        // X1'X1, (X1'X1)^{-1}
        let x1tx1 = x1_matrix.transpose() * x1_matrix.as_ref();
        let x1tx1_inv = x1tx1
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "IVLIML: X1'X1 not pd".to_string())?
            .solve(&Mat::identity(x1tx1.nrows(), x1tx1.nrows()));
        let x1tx1_inv_nd = x1tx1_inv.as_ref().to_owned();

        // Ỹ'MZ Ỹ = Ỹ'Ỹ - Ỹ'Z(Z'Z)^{-1}Z'Ỹ
        let yty = y_tilde_matrix.transpose() * y_tilde_matrix.as_ref();
        let zty = z_matrix.transpose() * y_tilde_matrix.as_ref();
        let ytmz = yty.as_ref().to_owned();
        let zty_nd = zty.as_ref().to_owned();
        let ytmz_nd: Mat<f64> =
            &ytmz - &((zty_nd.transpose() * ztz_inv_nd.as_ref()).as_ref() * zty_nd.as_ref());

        // Ỹ'MX1 Ỹ = Ỹ'Ỹ - Ỹ'X1(X1'X1)^{-1}X1'Ỹ
        let x1ty = x1_matrix.transpose() * y_tilde_matrix.as_ref();
        let x1ty_nd = x1ty.as_ref().to_owned();
        let ytmx1_nd: Mat<f64> =
            &ytmz - &((x1ty_nd.transpose() * x1tx1_inv_nd.as_ref()).as_ref() * x1ty_nd.as_ref());

        // G = (Ỹ'MZ Ỹ)^{-1/2} Ỹ'MX1 Ỹ (Ỹ'MZ Ỹ)^{-1/2}
        let evd = yss_sci_linalg::SymmetricEigen::factor(ytmz_nd.as_ref())
            .map_err(|_| "IVLIML: EVD of Ỹ'MZ Ỹ failed".to_string())?;
        let s_col = evd.values();
        let u = evd.vectors();
        let size = k_endog + 1;
        let mut lambda_inv_sqrt = Mat::zeros(size, size);
        for i in 0..size {
            let si = s_col[i];
            if si > 1e-12 {
                lambda_inv_sqrt.as_mut()[(i, i)] = 1.0 / si.sqrt();
            }
        }
        let ytmz_inv_sqrt = (u.as_ref() * lambda_inv_sqrt.as_ref()).as_ref() * u.transpose();
        let g = ytmz_inv_sqrt.as_ref() * ytmx1_nd.as_ref() * ytmz_inv_sqrt.as_ref();

        // κ = minimum eigenvalue of G
        let g_nd = g.as_ref().to_owned();
        let evd_g = yss_sci_linalg::SymmetricEigen::factor(g_nd.as_ref())
            .map_err(|_| "IVLIML: EVD of G failed".to_string())?;
        let s_g = evd_g.values();
        let kappa = s_g.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);

        // β̂ = {X'(I − κMZ)X}^{-1} X'(I − κMZ)y
        // X'(I−κMZ)X = (1-κ)X'X + κ X'Z(Z'Z)^{-1}Z'X
        // X'(I−κMZ)y = (1-κ)X'y + κ X'Z(Z'Z)^{-1}Z'y
        let xtx = x_matrix.transpose() * x_matrix.as_ref();
        let xty = x_matrix.transpose() * y_vector.as_ref();
        let xtz = x_matrix.transpose() * z_matrix.as_ref();
        let ztx = z_matrix.transpose() * x_matrix.as_ref();
        let zty_y = z_matrix.transpose() * y_vector.as_ref();

        let xtx_nd = xtx.as_ref().to_owned();
        let xty_nd = xty.as_ref().to_owned();
        let xtz_nd = xtz.as_ref().to_owned();
        let ztx_nd = ztx.as_ref().to_owned();
        let zty_y_nd = zty_y.as_ref().to_owned();

        let xt_ikmz_x_nd: Mat<f64> = yss_sci_linalg::Scale(1.0 - kappa) * &xtx_nd
            + yss_sci_linalg::Scale(kappa)
                * ((xtz_nd.as_ref() * ztz_inv_nd.as_ref()).as_ref() * ztx_nd.as_ref());
        let xt_ikmz_y_nd: Col<f64> = yss_sci_linalg::Scale(1.0 - kappa) * &xty_nd
            + yss_sci_linalg::Scale(kappa)
                * ((xtz_nd.as_ref() * ztz_inv_nd.as_ref()).as_ref() * zty_y_nd.as_ref());

        let xt_ikmz_x_matrix = xt_ikmz_x_nd.as_ref().to_owned();
        let xt_ikmz_y_vector = xt_ikmz_y_nd.as_ref().to_owned();
        let xt_ikmz_x_inv = xt_ikmz_x_matrix
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "IVLIML: X'(I−κMZ)X not pd".to_string())?
            .solve(&Mat::identity(xt_ikmz_x_nd.nrows(), xt_ikmz_x_nd.nrows()));
        let betas_vector = xt_ikmz_x_inv.as_ref() * xt_ikmz_y_vector.as_ref();
        let betas_nd = betas_vector.as_ref().to_owned();

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

        let u_structural: Col<f64> = &self.endog - &(x.as_ref() * betas_nd.as_ref());
        let ss_residual = u_structural.transpose() * u_structural.as_ref();
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
        let r2_adjusted = if ms_total > 1e-300 {
            1.0 - ms_residual / ms_total
        } else {
            0.0
        };

        let sigma2_df = if self.small { df_residual } else { n };
        let cov_beta = compute_cov_beta(
            &x_matrix,
            &xt_ikmz_x_inv,
            &u_structural,
            sigma2_df,
            self.options.constant.then_some(0),
            &self.options.covariance,
        )?;

        let inference = coefficient_inference(&betas_nd, &cov_beta, df_residual, self.small)?;
        let covariance_type = self.options.covariance.name().to_owned();
        let model_test = model_test(
            &betas_nd,
            &cov_beta,
            self.options.constant,
            df_residual,
            self.small,
        )?;

        Ok(crate::causal::iv::IvEstimate {
            fitted: &self.endog - &u_structural,
            residuals: u_structural,
            betas: betas_nd,
            inference,
            statistics: InstrumentalVariableStatistics {
                covariance_type,
                model_test,
                observations: n,
                df_residual,
                r2,
                adjusted_r2: r2_adjusted,
                condition_number: cond_no,
                kappa,
            },
        })
    }
}
