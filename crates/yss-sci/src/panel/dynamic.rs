//! One-step Arellano–Bond difference GMM with collapsed lag-level instruments.
use super::data::{Result, failed, groups, parameter};
use statrs::distribution::{ContinuousCDF, Normal};
use yss_sci_contract::{
    execution::ScientificExecutionControl, panel::*,
    regression::fit::RegressionCoefficientStatistics,
};
use yss_sci_linalg::{Col, Mat, MatrixExt, Scale, Solve, matrix_rank};

fn inverse(a: &Mat<f64>) -> Result<Mat<f64>> {
    if matrix_rank(a.as_ref()).map_err(|_| failed())?.0 != a.nrows() {
        return Err(parameter());
    }
    let inverse = a
        .checked_cholesky()
        .map_err(|_| failed())?
        .solve(&Mat::<f64>::identity(a.nrows(), a.nrows()));
    if (0..inverse.ncols()).any(|j| (0..inverse.nrows()).any(|i| !inverse[(i, j)].is_finite())) {
        return Err(failed());
    }
    Ok(inverse)
}

fn equations(data: &PanelData<'_>, rows: &[usize], lag: usize) -> (Col<f64>, Mat<f64>, Mat<f64>) {
    let n = rows.len() - 2;
    let k = data.predictors.len();
    let y = Col::from_iter(
        (2..rows.len()).map(|t| data.response[rows[t]] - data.response[rows[t - 1]]),
    );
    let x = Mat::from_fn(n, k + 1, |i, j| {
        let t = i + 2;
        if j == 0 {
            data.response[rows[t - 1]] - data.response[rows[t - 2]]
        } else {
            data.predictors[j - 1][rows[t]] - data.predictors[j - 1][rows[t - 1]]
        }
    });
    let z = Mat::from_fn(n, lag - 1 + k, |i, j| {
        let t = i + 2;
        if j < lag - 1 {
            if t >= j + 2 {
                data.response[rows[t - j - 2]]
            } else {
                0.0
            }
        } else {
            x[(i, j - (lag - 1) + 1)]
        }
    });
    (y, x, z)
}

pub fn difference_gmm(
    data: PanelData<'_>,
    options: DynamicPanelOptions,
    control: &ScientificExecutionControl,
) -> Result<DynamicPanelFit> {
    let groups = groups(&data, control)?;
    let periods = groups[0].len();
    let lag = options.max_instrument_lag;
    if groups.len() < 3
        || periods < 4
        || lag < 2
        || lag >= periods
        || groups
            .iter()
            .any(|g| g.len() != periods || data.time[g[0]] != data.time[groups[0][0]])
    {
        return Err(parameter());
    }
    let p = data.predictors.len() + 1;
    let q = lag - 1 + data.predictors.len();
    let n = groups.len() * (periods - 2);
    if n <= p || q >= groups.len() {
        return Err(parameter());
    }
    let mut moment_weight = Mat::<f64>::zeros(q, q);
    let mut zx = Mat::<f64>::zeros(q, p);
    let mut zy = Col::<f64>::zeros(q);
    for rows in &groups {
        control.check()?;
        let (y, x, z) = equations(&data, rows, lag);
        // H = D D': differencing iid level errors gives 2 on the diagonal and -1 adjacent.
        let hz = Mat::from_fn(z.nrows(), q, |i, j| {
            2.0 * z[(i, j)]
                - if i > 0 { z[(i - 1, j)] } else { 0.0 }
                - if i + 1 < z.nrows() {
                    z[(i + 1, j)]
                } else {
                    0.0
                }
        });
        let weight = z.transpose() * hz.as_ref();
        let cross = z.transpose() * x.as_ref();
        let outcome = z.transpose() * y.as_ref();
        for i in 0..q {
            zy[i] += outcome[i];
            for j in 0..q {
                moment_weight[(i, j)] += weight[(i, j)];
            }
            for j in 0..p {
                zx[(i, j)] += cross[(i, j)];
            }
        }
    }
    control.check()?;
    let weight = inverse(&moment_weight)?;
    let left = zx.transpose() * weight.as_ref();
    let bread = inverse(&(left.as_ref() * zx.as_ref()))?;
    let projection = bread.as_ref() * left.as_ref();
    let beta = projection.as_ref() * zy.as_ref();
    let mut meat = Mat::<f64>::zeros(q, q);
    let mut source_rows = Vec::with_capacity(n);
    let mut fitted = Vec::with_capacity(n);
    let mut residuals = Vec::with_capacity(n);
    let mut rss = 0.0;
    for rows in &groups {
        control.check()?;
        let (y, x, z) = equations(&data, rows, lag);
        let predictions = x.as_ref() * beta.as_ref();
        let errors = y.as_ref() - predictions.as_ref();
        let score = z.transpose() * errors.as_ref();
        for i in 0..q {
            for j in 0..q {
                meat[(i, j)] += score[i] * score[j];
            }
        }
        for i in 0..errors.nrows() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            rss += errors[i] * errors[i];
            fitted.push(predictions[i]);
            residuals.push(errors[i]);
            source_rows.push(rows[i + 2]);
        }
    }
    // Homoskedastic level-error variance is half the mean squared differenced residual.
    // Robust inference uses entity score clusters, without a finite-sample correction.
    let covariance = if options.robust {
        projection.as_ref() * meat.as_ref() * projection.transpose()
    } else {
        Scale(rss / (2.0 * n as f64)) * &bread
    };
    let normal = Normal::new(0.0, 1.0).map_err(|_| failed())?;
    let critical = normal.inverse_cdf(0.975);
    let mut se = Vec::with_capacity(p);
    let mut z_values = Vec::with_capacity(p);
    let mut p_values = Vec::with_capacity(p);
    let mut lower = Vec::with_capacity(p);
    let mut upper = Vec::with_capacity(p);
    for j in 0..p {
        if !beta[j].is_finite() || !covariance[(j, j)].is_finite() || covariance[(j, j)] <= 0.0 {
            return Err(failed());
        }
        let std = covariance[(j, j)].sqrt();
        let statistic = beta[j] / std;
        se.push(std);
        z_values.push(statistic);
        p_values.push(crate::distribution::normal_two_sided_p(statistic));
        lower.push(beta[j] - critical * std);
        upper.push(beta[j] + critical * std);
    }
    let covariance = (0..p)
        .map(|i| (0..p).map(|j| covariance[(i, j)]).collect())
        .collect();
    let result = DynamicPanelFit {
        method: "arellano_bond_one_step_collapsed".into(),
        covariance: if options.robust {
            "entity_robust"
        } else {
            "nonrobust"
        }
        .into(),
        observations: n,
        entities: groups.len(),
        time_periods: periods,
        instruments: q,
        max_instrument_lag: lag,
        parameter_names: std::iter::once("lag(response,1)".into())
            .chain((1..p).map(|j| format!("x{j}")))
            .collect(),
        coefficients: beta.iter().copied().collect(),
        inference: RegressionCoefficientStatistics {
            covariance,
            standard_errors: se,
            statistic_values: z_values,
            p_values,
            confidence_interval_lower: lower,
            confidence_interval_upper: upper,
        },
        source_rows,
        fitted,
        residuals,
    };
    if result
        .coefficients
        .iter()
        .chain(&result.fitted)
        .chain(&result.residuals)
        .chain(&result.inference.statistic_values)
        .chain(&result.inference.confidence_interval_lower)
        .chain(&result.inference.confidence_interval_upper)
        .chain(result.inference.covariance.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(failed());
    }
    control.check()?;
    Ok(result)
}
