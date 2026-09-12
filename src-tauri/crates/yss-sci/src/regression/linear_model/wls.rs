use crate::regression::covariance::compute_cov_beta;
use yss_sci_contract::regression::CovParams;

use faer::{Col, Mat};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};
use yss_linalg::matrix_rank;
use yss_linalg::{MatrixExt, Solve};

pub struct WLSConfig {
    pub constant: bool,
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
}

pub struct WLS {
    pub endog: Col<f64>,
    pub exog: Mat<f64>,
    pub weights: Col<f64>,
    pub config: WLSConfig,
}

#[derive(Debug)]
pub struct WLSModel {
    pub params: Col<f64>,
}

#[derive(Debug)]
pub struct WLSResult {
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
    pub model: WLSModel,
    pub betas: Col<f64>,
    pub stds: Col<f64>,
    pub tvalues: Col<f64>,
    pub pvalues: Col<f64>,
    pub conf_int_left: Col<f64>,
    pub conf_int_right: Col<f64>,
    pub cov_beta: Mat<f64>,
    pub cond_no: f64,
}

impl WLS {
    pub fn fit(&self) -> Result<WLSResult, String> {
        let sqrt_weights = self.weights.map(|&w| w.sqrt());

        let mut z = self.endog.as_ref().to_owned();
        let mut zz = self.exog.as_ref().to_owned();

        for (i, &sw) in sqrt_weights.iter().enumerate() {
            z[i] *= sw;
        }
        for (i, mut row) in zz.row_iter_mut().enumerate() {
            let sw = sqrt_weights[i];
            row *= faer::Scale(sw);
        }

        let (rank, cond_no) = matrix_rank(zz.as_ref()).unwrap_or((0, f64::INFINITY));
        let n = zz.nrows();
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        let xtx = zz.transpose() * zz.as_ref();
        let xtz = zz.transpose() * z.as_ref();
        let xtx_inv = xtx
            .checked_cholesky()
            .map_err(|_| "WLS: X'WX matrix is not positive definite".to_string())?
            .solve(&Mat::identity(xtx.nrows(), xtx.nrows()));
        let betas = xtx_inv.as_ref() * xtz.as_ref();
        let z_hat = zz.as_ref() * betas.as_ref();

        let z_mean = z.iter().mean();
        let ss_total = if self.config.constant {
            z.iter().map(|v| (v - z_mean).powi(2)).sum::<f64>()
        } else {
            z.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = (z.as_ref() - z_hat.as_ref())
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
            FisherSnedecor::new(df1, df2).map_err(|e| format!("WLS: FisherSnedecor: {}", e))?;
        let f_p_value = 1.0 - dist.cdf(f_safe);

        let u = &z - z_hat.as_ref();
        let x_nd = zz.as_ref().to_owned();
        let xtx_inv_nd = xtx_inv.as_ref().to_owned();
        let u_nd: Col<f64> = u.as_ref().to_owned();

        let cov_beta = compute_cov_beta(
            &x_nd,
            &xtx_inv_nd,
            &u_nd,
            df_residual,
            &covariance_type,
            self.config.cov_params.as_ref(),
        )?;

        let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());
        let betas_nd = betas.as_ref().to_owned();
        let t_values: Vec<f64> = betas_nd
            .iter()
            .zip(std_err.iter())
            .map(|(b, se)| b / se)
            .collect();

        let t_dist = StudentsT::new(0.0, 1.0, df_residual as f64)
            .map_err(|e| format!("WLS: StudentsT: {}", e))?;
        let p_values: Vec<f64> = t_values
            .iter()
            .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
            .collect();

        let t_crit = t_dist.inverse_cdf(0.975);
        let ci_lower = betas_nd.clone() - faer::Scale(t_crit) * std_err.clone();
        let ci_upper = betas_nd.clone() + faer::Scale(t_crit) * std_err.clone();

        Ok(WLSResult {
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
            covariance_type,
            r2,
            r2_adjusted,
            fvalue: f,
            f_p_value,
            model: WLSModel {
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
