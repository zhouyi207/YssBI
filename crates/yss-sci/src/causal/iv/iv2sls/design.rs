use super::types::IV2SLS;
use yss_sci_linalg::{Mat, MatrixExt, Solve};

pub(super) struct PreparedIvDesign {
    pub z: Mat<f64>,
    pub ztz_inverse: Mat<f64>,
    pub first_stage_coefficients: Mat<f64>,
    pub endog_hat: Mat<f64>,
    pub x: Mat<f64>,
    pub x_struct: Mat<f64>,
}

impl IV2SLS {
    pub(super) fn design(&self) -> Result<PreparedIvDesign, String> {
        let n = self.endog.nrows();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IV2SLS: underidentified — {} instruments < {} endogenous. Need at least {} instruments.",
                k_iv, k_endog, k_endog
            ));
        }

        // Z = [exog, instruments] for stage 1 (with constant if config.constant)
        let k_z = if self.options.constant {
            k_exog + k_iv + 1
        } else {
            k_exog + k_iv
        };
        let included = k_exog + usize::from(self.options.constant);
        let z = Mat::from_fn(n, k_z, |row, col| {
            if self.options.constant && col == 0 {
                1.0
            } else if col < included {
                self.exog[(row, col - usize::from(self.options.constant))]
            } else {
                self.instruments[(row, col - included)]
            }
        });

        // Stage 1: endog_hat = Z * (Z'Z)^{-1} Z' * endog for each endogenous
        let ztz = z.transpose() * z.as_ref();
        let ztz_inv = ztz
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: Z'Z is not positive definite (stage 1). Check instruments and exog for collinearity.".to_string()
            })?
            .solve(&Mat::identity(ztz.nrows(), ztz.nrows()));

        let mut endog_hat = Mat::zeros(n, k_endog);
        let mut first_stage_coefficients = Mat::zeros(k_z, k_endog);
        for j in 0..k_endog {
            let zty = z.transpose() * self.endog_reg.col(j);
            let gamma = ztz_inv.as_ref() * zty.as_ref();
            let hat = z.as_ref() * gamma.as_ref();
            for i in 0..k_z {
                first_stage_coefficients[(i, j)] = gamma[i];
            }
            for i in 0..n {
                endog_hat[(i, j)] = hat[i];
            }
        }

        // Stage 2: X = [exog, endog_hat] (with constant)
        let k_x = if self.options.constant {
            k_exog + k_endog + 1
        } else {
            k_exog + k_endog
        };
        let x = Mat::from_fn(n, k_x, |row, col| {
            if col < included {
                z[(row, col)]
            } else {
                endog_hat[(row, col - included)]
            }
        });
        let x_struct = Mat::from_fn(n, k_x, |row, col| {
            if col < included {
                z[(row, col)]
            } else {
                self.endog_reg[(row, col - included)]
            }
        });
        Ok(PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv,
            first_stage_coefficients,
            endog_hat,
            x,
            x_struct,
        })
    }
}
