use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};

pub struct GLSConfig {
    pub constant: bool,
}

pub struct GLS {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub sigma: Array2<f64>,
    pub config: GLSConfig,
}

#[derive(Debug)]
pub struct GLSModel {
    pub params: Array1<f64>,
}

#[derive(Debug)]
pub struct GLSResult {
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
    pub covariance_type: String,
    pub r2: f64,
    pub r2_adjusted: f64,
    pub fvalue: f64,
    pub f_p_value: f64,
    pub model: GLSModel,
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,
}

impl GLS {
    pub fn fit(&self) -> Result<GLSResult, String> {
        let l = self
            .sigma
            .view()
            .into_faer()
            .llt(Side::Lower)
            .map_err(|_| "GLS: Sigma is not positive definite".to_string())?
            .L()
            .to_owned();

        let mut endog = self.endog.view().into_faer_col().to_owned();
        let mut exog = self.exog.view().into_faer().to_owned();

        l.as_ref().solve_lower_triangular_in_place(endog.as_mut());
        l.as_ref().solve_lower_triangular_in_place(exog.as_mut());

        let (rank, cond_no) = matrix_rank(exog.as_ref().to_owned());
        let n = exog.nrows();
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let xtx = exog.as_ref().transpose() * exog.as_ref();
        let xty = exog.as_ref().transpose() * endog.as_ref();

        let xtx_inv = xtx
            .llt(Side::Lower)
            .map_err(|_| "GLS: X'Sigma^{-1}X is not positive definite".to_string())?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let betas = xtx_inv.as_ref() * xty;
        let y_hat = exog.as_ref() * betas.as_ref();

        let y_mean = endog.iter().mean();
        let ss_total = if self.config.constant {
            endog.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            endog.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = (endog.as_ref() - y_hat.as_ref()).iter().map(|v| v.powi(2)).sum::<f64>();
        let ss_model = ss_total - ss_residual;

        let r2 = 1.0 - ss_residual / ss_total;
        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = 1.0 - ms_residual / ms_total;
        let f = ms_model / ms_residual;

        let f_safe = f.max(0.0);
        let df1 = (df_model as f64).max(1.0);
        let df2 = (df_residual as f64).max(1.0);
        let dist = FisherSnedecor::new(df1, df2)
            .map_err(|e| format!("GLS: FisherSnedecor: {}", e))?;
        let f_p_value = 1.0 - dist.cdf(f_safe);

        let cov_beta = xtx_inv.as_ref().into_ndarray().to_owned();
        let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);
        let betas_nd = betas.as_ref().into_ndarray().to_owned();
        let t_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| b / se)
            .collect();

        let t_dist = StudentsT::new(0.0, 1.0, df_residual as f64)
            .map_err(|e| format!("GLS: StudentsT: {}", e))?;
        let p_values: Vec<f64> = t_values
            .iter()
            .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
            .collect();

        let t_crit = t_dist.inverse_cdf(0.975);
        let ci_lower = betas_nd.clone() - t_crit * std_err.clone();
        let ci_upper = betas_nd.clone() + t_crit * std_err.clone();

        Ok(GLSResult {
            num_observation: n,
            ss_model,
            ss_residual,
            ss_total,
            df_model,
            df_residual,
            df_total,
            ms_model,
            ms_residual,
            ms_total,
            covariance_type: "GLS (known Sigma)".to_string(),
            r2,
            r2_adjusted,
            fvalue: f,
            f_p_value,
            model: GLSModel {
                params: betas_nd.clone(),
            },
            betas: betas_nd,
            stds: std_err,
            tvalues: Array1::from_vec(t_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
        })
    }
}
