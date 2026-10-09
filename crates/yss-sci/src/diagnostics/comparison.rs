//! Likelihood and nested-model diagnostics over retained OLS/WLS or binary fits.
use crate::regression::models::common::{Result, failed, finite, inverse, parameter, validate};
use statrs::distribution::{ChiSquared, Continuous, ContinuousCDF, FisherSnedecor, Normal};
use yss_sci_contract::regression::fit::FittedRegression;
use yss_sci_contract::{
    diagnostics::model::*,
    execution::ScientificExecutionControl as Control,
    regression::fit::{BinaryRegressionLink, RegressionStatistics},
};
use yss_sci_linalg::{Mat, matrix_rank};

struct View<'a> {
    family: &'static str,
    design: &'a [Vec<f64>],
    fitted: &'a [f64],
    residuals: &'a [f64],
    weights: Option<&'a [f64]>,
    log_likelihood: f64,
    parameters: usize,
}
impl View<'_> {
    fn info(&self) -> Result<InformationCriteria> {
        let n = self.fitted.len();
        Ok(InformationCriteria {
            family: self.family.into(),
            observations: n,
            parameters: self.parameters,
            log_likelihood: self.log_likelihood,
            aic: finite(-2.0 * self.log_likelihood + 2.0 * self.parameters as f64)?,
            bic: finite(-2.0 * self.log_likelihood + (n as f64).ln() * self.parameters as f64)?,
        })
    }
}

fn view<'a>(model: FittedRegression<'a>, control: &Control) -> Result<View<'a>> {
    control.check()?;
    match model {
        FittedRegression::Linear(m) => {
            super::influence::validate_linear(m, control)?;
            let n = m.residuals.len();
            let weight_scale = m
                .weights
                .as_ref()
                .map_or(1.0, |w| w.iter().copied().fold(0.0, f64::max));
            let rss = finite(
                m.residuals
                    .iter()
                    .enumerate()
                    .map(|(i, u)| {
                        let z = u * m
                            .weights
                            .as_ref()
                            .map_or(1.0, |w| (w[i] / weight_scale).sqrt());
                        z * z
                    })
                    .sum(),
            )?;
            if rss <= 0.0 {
                return Err(parameter());
            }
            let log_determinant = m.weights.as_ref().map_or(0.0, |w| {
                w.iter().map(|v| (v / weight_scale).ln()).sum::<f64>()
            });
            let ll = finite(
                -0.5 * n as f64 * ((2.0 * std::f64::consts::PI).ln() + 1.0 + (rss / n as f64).ln())
                    + 0.5 * log_determinant,
            )?;
            Ok(View {
                family: if m.weights.is_some() { "wls" } else { "ols" },
                design: &m.design,
                fitted: &m.fitted,
                residuals: &m.residuals,
                weights: m.weights.as_deref(),
                log_likelihood: ll,
                parameters: m.coefficients.len() + 1,
            })
        }
        FittedRegression::Binary(m) => {
            let RegressionStatistics::Binary { link, model, .. } = &m.statistics else {
                return Err(parameter());
            };
            validate(&m.residuals, &m.design, control)?;
            if m.design.is_empty()
                || m.design.len() != m.coefficients.len()
                || m.fitted.len() != m.residuals.len()
                || !model.converged
                || m.fitted
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                || m.fitted.iter().zip(&m.residuals).any(|(f, r)| {
                    let y = f + r;
                    !close(y, 0.0) && !close(y, 1.0)
                })
            {
                return Err(parameter());
            }
            Ok(View {
                family: match link {
                    BinaryRegressionLink::Logit => "logit",
                    BinaryRegressionLink::Probit => "probit",
                },
                design: &m.design,
                fitted: &m.fitted,
                residuals: &m.residuals,
                weights: None,
                log_likelihood: finite(model.log_likelihood)?,
                parameters: m.coefficients.len(),
            })
        }
    }
}

pub fn information_criteria(
    model: FittedRegression<'_>,
    control: &Control,
) -> Result<InformationCriteria> {
    view(model, control)?.info()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}
fn chi_square(statistic: f64, df: usize) -> Result<ComparisonTest> {
    if df == 0 || statistic < 0.0 {
        return Err(parameter());
    }
    let statistic = finite(statistic)?;
    Ok(ComparisonTest {
        statistic,
        degrees_of_freedom: df,
        denominator_df: None,
        p_value: ChiSquared::new(df as f64)
            .map_err(|_| failed())?
            .sf(statistic),
    })
}

