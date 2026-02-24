use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Side, prelude::Solve};
use ndarray::{Array1, Array2};

pub struct GLS {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub sigma: Array2<f64>,
}

pub struct GLSModel {
    params: Array1<f64>,
}

pub struct GLSResult {
    pub model: GLSModel,
}

impl GLS {
    pub fn fit(&self) -> GLSResult {
        // 計算 Cholesky 因子 L
        let l = &self
            .sigma
            .view()
            .into_faer()
            .llt(Side::Lower)
            .unwrap()
            .L()
            .to_owned();

        // whitening: z = L⁻¹ y   Z = L⁻¹ X
        let mut endog = self.endog.view().into_faer_col().to_owned();
        let mut exog = self.exog.view().into_faer().to_owned();

        l.as_ref().solve_lower_triangular_in_place(endog.as_mut());
        l.as_ref().solve_lower_triangular_in_place(exog.as_mut());

        // 現在對白化後的資料做普通最小二乘
        let xtx = exog.as_ref().transpose() * exog.as_ref();
        let xty = exog.as_ref().transpose() * endog.as_ref();

        // xtx 是实对称矩阵
        let beta = xtx.llt(Side::Lower).unwrap().solve(xty);

        let params = beta.as_ref().into_ndarray().to_owned();

        GLSResult {
            model: GLSModel { params },
        }
    }
}
