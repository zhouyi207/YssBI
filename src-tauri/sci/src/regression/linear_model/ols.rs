use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use num_traits::Pow;
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};

pub struct OLSConfig {
    pub constant: bool,
}

pub struct OLS {
    pub endog: Array1<f64>, // 因变量 y
    pub exog: Array2<f64>,  // 自变量 X (n × k)
    pub config: OLSConfig
}

#[derive(Debug)]
pub struct OLSModel {
    pub params: Array1<f64>, // 估计的系数 β
}

#[derive(Debug)]
pub struct OLSResult {
    pub num_observation: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: &'static str,
    pub r2: f64,
    pub r2_adjusted: f64,
    pub fvalue: f64,
    pub f_p_value: f64,

    pub model: OLSModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,  // 置信区间左侧
    pub conf_int_right: Array1<f64>, // 置信区间右侧

    // 矩阵是否病态（多重共线性）
    pub cond_no: f64,
}

impl OLS {
    pub fn fit(&self) -> Result<OLSResult, String> {
        let y = self.endog.view().into_faer_col().to_owned();
        let x = self.exog.view().into_faer().to_owned();

        let (rank, cond_no) = matrix_rank(x.as_ref().to_owned());

        let num_obversion = x.nrows();
        let df_redidual = num_obversion - rank;
        let df_model = if self.config.constant {
            rank - 1
        } else {
            rank
        };
        let df_total = df_redidual + df_model;

        let covariance_type = "nonrobust";

        // 普通最小二乘
        let xtx = x.transpose() * x.as_ref();
        let xty = x.transpose() * y.as_ref();
        let xtx_inv = xtx
            .llt(Side::Lower)
            .map_err(|_| "OLS: X'X matrix is not positive definite (likely rank-deficient or has multicollinearity). Check your input variables.".to_string())?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let betas = xtx_inv.as_ref() * xty;
        let y_hat = x.as_ref() * betas.as_ref();

        let y_mean = y.iter().mean();


        let ss_total = if self.config.constant {
            y.iter().map(|v| (v - y_mean).pow(2)).sum::<f64>()
        } else {
            y.iter().map(|v| v.pow(2)).sum::<f64>()
        };
        let ss_residual = (y.as_ref() - y_hat.as_ref()).iter().map(|v| v.pow(2)).sum::<f64>();
        let ss_model = ss_total - ss_residual;

        let r2 = 1.0 - ss_residual / ss_total;

        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_redidual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = 1.0 - ms_residual / ms_total;

        let f = ms_model / ms_residual;

        let dist = FisherSnedecor::new(df_model as f64, df_redidual as f64).unwrap();
        let f_p_value = 1.0 - dist.cdf(f);
        
        // 残差方差
        let u = y - y_hat.as_ref();
        let sigma2 = (u.transpose() * u.as_ref()) / df_redidual as f64;

        // 参数协方差矩阵
        let cov_beta = sigma2 * xtx_inv.as_ref();

        // std err
        let std_err = cov_beta
            .diagonal()
            .column_vector()
            .map(|v| v.sqrt())
            .to_owned();

        // t value
        let t_values: Vec<f64> = betas
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| b / se)
            .collect();

        let t_dist = StudentsT::new(0.0, 1.0, df_redidual as f64).unwrap();
        let p_values = t_values
            .iter()
            .map(|t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
            .collect::<Vec<f64>>();

        let t_cirt = t_dist.inverse_cdf(0.975);
        let ci_lower = betas.as_ref() - t_cirt * std_err.as_ref();
        let ci_upper = betas.as_ref() + t_cirt * std_err.as_ref();

        Ok(OLSResult {
            num_observation: num_obversion,
            ss_model,
            ss_residual,
            ss_total,
            df_model,
            df_residual: df_redidual,
            df_total,
            ms_model,
            ms_residual,
            ms_total,
            covariance_type,
            r2,
            r2_adjusted,
            fvalue: f,
            f_p_value,
            model: OLSModel {
                params: betas.as_ref().into_ndarray().to_owned(),
            },
            betas: betas.as_ref().into_ndarray().to_owned(),
            stds: std_err.as_ref().into_ndarray().to_owned(),
            tvalues: Array1::from_vec(t_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower.as_ref().into_ndarray().to_owned(),
            conf_int_right: ci_upper.as_ref().into_ndarray().to_owned(),
            cond_no,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_ols_with_iris() {
        // 读取 iris.csv 数据
        let mut rdr = csv::Reader::from_path("tests/iris.csv").unwrap();

        let mut sepal_length = Vec::new();
        let mut sepal_width = Vec::new();
        let mut petal_length = Vec::new();
        let mut petal_width = Vec::new();

        for result in rdr.records() {
            let record = result.unwrap();
            sepal_length.push(record[0].parse::<f64>().unwrap());
            sepal_width.push(record[1].parse::<f64>().unwrap());
            petal_length.push(record[2].parse::<f64>().unwrap());
            petal_width.push(record[3].parse::<f64>().unwrap());
        }

        let n = sepal_length.len();

        let mut exog_data = Vec::with_capacity(n * 4);

        for i in 0..n {
            exog_data.push(1.0);
            exog_data.push(sepal_width[i]);
            exog_data.push(petal_length[i]);
            exog_data.push(petal_width[i]);
        }

        let exog = Array2::from_shape_vec((n, 4), exog_data).unwrap();
        let endog = Array1::from_vec(sepal_length);

        let ols_config = OLSConfig { constant: true};
        let ols = OLS { endog, exog, config: ols_config};
        let result = ols.fit().unwrap();

        // 打印结果
        println!("回归系数 (betas): {:?}", result.betas);
        println!("标准误 (std errors): {:?}", result.stds);
        println!("t值 (t-values): {:?}", result.tvalues);
        println!("p值 (p-values): {:?}", result.pvalues);
        println!("置信区间下限: {:?}", result.conf_int_left);
        println!("置信区间上限: {:?}", result.conf_int_right);
        println!("cond: {:?}", result.cond_no);

        // 基本断言：确保系数数量正确
        assert_eq!(result.betas.len(), 4);
        assert_eq!(result.stds.len(), 4);
        assert_eq!(result.tvalues.len(), 4);
        assert_eq!(result.pvalues.len(), 4);

        // 验证所有 p 值都在 [0, 1] 范围内
        for &p in result.pvalues.iter() {
            assert!(p >= 0.0 && p <= 1.0, "p-value should be between 0 and 1");
        }
    }
}
