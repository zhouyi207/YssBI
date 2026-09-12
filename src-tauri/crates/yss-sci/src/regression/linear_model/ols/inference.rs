use super::fit::OlsSolution;
use super::{OLS, OlsFit, OlsFitError};
use crate::regression::covariance::compute_cov_beta;
use faer::Col;
use num_traits::{One, Pow, Zero};
use statrs::{
    distribution::{ContinuousCDF, FisherSnedecor, StudentsT},
    statistics::Statistics,
};
use yss_linalg::{MatrixExt, Solve};

fn is_robust_cov_type(cov_type: &str) -> bool {
    matches!(
        cov_type,
        "HC0" | "HC1" | "HC2" | "HC3" | "cluster" | "HAC" | "newey" | "fixed scale"
    )
}

pub(super) fn infer(model: &OLS, solution: OlsSolution) -> Result<OlsFit, OlsFitError> {
    let OlsSolution {
        x,
        y,
        rank,
        cond_no,
        xtx_inv,
        betas,
        y_hat,
    } = solution;
    let num_observations = x.nrows();
    let df_residual = num_observations - rank;
    let df_model = if model.config.constant {
        rank - 1
    } else {
        rank
    };
    let df_total = df_residual + df_model;

    let covariance_type = model.config.covariance.name().to_string();
    let covariance_parameters = model.config.covariance.parameters();

    let y_mean = y.iter().mean();

    let ss_total = if model.config.constant {
        y.iter().map(|v| (v - y_mean).pow(2)).sum::<f64>()
    } else {
        y.iter().map(|v| v.pow(2)).sum::<f64>()
    };
    let ss_residual = (y.as_ref() - y_hat.as_ref())
        .iter()
        .map(|v| v.pow(2))
        .sum::<f64>();
    let ss_model = ss_total - ss_residual;

    let r2 = 1.0 - ss_residual / ss_total;

    let ms_model = ss_model / df_model as f64;
    let ms_residual = ss_residual / df_residual as f64;
    let ms_total = ss_total / df_total as f64;
    let r2_adjusted = 1.0 - ms_residual / ms_total;

    // 残差（需在 cov_beta 之前计算）
    let u = &y - y_hat.as_ref();

    let x_nd = x.as_ref().to_owned();
    let xtx_inv_nd = xtx_inv.as_ref().to_owned();
    let u_nd: Col<f64> = u.as_ref().to_owned();

    // 参数协方差矩阵（根据 cov_type）
    let cov_beta = compute_cov_beta(
        &x_nd,
        &xtx_inv_nd,
        &u_nd,
        df_residual,
        &covariance_type,
        covariance_parameters.as_ref(),
    )
    .map_err(OlsFitError::Covariance)?;

    let cov_beta_nonrobust = faer::Scale(ms_residual) * &xtx_inv_nd;

    // F 统计量：robust VCE 时用 Wald，否则用经典 F
    let (f, f_p_value) = if df_model > 0 {
        if is_robust_cov_type(&covariance_type) {
            let betas_nd = betas.as_ref().to_owned();
            // Wald = β_s' V_s^{-1} β_s，F = Wald / df_model
            let (beta_s, v_s) = if model.config.constant && rank > 1 {
                (
                    betas_nd.subrows(1, betas_nd.nrows() - 1).to_owned(),
                    cov_beta
                        .submatrix(1, 1, cov_beta.nrows() - 1, cov_beta.ncols() - 1)
                        .to_owned(),
                )
            } else {
                (betas_nd.clone(), cov_beta.clone())
            };
            let wald = if beta_s.nrows() > 0 {
                let v_matrix = v_s.as_ref().to_owned();
                let beta_vector = beta_s.as_ref().to_owned();
                match v_matrix.as_ref().checked_cholesky() {
                    Ok(llt) => {
                        let x_sol = llt.solve(&beta_vector.as_ref());
                        beta_s.transpose() * x_sol.as_ref()
                    }
                    Err(_) => 0.0, // cov 非正定（如 cluster 聚类少）时回退
                }
            } else {
                0.0
            };
            let f_val = (wald / df_model as f64).max(0.0);
            let df1 = (df_model as f64).max(1.0);
            let df2 = (df_residual as f64).max(1.0);
            let dist = FisherSnedecor::new(df1, df2)
                .map_err(|e| {
                    format!(
                        "OLS F-distribution: df_model={} df_residual={} {}",
                        df_model, df_residual, e
                    )
                })
                .map_err(OlsFitError::Inference)?;
            (f_val, 1.0 - dist.cdf(f_val))
        } else {
            let f_val = (ms_model / ms_residual).max(0.0);
            let df1 = (df_model as f64).max(1.0);
            let df2 = (df_residual as f64).max(1.0);
            let dist = FisherSnedecor::new(df1, df2)
                .map_err(|e| {
                    format!(
                        "OLS F-distribution: df_model={} df_residual={} {}",
                        df_model, df_residual, e
                    )
                })
                .map_err(OlsFitError::Inference)?;
            (f_val, 1.0 - dist.cdf(f_val))
        }
    } else {
        (0.0, 1.0)
    };

    // std err
    let std_err: Col<f64> = cov_beta.diagonal().column_vector().map(|v| v.sqrt());

    // t value
    let betas_nd = betas.as_ref().to_owned();
    let t_values: Vec<f64> = betas_nd
        .iter()
        .zip(std_err.iter())
        .map(|(b, se)| b / se)
        .collect();

    let t_df = (df_residual as f64).max(1.0);
    let t_dist = StudentsT::new(f64::zero(), f64::one(), t_df).map_err(|e| {
        OlsFitError::Inference(format!(
            "OLS t-distribution: df_residual={} {}",
            df_residual, e
        ))
    })?;
    let p_values: Vec<f64> = t_values
        .iter()
        .map(|&t| 2.0 * (1.0 - t_dist.cdf(t.abs())))
        .collect();

    let t_critical = t_dist.inverse_cdf(0.975);
    let ci_lower = betas_nd.clone() - faer::Scale(t_critical) * std_err.clone();
    let ci_upper = betas_nd.clone() + faer::Scale(t_critical) * std_err.clone();

    Ok(OlsFit {
        num_observation: num_observations,
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
        betas: betas_nd,
        fitted: y_hat,
        residuals: u,
        rank,
        stds: std_err,
        tvalues: (t_values).into_iter().collect::<Col<f64>>(),
        pvalues: (p_values).into_iter().collect::<Col<f64>>(),
        conf_int_left: ci_lower,
        conf_int_right: ci_upper,
        cov_beta,
        cov_beta_nonrobust,
        cond_no,
    })
}
