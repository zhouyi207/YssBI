//! Publication-asymmetry tests, p-value combination and model sensitivity.
use super::{data::*, model};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};
use yss_sci_contract::association::{CorrelationResult, RankCorrelationOptions, RankInference};

pub fn egger(
    y: &[f64],
    variances: &[f64],
    confidence: f64,
    control: &Control,
) -> Result<MetaSummary> {
    study_data(y, variances, control)?;
    let precision = variances.iter().map(|v| 1. / v.sqrt()).collect::<Vec<_>>();
    let standardized = y
        .iter()
        .zip(&precision)
        .map(|(y, p)| y * p)
        .collect::<Vec<_>>();
    let mut fit = model::summary(
        &standardized,
        &vec![1.; y.len()],
        &[precision],
        MetaOptions {
            estimator: MetaEstimator::Fixed,
            inference: MetaInference::KnappHartung,
            confidence_level: confidence,
        },
        control,
    )?;
    fit.coefficients[0].term = "asymmetry_intercept".into();
    fit.coefficients[1].term = "precision".into();
    Ok(fit)
}
pub fn begg(y: &[f64], v: &[f64], control: &Control) -> Result<CorrelationResult> {
    let fit = model::summary(
        y,
        v,
        &[],
        MetaOptions {
            estimator: MetaEstimator::Fixed,
            ..Default::default()
        },
        control,
    )?;
    let theta = fit.coefficients[0].estimate;
    let pooled_variance = fit.covariance[0][0];
    let standardized = y
        .iter()
        .zip(v)
        .map(|(y, v)| finite((y - theta) / (v - pooled_variance).sqrt()))
        .collect::<Result<Vec<_>>>()?;
    crate::association::kendall(
        &standardized,
        v,
        RankCorrelationOptions {
            inference: RankInference::Asymptotic,
            ..Default::default()
        },
        control,
    )
}
pub fn combine_p(
    p_values: &[f64],
    weights: Option<&[f64]>,
    stouffer: bool,
    control: &Control,
) -> Result<CombinedP> {
    aligned(&[p_values], control)?;
    if p_values.iter().any(|p| !(0.0..=1.0).contains(p)) {
        return Err(parameter());
    }
    let n = p_values.len();
    if !stouffer {
        let statistic = -2. * p_values.iter().map(|p| p.ln()).sum::<f64>();
        let df = n.checked_mul(2).ok_or_else(parameter)?;
        return Ok(CombinedP {
            method: "fisher".into(),
            studies: n,
            statistic: statistic.is_finite().then_some(statistic),
            degrees_of_freedom: Some(df),
            p_value: if statistic.is_infinite() {
                0.
            } else {
                ChiSquared::new(df as f64)
                    .map_err(|_| failed())?
                    .sf(statistic)
            },
        });
    }
    let unit_weights;
    let weights = match weights {
        Some(weights) => weights,
        None => {
            unit_weights = vec![1.; n];
            &unit_weights
        }
    };
    aligned(&[p_values, weights], control)?;
    if weights.iter().any(|w| *w <= 0.) {
        return Err(parameter());
    }
    let scale = weights.iter().copied().fold(0., f64::max);
    let normal = Normal::new(0., 1.).expect("normal");
    let mut numerator = 0.;
    let mut denominator = 0.;
    for (p, w) in p_values.iter().zip(weights) {
        control.check()?;
        let w = w / scale;
        numerator += w * -normal.inverse_cdf(*p);
        denominator += w * w;
    }
    let statistic = numerator / denominator.sqrt();
    if statistic.is_nan() {
        return Err(parameter());
    }
    Ok(CombinedP {
        method: "stouffer".into(),
        studies: n,
        statistic: statistic.is_finite().then_some(statistic),
        degrees_of_freedom: None,
        p_value: normal.sf(statistic),
    })
}
pub fn leave_one_out(
    y: &[f64],
    v: &[f64],
    options: MetaOptions,
    control: &Control,
) -> Result<Vec<OmissionResult>> {
    if study_data(y, v, control)? < 3 {
        return Err(parameter());
    }
    let mut rows = Vec::with_capacity(y.len());
    for i in 0..y.len() {
        control.check()?;
        let subset = |values: &[f64]| {
            values
                .iter()
                .enumerate()
                .filter_map(|(j, &v)| (i != j).then_some(v))
                .collect::<Vec<_>>()
        };
        let fit = model::summary(&subset(y), &subset(v), &[], options, control)?;
        let c = &fit.coefficients[0];
        let ci = c.confidence_interval.ok_or_else(failed)?;
        rows.push(OmissionResult {
            omitted_study: i + 1,
            estimate: c.estimate,
            standard_error: c.standard_error.ok_or_else(failed)?,
            lower: ci[0],
            upper: ci[1],
            tau_squared: fit.heterogeneity.tau_squared,
            i_squared_percent: fit.heterogeneity.i_squared_percent,
        });
    }
    Ok(rows)
}
pub fn sensitivity(
    y: &[f64],
    v: &[f64],
    options: MetaOptions,
    control: &Control,
) -> Result<(SensitivitySummary, Vec<OmissionResult>)> {
    let baseline = model::summary(y, v, &[], options, control)?;
    let alternative_models = [
        MetaEstimator::Fixed,
        MetaEstimator::DerSimonianLaird,
        MetaEstimator::PauleMandel,
    ]
    .into_iter()
    .map(|estimator| {
        model::summary(
            y,
            v,
            &[],
            MetaOptions {
                estimator,
                ..options
            },
            control,
        )
    })
    .collect::<Result<_>>()?;
    Ok((
        SensitivitySummary {
            baseline,
            alternative_models,
        },
        leave_one_out(y, v, options, control)?,
    ))
}
