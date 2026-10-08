use super::types::IV2SLS;
use yss_sci_linalg::{Mat, MatrixExt, Solve};

pub(super) struct PreparedIvDesign {
    pub z: Mat<f64>,
    pub ztz_inverse: Mat<f64>,
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

        // Stage 1: endog_hat = Z * (Z'Z)^{-1} Z' * endog for each endogenous
        let z_matrix = z.as_ref().to_owned();
        let ztz = z_matrix.transpose() * z_matrix.as_ref();
        let ztz_inv = ztz
            .checked_cholesky()
            .map_err(|_| {
                "IV2SLS: Z'Z is not positive definite (stage 1). Check instruments and exog for collinearity.".to_string()
            })?
            .solve(&Mat::identity(ztz.nrows(), ztz.nrows()));

        let mut endog_hat = Mat::zeros(n, k_endog);
        for j in 0..k_endog {
            let endog_col = self.endog_reg.col(j).to_owned();
            let endog_vector = endog_col.as_ref().to_owned();
            let zty = z_matrix.transpose() * endog_vector.as_ref();
            let gamma = ztz_inv.as_ref() * zty.as_ref();
            let hat = z_matrix.as_ref() * gamma.as_ref();
            let hat_arr = hat.as_ref().to_owned();
            for i in 0..n {
                endog_hat[(i, j)] = hat_arr[i];
            }
        }

        // Stage 2: X = [exog, endog_hat] (with constant)
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
                x_raw.push(endog_hat[(i, j)]);
            }
        }
        let x = yss_sci_linalg::MatRef::from_row_major_slice(&(x_raw), n, k_x).to_owned();

        let mut x_struct_raw = Vec::with_capacity(n * k_x);
        for i in 0..n {
            if self.options.constant {
                x_struct_raw.push(1.0);
            }
            for j in 0..k_exog {
                x_struct_raw.push(self.exog[(i, j)]);
            }
            for j in 0..k_endog {
                x_struct_raw.push(self.endog_reg[(i, j)]);
            }
        }
        let x_struct =
            yss_sci_linalg::MatRef::from_row_major_slice(&(x_struct_raw), n, k_x).to_owned();
        Ok(PreparedIvDesign {
            z,
            ztz_inverse: ztz_inv,
            endog_hat,
            x,
            x_struct,
        })
    }
}
