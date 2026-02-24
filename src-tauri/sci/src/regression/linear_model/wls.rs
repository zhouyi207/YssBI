use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray};
use faer::{Side, prelude::Solve};
use ndarray::{Array1, Array2};

pub struct WLS {
    pub endog: Array1<f64>,          // 因变量 y
    pub exog: Array2<f64>,           // 自变量 X (n × k)
    pub weights: Array1<f64>,        // 权重向量 w (长度 n)，每个观测的权重
}

pub struct WLSModel {
    pub params: Array1<f64>,         // 估计的系数 β
}

pub struct WLSResult {
    pub model: WLSModel,
}

impl WLS {
    pub fn fit(&self) -> WLSResult {
        // 1. 计算权重平方根：sqrt(w)
        let sqrt_weights = self.weights.mapv(|w| w.sqrt());

        // 2. whitening：对 y 和 X 做加权变换
        //    z = sqrt(w) * y
        //    Z = sqrt(w) * X   （逐行乘以 sqrt(w_i)）
        let mut z = self.endog.view().into_faer_col().to_owned();
        let mut zz = self.exog.view().into_faer().to_owned();

        // 对 z 逐元素乘以 sqrt(w_i)
        for (i, &sw) in sqrt_weights.iter().enumerate() {
            z[i] *= sw;
        }

        // 对 Z 每一行乘以 sqrt(w_i)
        for (i, mut row) in zz.row_iter_mut().enumerate() {
            let sw = sqrt_weights[i];
            row *= sw;
        }

        // 3. 对白化后的数据做普通最小二乘
        let xtx = zz.transpose() * zz.as_ref();           // Zᵀ Z
        let xtz = zz.transpose() * z.as_ref();           // Zᵀ z

        // XtX 是对称正定矩阵（假设 exog 满列秩）
        let llt = xtx.llt(Side::Lower).unwrap();
        let beta = llt.solve(xtz);

        // 4. 转回 ndarray
        let params = beta.as_ref().into_ndarray().to_owned();

        WLSResult {
            model: WLSModel { params },
        }
    }
}