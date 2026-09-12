use faer::{Col, Mat};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};
use yss_linalg::matrix_rank;
use yss_linalg::{MatrixExt, Solve};

pub struct GLSConfig {
    pub constant: bool,
}

pub struct GLS {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub sigma: Mat<f64>,
    pub config: GLSConfig,
}

#[derive(Debug)]
pub struct GLSModel {
    pub params: Col<f64>,
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
    pub betas: Col<f64>,
    pub stds: Col<f64>,
    pub tvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,
    pub conf_int_right: Col<f64>,
    pub cov_beta: Mat<f64>,
    pub cond_no: f64,
}

impl GLS {
    pub fn fit(&self) -> Result<GLSResult, String> {
        let l = self
            .sigma
            .as_ref()
            .checked_cholesky()
            .map_err(|_| "GLS: Sigma is not positive definite".to_string())?
            .lower()
            .to_owned();

        let mut endog = self.endog.as_ref().to_owned();
        let mut exog = self.exog.as_ref().to_owned();

        l.as_ref().solve_lower_triangular_in_place(endog.as_mut());
        l.as_ref().solve_lower_triangular_in_place(exog.as_mut());

        let (rank, cond_no) = matrix_rank(exog.as_ref()).unwrap_or((0, f64::INFINITY));
        let n = exog.nrows();
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let xtx = exog.transpose() * exog.as_ref();
        let xty = exog.transpose() * endog.as_ref();

        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| "GLS: X'Sigma^{-1}X is not positive definite".to_string())?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas = xtx_inv.as_ref() * xty.as_ref();
        let y_hat = exog.as_ref() * betas.as_ref();

        let y_mean = endog.iter().mean();
        let ss_total = if self.config.constant {
            endog.iter().map(|v| (v - y_mean).powi(2)).sum::<f64>()
        } else {
            endog.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = (endog.as_ref() - y_hat.as_ref())
            .iter()
            .map(|v| v.powi(2))
            .sum::<f64>();
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
        let dist =
            FisherSnedecor::new(df1, df2).map_err(|e| format!("GLS: FisherSnedecor: {}", e))?;
        let f_p_value = 1.0 - dist.cdf(f_safe);

        let cov_beta = xtx_inv.as_ref().to_owned();
        let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());
        let betas_nd = betas.as_ref().to_owned();
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
        let ci_lower = betas_nd.clone() - faer::Scale(t_crit) * std_err.clone();
        let ci_upper = betas_nd.clone() + faer::Scale(t_crit) * std_err.clone();

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
            tvalues: (t_values).into_iter().collect::<Col<f64>>(),
            pvalues: (p_values).into_iter().collect::<Col<f64>>(),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
        })
    }
}
