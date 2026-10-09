use super::model::IvModel;
use yss_sci_linalg::{ColRef, Mat, MatRef, MatrixExt, Solve};

pub(super) struct PreparedIvDesign {
    pub z: Mat<f64>,
    pub ztz_inverse: Mat<f64>,
    pub first_stage_coefficients: Mat<f64>,
    pub endog_hat: Mat<f64>,
    pub x: Mat<f64>,
    pub x_struct: Mat<f64>,
}

pub(super) fn prepare_instruments(
    observations: usize,
    columns: &[ColRef<'_, f64>],
    constant: bool,
) -> Result<(Mat<f64>, Mat<f64>), String> {
    let intercept = usize::from(constant);
    let z = Mat::from_fn(observations, columns.len() + intercept, |row, col| {
        if constant && col == 0 {
            1.0
        } else {
            columns[col - intercept][row]
        }
    });
    let ztz = z.transpose() * z.as_ref();
    let inverse = ztz
        .checked_cholesky()
        .map_err(|_| {
            "IV: Z'Z is not positive definite (stage 1). Check instruments and exog for collinearity.".to_string()
        })?
        .solve(&Mat::identity(ztz.nrows(), ztz.nrows()));
    Ok((z, inverse))
}

pub(super) fn project_endogenous(
    z: &Mat<f64>,
    inverse: &Mat<f64>,
    columns: &[ColRef<'_, f64>],
) -> (Mat<f64>, Mat<f64>) {
    let mut coefficients = Mat::zeros(z.ncols(), columns.len());
    let mut fitted = Mat::zeros(z.nrows(), columns.len());
    for (j, column) in columns.iter().enumerate() {
        let zty = z.transpose() * *column;
        let gamma = inverse.as_ref() * zty.as_ref();
        let hat = z.as_ref() * gamma.as_ref();
        for i in 0..z.ncols() {
            coefficients[(i, j)] = gamma[i];
        }
        for i in 0..z.nrows() {
            fitted[(i, j)] = hat[i];
        }
    }
    (coefficients, fitted)
}

pub(super) fn regressor_design(
    included: MatRef<'_, f64>,
    endogenous: &[ColRef<'_, f64>],
) -> Mat<f64> {
    Mat::from_fn(
        included.nrows(),
        included.ncols() + endogenous.len(),
        |row, col| {
            if col < included.ncols() {
                included[(row, col)]
            } else {
                endogenous[col - included.ncols()][row]
            }
        },
    )
}

impl IvModel {
    pub(super) fn design(&self) -> Result<PreparedIvDesign, String> {
        let n = self.endog.nrows();
        let k_exog = self.exog.ncols();
        let k_endog = self.endog_reg.ncols();
        let k_iv = self.instruments.ncols();

        if k_iv < k_endog {
            return Err(format!(
                "IV: underidentified — {} instruments < {} endogenous. Need at least {} instruments.",
                k_iv, k_endog, k_endog
            ));
        }

        let included = k_exog + usize::from(self.options.constant);
        let instrument_columns = self
            .exog
            .col_iter()
            .chain(self.instruments.col_iter())
            .collect::<Vec<_>>();
        let (z, ztz_inv) = prepare_instruments(n, &instrument_columns, self.options.constant)?;
        let endogenous_columns = self.endog_reg.col_iter().collect::<Vec<_>>();
        let (first_stage_coefficients, endog_hat) =
            project_endogenous(&z, &ztz_inv, &endogenous_columns);
        let included_design = z.subcols(0, included);
        let projected_columns = endog_hat.col_iter().collect::<Vec<_>>();
        let x = regressor_design(included_design, &projected_columns);
        let x_struct = regressor_design(included_design, &endogenous_columns);
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
