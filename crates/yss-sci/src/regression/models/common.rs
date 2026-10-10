use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use yss_sci_contract::execution::{
    ScientificComputationError as Error, ScientificExecutionControl as Control,
    ScientificInputViolation as Violation,
};
use yss_sci_contract::regression::models::*;
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

pub(crate) type Result<T> = std::result::Result<T, Error>;
pub(crate) fn invalid(violation: Violation) -> Error {
    Error::InvalidInput { violation }
}
pub(crate) fn parameter() -> Error {
    invalid(Violation::ParameterOutOfRange)
}
pub(crate) fn failed() -> Error {
    Error::ComputationFailed
}
pub(crate) fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(failed())
    }
}
pub(crate) fn normal_log_cdf(z: f64) -> f64 {
    if z < -10.0 {
        -0.5 * z * z - (-z).ln() - 0.5 * (2.0 * std::f64::consts::PI).ln()
            + normal_log_tail_correction(z)
    } else {
        Normal::new(0.0, 1.0).expect("normal").cdf(z).ln()
    }
}
fn normal_log_tail_correction(z: f64) -> f64 {
    let t = 1.0 / (z * z);
    (1.0 - t + 3.0 * t * t - 15.0 * t.powi(3) + 105.0 * t.powi(4)).ln()
}
pub(crate) fn normal_log_cdf_density_ratio(z: f64) -> f64 {
    if z < -10.0 {
        -(-z).ln() + normal_log_tail_correction(z)
    } else {
        normal_log_cdf(z) + 0.5 * z * z + 0.5 * (2.0 * std::f64::consts::PI).ln()
    }
}
pub(crate) fn check_iteration(options: IterationOptions) -> Result<()> {
    if options.max_iterations == 0
        || !options.tolerance.is_finite()
        || !(1e-12..=0.01).contains(&options.tolerance)
    {
        return Err(parameter());
    }
    Ok(())
}
pub(crate) fn validate(y: &[f64], predictors: &[Vec<f64>], control: &Control) -> Result<()> {
    control.check()?;
    if y.is_empty() {
        return Err(invalid(Violation::EmptyInput));
    }
    if predictors.iter().any(|x| x.len() != y.len()) {
        return Err(invalid(Violation::ShapeMismatch));
    }
    for x in std::iter::once(y).chain(predictors.iter().map(Vec::as_slice)) {
        for (i, v) in x.iter().enumerate() {
            if i % 1024 == 0 {
                control.check()?;
            }
            if !v.is_finite() {
                return Err(invalid(Violation::NonFiniteInput));
            }
        }
    }
    Ok(())
}
pub(super) fn mean(x: &[f64]) -> f64 {
    x.iter().map(|v| v / x.len() as f64).sum()
}
pub(super) fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| a * b).sum()
}
pub(super) fn median(x: &[f64]) -> f64 {
    let mut x = x.to_vec();
    x.sort_by(f64::total_cmp);
    let n = x.len();
    if n.is_multiple_of(2) {
        x[n / 2 - 1] / 2.0 + x[n / 2] / 2.0
    } else {
        x[n / 2]
    }
}
pub(crate) fn names(p: usize, constant: bool) -> Vec<String> {
    (0..p + usize::from(constant))
        .map(|i| {
            if constant && i == 0 {
                "intercept".into()
            } else {
                format!("x{}", i + usize::from(!constant))
            }
        })
        .collect()
}
pub(crate) struct Design {
    pub x: Mat<f64>,
    pub means: Vec<f64>,
    pub scales: Vec<f64>,
    pub constant: bool,
}
impl Design {
    pub fn new(
        predictors: &[impl AsRef<[f64]>],
        n: usize,
        constant: bool,
        standardize: bool,
        rank: bool,
        control: &Control,
    ) -> Result<Self> {
        control.check()?;
        let p = predictors.len() + usize::from(constant);
        if p == 0 {
            return Err(invalid(Violation::ShapeMismatch));
        }
        if n < 2 || (rank && n <= p) {
            return Err(invalid(Violation::EmptyInput));
        }
        let mut means = Vec::with_capacity(predictors.len());
        let mut scales = Vec::with_capacity(predictors.len());
        for column in predictors {
            let x = column.as_ref();
            let m = if constant { mean(x) } else { 0.0 };
            let max = x.iter().map(|v| (v - m).abs()).fold(0.0, f64::max);
            let s = if standardize && max > 0.0 {
                max * (x.iter().map(|v| ((v - m) / max).powi(2)).sum::<f64>() / n as f64).sqrt()
            } else {
                1.0
            };
            if !m.is_finite() || !s.is_finite() || s <= 0.0 {
                return Err(failed());
            }
            means.push(m);
            scales.push(s);
        }
        let x = Mat::from_fn(n, p, |i, j| {
            if constant && j == 0 {
                1.0
            } else {
                let k = j - usize::from(constant);
                (predictors[k].as_ref()[i] - means[k]) / scales[k]
            }
        });
        if rank {
            let (r, _) = matrix_rank(x.as_ref()).map_err(|_| failed())?;
            if r != p {
                return Err(invalid(Violation::DataOutOfRange));
            }
        }
        control.check()?;
        Ok(Self {
            x,
            means,
            scales,
            constant,
        })
    }
    pub fn raw_jacobian(&self, response_scale: f64) -> Mat<f64> {
        let p = self.x.ncols();
        Mat::from_fn(p, p, |i, j| {
            if self.constant && i == 0 {
                if j == 0 {
                    response_scale
                } else {
                    -self.means[j - 1] / self.scales[j - 1] * response_scale
                }
            } else if i == j {
                // The joint unit ratio can be finite even when the predictor
                // reciprocal overflows before response units are applied.
                response_scale / self.scales[j - usize::from(self.constant)]
            } else {
                0.0
            }
        })
    }
    pub fn raw(&self, beta: &[f64], covariance: Option<Mat<f64>>) -> (Vec<f64>, Option<Mat<f64>>) {
        let offset = usize::from(self.constant);
        let mut raw = beta.to_vec();
        for (j, scale) in self.scales.iter().enumerate() {
            raw[offset + j] /= scale;
            if self.constant {
                raw[0] -= self.means[j] * raw[offset + j];
            }
        }
        let covariance = covariance.map(|mut covariance| {
            let p = self.x.ncols();
            let scale = |i| {
                if self.constant && i == 0 {
                    1.0
                } else {
                    self.scales[i - offset]
                }
            };
            for i in 0..p {
                for j in 0..p {
                    let (a, b) = (scale(i), scale(j));
                    let product = a * b;
                    let value = covariance[(i, j)];
                    covariance[(i, j)] = if product.is_normal() {
                        value / product
                    } else {
                        // Preserve small cross-covariances without forming an
                        // underflowing scale product or overflowing reciprocal.
                        let first = value / a.min(b);
                        if first.is_finite() {
                            first / a.max(b)
                        } else {
                            (value / a.max(b)) / a.min(b)
                        }
                    };
                }
            }
            if self.constant {
                // Center the intercept row before the column so both passes
                // consume the correct, already-scaled slope entries in place.
                for j in 0..p {
                    let shift = (1..p)
                        .map(|i| self.means[i - 1] * covariance[(i, j)])
                        .sum::<f64>();
                    covariance[(0, j)] -= shift;
                }
                for i in 0..p {
                    let shift = (1..p)
                        .map(|j| covariance[(i, j)] * self.means[j - 1])
                        .sum::<f64>();
                    covariance[(i, 0)] -= shift;
                }
            }
            covariance
        });
        (raw, covariance)
    }
}
pub(crate) fn transform(
    beta: &[f64],
    covariance: Option<Mat<f64>>,
    j: &Mat<f64>,
) -> (Vec<f64>, Option<Mat<f64>>) {
    let b = j.as_ref() * Col::from_iter(beta.iter().copied()).as_ref();
    let covariance = covariance.map(|c| j.as_ref() * c.as_ref() * j.transpose());
    (b.iter().copied().collect(), covariance)
}
pub(crate) fn fitted(x: &Mat<f64>, beta: &[f64]) -> Vec<f64> {
    (0..x.nrows())
        .map(|i| (0..x.ncols()).map(|j| x[(i, j)] * beta[j]).sum())
        .collect()
}
pub(crate) fn gram(x: &Mat<f64>, weights: Option<&[f64]>, control: &Control) -> Result<Mat<f64>> {
    let p = x.ncols();
    let mut a = Mat::zeros(p, p);
    for i in 0..x.nrows() {
        if i % 1024 == 0 {
            control.check()?;
        }
        let w = weights.map_or(1.0, |v| v[i]);
        if !w.is_finite() || w < 0.0 {
            return Err(failed());
        }
        for j in 0..p {
            for k in 0..=j {
                a[(j, k)] += w * x[(i, j)] * x[(i, k)];
            }
        }
    }
    for j in 0..p {
        for k in 0..j {
            a[(k, j)] = a[(j, k)];
        }
    }
    Ok(a)
}
pub(crate) fn inverse(a: &Mat<f64>) -> Result<Mat<f64>> {
    a.checked_cholesky()
        .map_err(|_| failed())
        .map(|f| f.solve(&Mat::identity(a.ncols(), a.ncols())))
}
pub(crate) fn least_squares(
    x: &Mat<f64>,
    y: &[f64],
    weights: Option<&[f64]>,
    control: &Control,
) -> Result<(Vec<f64>, Mat<f64>)> {
    let inv = inverse(&gram(x, weights, control)?)?;
    let rhs = Col::from_fn(x.ncols(), |j| {
        (0..y.len())
            .map(|i| x[(i, j)] * y[i] * weights.map_or(1.0, |w| w[i]))
            .sum()
    });
    let beta = inv.as_ref() * rhs.as_ref();
    control.check()?;
    if beta.iter().any(|v| !v.is_finite()) {
        return Err(failed());
    }
    Ok((beta.iter().copied().collect(), inv))
}
pub(crate) fn coefficient_table(
    beta: &[f64],
    names: Vec<String>,
    covariance: Option<&Mat<f64>>,
    df: Option<usize>,
    confidence_level: f64,
) -> Result<Vec<RegressionCoefficient>> {
    let inference = covariance
        .map(|_| -> Result<_> {
            let q =
                crate::inference::intervals::critical(confidence_level, df.map(|df| df as f64))?;
            let t = df
                .map(|df| StudentsT::new(0.0, 1.0, df as f64).map_err(|_| failed()))
                .transpose()?;
            Ok((t, q))
        })
        .transpose()?;
    beta.iter()
        .enumerate()
        .map(|(i, &estimate)| {
            let se = covariance
                .map(|c| {
                    let variance = finite(c[(i, i)])?;
                    if variance < 0.0 {
                        return Err(failed());
                    }
                    finite(variance.sqrt())
                })
                .transpose()?;
            let positive_parameter = matches!(
                names[i].as_str(),
                "alpha" | "sigma" | "precision" | "amplitude"
            );
            let statistic = if positive_parameter {
                None
            } else {
                se.filter(|&v| v > 0.0)
                    .map(|v| finite(estimate / v))
                    .transpose()?
            };
            let p_value = statistic.map(|v| {
                inference
                    .as_ref()
                    .and_then(|(t, _)| t.as_ref())
                    .map_or_else(
                        || crate::distribution::normal_two_sided_p(v),
                        |d| {
                            crate::distribution::student_t_probability(
                                d,
                                v,
                                yss_sci_contract::hypothesis::Alternative::TwoSided,
                            )
                        },
                    )
                    .clamp(0.0, 1.0)
            });
            let confidence_interval = se
                .zip(inference.as_ref())
                .map(|(s, (_, q))| -> Result<[f64; 2]> {
                    Ok([finite(estimate - q * s)?, finite(estimate + q * s)?])
                })
                .transpose()?;
            Ok(RegressionCoefficient {
                term: names[i].clone(),
                estimate,
                standard_error: se,
                statistic,
                p_value,
                confidence_interval,
            })
        })
        .collect()
}
// Keep coefficient-axis facts together and explicit at each estimator's projection boundary.
#[allow(clippy::too_many_arguments)]
pub(super) fn result(
    method: &str,
    y: &[f64],
    predicted: Vec<f64>,
    beta: Vec<f64>,
    terms: Vec<String>,
    covariance: Option<Mat<f64>>,
    constant: bool,
    df: Option<usize>,
    details: RegressionDetails,
) -> Result<RegressionModelResult> {
    let residuals = y
        .iter()
        .zip(&predicted)
        .map(|(y, p)| y - p)
        .collect::<Vec<_>>();
    let rss = finite(dot(&residuals, &residuals))?;
    if predicted.iter().chain(beta.iter()).any(|v| !v.is_finite()) {
        return Err(failed());
    }
    if covariance
        .as_ref()
        .is_some_and(|c| c.row_iter().any(|r| r.iter().any(|v| !v.is_finite())))
    {
        return Err(failed());
    }
    let center = if constant { mean(y) } else { 0.0 };
    let scale = y.iter().map(|v| (v - center).abs()).fold(0.0, f64::max);
    let r_squared = if scale > 0.0 {
        finite(scale)?;
        let tss = y
            .iter()
            .map(|v| ((v - center) / scale).powi(2))
            .sum::<f64>();
        let residual_ss = residuals.iter().map(|v| (v / scale).powi(2)).sum::<f64>();
        Some(finite(1.0 - residual_ss / tss)?)
    } else {
        None
    };
    let adjusted_r_squared = df.and_then(|df| {
        r_squared.map(|r| 1.0 - (1.0 - r) * (y.len() - usize::from(constant)) as f64 / df as f64)
    });
    let coefficients = coefficient_table(&beta, terms, covariance.as_ref(), df, 0.95)?;
    let covariance = covariance.map(|c| super::super::design::covariance_rows(&c));
    Ok(RegressionModelResult {
        method: method.into(),
        observations: y.len(),
        constant,
        coefficients,
        covariance,
        fitted: predicted,
        residuals,
        categories: vec![],
        fitted_categories: vec![],
        probabilities: vec![],
        statistics: ModelStatistics {
            rss: Some(rss),
            rmse: Some((rss / y.len() as f64).sqrt()),
            r_squared,
            adjusted_r_squared,
            df_residual: df,
            log_likelihood: None,
            aic: None,
            bic: None,
        },
        iterations: 1,
        converged: true,
        details,
    })
}
pub(crate) fn ols(
    y: &[f64],
    predictors: &[Vec<f64>],
    constant: bool,
    control: &Control,
) -> Result<RegressionModelResult> {
    validate(y, predictors, control)?;
    // Existing OLS owns coefficient inference; an intercept-only baseline has no F test.
    if !predictors.is_empty() {
        let fit = super::super::linear::fit::fit_linear_regression(
            y.to_vec(),
            predictors,
            yss_sci_contract::regression::OlsOptions {
                constant,
                ..Default::default()
            },
            yss_sci_contract::regression::linear::LinearRegressionMethod::Ols,
            yss_sci_contract::StatisticalObservationMetadata {
                original_observation_count: y.len(),
                used_observation_count: y.len(),
                dropped_null_count: 0,
                dropped_nan_count: 0,
                missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
            },
        )
        .map_err(|_| failed())?;
        let stats = fit.statistics.coefficient_statistics();
        let cov = Mat::from_fn(fit.coefficients.len(), fit.coefficients.len(), |i, j| {
            stats.covariance[i][j]
        });
        let mut r = result(
            "ols",
            y,
            fit.fitted,
            fit.coefficients,
            names(predictors.len(), constant),
            Some(cov),
            constant,
            Some(y.len() - predictors.len() - usize::from(constant)),
            RegressionDetails::Linear,
        )?;
        gaussian_likelihood(&mut r)?;
        control.check()?;
        return Ok(r);
    }
    let design = Design::new(predictors, y.len(), constant, true, true, control)?;
    let (beta, inv) = least_squares(&design.x, y, None, control)?;
    let pred = fitted(&design.x, &beta);
    let rss = y
        .iter()
        .zip(&pred)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>();
    let df = y.len() - beta.len();
    let cov = Mat::from_fn(inv.ncols(), inv.ncols(), |i, j| {
        inv[(i, j)] * rss / df as f64
    });
    let mut r = result(
        "ols",
        y,
        pred,
        beta,
        names(predictors.len(), constant),
        Some(cov),
        constant,
        Some(df),
        RegressionDetails::Linear,
    )?;
    gaussian_likelihood(&mut r)?;
    Ok(r)
}
pub(super) fn gaussian_likelihood(r: &mut RegressionModelResult) -> Result<()> {
    if let Some(rss) = r.statistics.rss.filter(|&rss| rss > 0.0) {
        let n = r.observations as f64;
        let ll = finite(-0.5 * n * ((2.0 * std::f64::consts::PI).ln() + 1.0 + (rss / n).ln()))?;
        likelihood_statistics(r, ll, r.coefficients.len() + 1);
    }
    Ok(())
}
pub(super) fn likelihood_statistics(r: &mut RegressionModelResult, ll: f64, parameters: usize) {
    r.statistics.log_likelihood = Some(ll);
    r.statistics.aic = Some(-2.0 * ll + 2.0 * parameters as f64);
    r.statistics.bic = Some(-2.0 * ll + (r.observations as f64).ln() * parameters as f64);
}
pub(crate) struct Minimum {
    pub beta: Vec<f64>,
    pub value: f64,
    pub iterations: usize,
}
pub(super) fn gradient(
    f: &impl Fn(&[f64]) -> Result<f64>,
    beta: &[f64],
    control: &Control,
) -> Result<Vec<f64>> {
    let mut x = beta.to_vec();
    let mut g = vec![0.0; beta.len()];
    for i in 0..beta.len() {
        control.check()?;
        let h = 1e-5 * (1.0 + beta[i].abs());
        x[i] = beta[i] + h;
        let a = f(&x)?;
        x[i] = beta[i] - h;
        let b = f(&x)?;
        x[i] = beta[i];
        g[i] = finite((a - b) / (2.0 * h))?;
    }
    Ok(g)
}
pub(crate) fn minimize(
    f: &impl Fn(&[f64]) -> Result<f64>,
    initial: Vec<f64>,
    options: IterationOptions,
    control: &Control,
) -> Result<Minimum> {
    check_iteration(options)?;
    control.check()?;
    let p = initial.len();
    if p == 0 {
        return Err(parameter());
    }
    let mut beta = initial;
    let mut value = finite(f(&beta)?)?;
    let mut g = gradient(f, &beta, control)?;
    let mut inv = Mat::identity(p, p);
    for iter in 0..options.max_iterations {
        control.check()?;
        if g.iter().fold(0.0_f64, |m, v| m.max(v.abs())) <= options.tolerance {
            return Ok(Minimum {
                beta,
                value,
                iterations: iter,
            });
        }
        let mut direction = (0..p)
            .map(|i| -(0..p).map(|j| inv[(i, j)] * g[j]).sum::<f64>())
            .collect::<Vec<_>>();
        let mut slope = dot(&direction, &g);
        if !slope.is_finite() || slope >= 0.0 {
            inv = Mat::identity(p, p);
            direction = g.iter().map(|v| -v).collect();
            slope = -dot(&g, &g);
        }
        let mut alpha = 1.0;
        let mut next = None;
        for _ in 0..50 {
            control.check()?;
            let trial = beta
                .iter()
                .zip(&direction)
                .map(|(b, d)| b + alpha * d)
                .collect::<Vec<_>>();
            match f(&trial) {
                Ok(v) if v.is_finite() && v <= value + 1e-4 * alpha * slope => {
                    next = Some((trial, v));
                    break;
                }
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(Error::DeadlineExceeded) => return Err(Error::DeadlineExceeded),
                _ => {
                    alpha *= 0.5;
                }
            }
        }
        let Some((b, v)) = next else {
            return Err(failed());
        };
        let ng = gradient(f, &b, control)?;
        let s = b.iter().zip(&beta).map(|(a, b)| a - b).collect::<Vec<_>>();
        let z = ng.iter().zip(&g).map(|(a, b)| a - b).collect::<Vec<_>>();
        let sz = dot(&s, &z);
        if sz > 1e-12 * dot(&s, &s).sqrt() * dot(&z, &z).sqrt() {
            let iz = (0..p)
                .map(|i| (0..p).map(|j| inv[(i, j)] * z[j]).sum::<f64>())
                .collect::<Vec<_>>();
            let ziz = dot(&z, &iz);
            for i in 0..p {
                for j in 0..p {
                    inv[(i, j)] +=
                        (1.0 + ziz / sz) * s[i] * s[j] / sz - (s[i] * iz[j] + iz[i] * s[j]) / sz;
                }
            }
        } else {
            inv = Mat::identity(p, p);
        }
        beta = b;
        value = v;
        g = ng;
    }
    Err(failed())
}
pub(crate) fn hessian(
    f: &impl Fn(&[f64]) -> Result<f64>,
    beta: &[f64],
    control: &Control,
) -> Result<Mat<f64>> {
    let p = beta.len();
    let mut h = Mat::zeros(p, p);
    let mut x = beta.to_vec();
    let base = f(beta)?;
    for i in 0..p {
        control.check()?;
        let hi = 1e-4 * (1.0 + beta[i].abs());
        x[i] = beta[i] + hi;
        let a = f(&x)?;
        x[i] = beta[i] - hi;
        let b = f(&x)?;
        x[i] = beta[i];
        h[(i, i)] = (a - 2.0 * base + b) / (hi * hi);
        for j in 0..i {
            let hj = 1e-4 * (1.0 + beta[j].abs());
            let mut v = 0.0;
            for (si, sj) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                x[i] = beta[i] + si * hi;
                x[j] = beta[j] + sj * hj;
                v += si * sj * f(&x)?;
            }
            x[i] = beta[i];
            x[j] = beta[j];
            h[(i, j)] = v / (4.0 * hi * hj);
            h[(j, i)] = h[(i, j)];
        }
    }
    control.check()?;
    Ok(h)
}
