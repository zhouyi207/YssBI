use crate::regression::covariance::{compute_cov_beta, CovParams};
use crate::tools::{IntoFaer, IntoFaerCol, IntoNdarray, matrix_rank};
use faer::{Mat, Side, linalg::solvers::Solve};
use ndarray::{Array1, Array2};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};

pub struct WLSConfig {
    pub constant: bool,
    pub cov_type: String,
    pub cov_params: Option<CovParams>,
}

pub struct WLS {
    pub endog: Array1<f64>,
    pub exog: Array2<f64>,
    pub weights: Array1<f64>,
    pub config: WLSConfig,
}

#[derive(Debug)]
pub struct WLSModel {
    pub params: Array1<f64>,
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
    pub betas: Array1<f64>,
    pub stds: Array1<f64>,
    pub tvalues: Array1<f64>,
    pub pvalues: Array1<f64>,
    pub conf_int_left: Array1<f64>,
    pub conf_int_right: Array1<f64>,
    pub cov_beta: Array2<f64>,
    pub cond_no: f64,
}

impl WLS {
    pub fn fit(&self) -> Result<WLSResult, String> {
        let sqrt_weights = self.weights.mapv(|w| w.sqrt());

        let mut z = self.endog.view().into_faer_col().to_owned();
        let mut zz = self.exog.view().into_faer().to_owned();

        for (i, &sw) in sqrt_weights.iter().enumerate() {
            z[i] *= sw;
        }
        for (i, mut row) in zz.row_iter_mut().enumerate() {
            let sw = sqrt_weights[i];
            row *= sw;
        }

        let (rank, cond_no) = matrix_rank(zz.as_ref().to_owned());
        let n = zz.nrows();
        let df_residual = n - rank;
        let df_model = if self.config.constant { rank - 1 } else { rank };
        let df_total = df_residual + df_model;

        let covariance_type = if self.config.cov_type.is_empty() {
            "nonrobust".to_string()
        } else {
            self.config.cov_type.clone()
        };

        let xtx = zz.as_ref().transpose() * zz.as_ref();
        let xtz = zz.as_ref().transpose() * z.as_ref();
        let xtx_inv = xtx
            .llt(Side::Lower)
            .map_err(|_| "WLS: X'WX matrix is not positive definite".to_string())?
            .solve(Mat::identity(xtx.nrows(), xtx.ncols()));
        let betas = xtx_inv.as_ref() * xtz;
        let z_hat = zz.as_ref() * betas.as_ref();

        let z_mean = z.iter().mean();
        let ss_total = if self.config.constant {
            z.iter().map(|v| (v - z_mean).powi(2)).sum::<f64>()
        } else {
            z.iter().map(|v| v.powi(2)).sum::<f64>()
        };
        let ss_residual = (z.as_ref() - z_hat.as_ref()).iter().map(|v| v.powi(2)).sum::<f64>();
        let ss_model = ss_total - ss_residual;

        let r2 = 1.0 - ss_residual / ss_total;
        let ms_model = ss_model / df_model as f64;
        let ms_residual = ss_residual / df_residual as f64;
        let ms_total = ss_total / df_total as f64;
        let r2_adjusted = 1.0 - ms_residual / ms_total;
        let f = ms_model / ms_residual;

        let dist = FisherSnedecor::new(df_model as f64, df_residual as f64)
            .map_err(|e| format!("WLS: FisherSnedecor: {}", e))?;
        let f_p_value = 1.0 - dist.cdf(f);

        let u = z - z_hat.as_ref();
        let x_nd = zz.as_ref().into_ndarray().to_owned();
        let xtx_inv_nd = xtx_inv.as_ref().into_ndarray().to_owned();
        let u_nd: Array1<f64> = u.as_ref().into_ndarray().to_owned();

        let cov_beta = compute_cov_beta(
            &x_nd,
            &xtx_inv_nd,
            &u_nd,
            df_residual,
            &covariance_type,
            self.config.cov_params.as_ref(),
        )?;

        let std_err: Array1<f64> = cov_beta.diag().mapv(f64::sqrt);
        let betas_nd = betas.as_ref().into_ndarray().to_owned();
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
        let ci_lower = betas_nd.clone() - t_crit * std_err.clone();
        let ci_upper = betas_nd.clone() + t_crit * std_err.clone();

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
            tvalues: Array1::from_vec(t_values),
            pvalues: Array1::from_vec(p_values),
            conf_int_left: ci_lower,
            conf_int_right: ci_upper,
            cov_beta,
            cond_no,
        })
    }
}