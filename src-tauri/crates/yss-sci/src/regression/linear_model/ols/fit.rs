use super::{OLS, OlsFit, OlsFitError};
use ndarray::{Array1, Array2};
use yss_linalg::{MatMul, MatrixExt, Solve, matrix_rank};
pub(super) struct OlsSolution {
    pub x: Array2<f64>,
    pub y: Array1<f64>,
    pub rank: usize,
    pub cond_no: f64,
    pub xtx_inv: Array2<f64>,
    pub betas: Array1<f64>,
    pub y_hat: Array1<f64>,
}

impl OLS {
    pub fn fit(&self) -> Result<OlsFit, OlsFitError> {
        let y = self.endog.view().to_owned();
        let x = self.exog.view().to_owned();

        let (rank, cond_no) = matrix_rank(x.view()).unwrap_or((0, f64::INFINITY));

        // 普通最小二乘
        let xtx = x.t().matmul(&x.view());
        let xty = x.t().matmul(&y.view());
        let xtx_inv = xtx
            .cholesky()
            .map_err(|_| OlsFitError::NotPositiveDefinite)?
            .solve(&ndarray::Array2::<f64>::eye(xtx.nrows()));
        let betas = xtx_inv.view().matmul(&xty);
        let y_hat = x.view().matmul(&betas.view());

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
