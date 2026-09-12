use super::{OLS, OlsFit, OlsFitError};
use faer::{Col, Mat};
use yss_linalg::{MatrixExt, Solve, matrix_rank};
pub(super) struct OlsSolution {
    pub x: Mat<f64>,
    pub y: Col<f64>,
    pub rank: usize,
    pub cond_no: f64,
    pub xtx_inv: Mat<f64>,
    pub betas: Col<f64>,
    pub y_hat: Col<f64>,
}

impl OLS {
    pub fn fit(&self) -> Result<OlsFit, OlsFitError> {
        let y = self.endog.as_ref().to_owned();
        let x = self.exog.as_ref().to_owned();

        let (rank, cond_no) = matrix_rank(x.as_ref()).unwrap_or((0, f64::INFINITY));

        // 普通最小二乘
        let xtx = x.transpose() * x.as_ref();
        let xty = x.transpose() * y.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| OlsFitError::NotPositiveDefinite)?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas = xtx_inv.as_ref() * xty.as_ref();
        let y_hat = x.as_ref() * betas.as_ref();

        super::inference::infer(
            self,
            OlsSolution {
                x,
                y,
                rank,
                cond_no,
                xtx_inv,
                betas,
                y_hat,
            },
        )
    }
}