pub fn compare_models(
    restricted: FittedRegression<'_>,
    full: FittedRegression<'_>,
    method: ComparisonMethod,
    control: &Control,
) -> Result<ModelComparison> {
    let r = view(restricted, control)?;
    let f = view(full, control)?;
    let n = f.fitted.len();
    let kr = r.design.len();
    let kf = f.design.len();
    if r.family != f.family
        || r.fitted.len() != n
        || kf <= kr
        || n <= kf
        || r.fitted
            .iter()
            .zip(r.residuals)
            .zip(f.fitted.iter().zip(f.residuals))
            .any(|((a, u), (b, v))| !close(a + u, b + v))
    {
        return Err(parameter());
    }
    if let (Some(rw), Some(fw)) = (r.weights, f.weights) {
        let rs = rw.iter().copied().fold(0.0, f64::max);
        let fs = fw.iter().copied().fold(0.0, f64::max);
        if rw.iter().zip(fw).any(|(a, b)| !close(a / rs, b / fs)) {
            return Err(parameter());
        }
    }
    let scales = f
        .design
        .iter()
        .map(|col| col.iter().map(|v| v.abs()).fold(0.0, f64::max))
        .collect::<Vec<_>>();
    if scales.contains(&0.0) {
        return Err(parameter());
    }
    let x = Mat::from_fn(n, kf, |i, j| f.design[j][i] / scales[j]);
    control.check()?;
    if matrix_rank(x.as_ref()).map_err(|_| failed())?.0 != kf {
        return Err(parameter());
    }
    let inv = inverse(&(x.transpose() * &x))?;
    // Verify column-space inclusion rather than comparing predictor names/counts.
    for col in r.design {
        control.check()?;
        let scale = col.iter().map(|v| v.abs()).fold(0.0, f64::max);
        if scale == 0.0 {
            return Err(parameter());
        }
        let rhs = (0..kf)
            .map(|j| (0..n).map(|i| x[(i, j)] * (col[i] / scale)).sum::<f64>())
            .collect::<Vec<_>>();
        let beta = (0..kf)
            .map(|j| (0..kf).map(|k| inv[(j, k)] * rhs[k]).sum::<f64>())
            .collect::<Vec<_>>();
        let residual = (0..n)
            .map(|i| (col[i] / scale - (0..kf).map(|j| x[(i, j)] * beta[j]).sum::<f64>()).powi(2))
            .sum::<f64>();
        let norm = col.iter().map(|v| (v / scale).powi(2)).sum::<f64>();
        if !residual.is_finite() || residual > 1e-14 * norm {
            return Err(parameter());
        }
    }
    let df = kf - kr;
    let delta = finite(f.log_likelihood - r.log_likelihood)?;
    if delta < -1e-8 * (1.0 + r.log_likelihood.abs()) {
        return Err(parameter());
    }
    let linear = matches!(f.family, "ols" | "wls");
    let mut result = ModelComparison {
        restricted: r.info()?,
        full: f.info()?,
        restrictions: df,
        likelihood_ratio: None,
        score: None,
        f_test: None,
    };
    if method != ComparisonMethod::Score {
        result.likelihood_ratio = Some(chi_square(2.0 * delta.max(0.0), df)?);
    }
    if linear {
        // Profile-likelihood difference is invariant to an overall WLS weight scale.
        let log_ratio = 2.0 * delta.max(0.0) / n as f64;
        if method != ComparisonMethod::LikelihoodRatio {
            result.score = Some(chi_square(n as f64 * -(-log_ratio).exp_m1(), df)?);
        }
        if method == ComparisonMethod::All {
            let statistic = finite(log_ratio.exp_m1() * (n - kf) as f64 / df as f64)?;
            result.f_test = Some(ComparisonTest {
                statistic,
                degrees_of_freedom: df,
                denominator_df: Some(n - kf),
                p_value: crate::distribution::fisher_snedecor_sf(
                    &FisherSnedecor::new(df as f64, (n - kf) as f64).map_err(|_| failed())?,
                    statistic,
                ),
            });
        }
    } else if method != ComparisonMethod::LikelihoodRatio {
        let normal = Normal::new(0.0, 1.0).expect("normal");
        let mut score = vec![0.0; kf];
        let mut information = Mat::zeros(kf, kf);
        for i in 0..n {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            let p = r.fitted[i];
            if p <= 0.0 || p >= 1.0 {
                return Err(parameter());
            }
            let variance = p * (1.0 - p);
            let derivative = if r.family == "logit" {
                variance
            } else {
                normal.pdf(normal.inverse_cdf(p))
            };
            let s = r.residuals[i] * derivative / variance;
            let w = derivative * derivative / variance;
            for j in 0..kf {
                score[j] += x[(i, j)] * s;
                for k in 0..kf {
                    information[(j, k)] += w * x[(i, j)] * x[(i, k)];
                }
            }
        }
        let inverse = inverse(&information)?;
        let statistic = (0..kf)
            .map(|j| score[j] * (0..kf).map(|k| inverse[(j, k)] * score[k]).sum::<f64>())
            .sum::<f64>();
        result.score = Some(chi_square(statistic, df)?);
    }
    control.check()?;
    Ok(result)
}
