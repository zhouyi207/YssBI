//! Tests for equality of group variances.
use super::checkpoint;
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use yss_sci_contract::hypothesis::{ClassicalTestResult, VarianceHomogeneityTest as Input};
use yss_sci_contract::{execution::ScientificExecutionControl, hypothesis::HypothesisError};

pub fn run(
    input: Input,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, HypothesisError> {
    control.check()?;
    let result = match input {
        Input::Levene { groups } => levene(groups, false, control),
        Input::BrownForsythe { groups } => levene(groups, true, control),
        Input::Bartlett { groups } => bartlett(groups, control),
    };
    control.check()?;
    result
}

fn levene(
    groups: Vec<Vec<f64>>,
    median_center: bool,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, HypothesisError> {
    validate_groups(&groups, control)?;
    let k = groups.len();
    let mut n = 0;
    let mut deviations = Vec::with_capacity(k);
    let mut means = Vec::with_capacity(k);
    let mut grand_sum = 0.0;
    for (i, group) in groups.iter().enumerate() {
        checkpoint(control, i)?;
        let center = if median_center {
            median(group, control)?
        } else {
            let mut sum = 0.0;
            for (j, value) in group.iter().enumerate() {
                checkpoint(control, j)?;
                sum += value;
            }
            sum / group.len() as f64
        };
        let mut deviation = Vec::with_capacity(group.len());
        let mut sum = 0.0;
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            let value = (value - center).abs();
            deviation.push(value);
            sum += value;
            grand_sum += value;
        }
        n += group.len();
        means.push(sum / group.len() as f64);
        deviations.push(deviation);
    }
    let grand = grand_sum / n as f64;
    let mut between = 0.0;
    let mut within = 0.0;
    for (i, (group, mean)) in deviations.iter().zip(&means).enumerate() {
        checkpoint(control, i)?;
        between += group.len() as f64 * (mean - grand).powi(2);
        let mut group_within = 0.0;
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            group_within += (value - mean).powi(2);
        }
        within += group_within;
    }
    if within <= 0.0 {
        return Err("Levene within-group deviation is zero".into());
    }
    let df1 = (k - 1) as f64;
    let df2 = (n - k) as f64;
    let statistic = (between / df1) / (within / df2);
    control.check()?;
    let p = 1.0
        - FisherSnedecor::new(df1, df2)
            .map_err(|_| "invalid Levene F distribution")?
            .cdf(statistic);
    control.check()?;
    Ok(result(
        if median_center {
            "brown_forsythe"
        } else {
            "levene"
        },
        statistic,
        df1,
        df2,
        p,
        groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                checkpoint(control, i)?;
                Ok(g.len())
            })
            .collect::<Result<_, HypothesisError>>()?,
    ))
}

fn bartlett(
    groups: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, HypothesisError> {
    validate_groups(&groups, control)?;
    let k = groups.len();
    let mut n = 0;
    let mut weighted_variance = 0.0;
    let mut weighted_log = 0.0;
    let mut reciprocal_df = 0.0;
    for (i, group) in groups.iter().enumerate() {
        checkpoint(control, i)?;
        n += group.len();
        let mut sum = 0.0;
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            sum += value;
        }
        let mean = sum / group.len() as f64;
        let mut squares = 0.0;
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            squares += (value - mean).powi(2);
        }
        let df = (group.len() - 1) as f64;
        let variance = squares / df;
        if variance <= 0.0 || !variance.is_finite() {
            return Err("Bartlett requires positive finite group variances".into());
        }
        weighted_variance += df * variance;
        weighted_log += df * variance.ln();
        reciprocal_df += 1.0 / df;
    }
    let pooled = weighted_variance / (n - k) as f64;
    let numerator = (n - k) as f64 * pooled.ln() - weighted_log;
    let correction = 1.0 + (reciprocal_df - 1.0 / (n - k) as f64) / (3.0 * (k - 1) as f64);
    let statistic = numerator / correction;
    let df = (k - 1) as f64;
    control.check()?;
    let p = 1.0
        - ChiSquared::new(df)
            .map_err(|_| "invalid Bartlett chi-square")?
            .cdf(statistic);
    control.check()?;
    Ok(result(
        "bartlett",
        statistic,
        df,
        0.0,
        p,
        groups
            .iter()
            .enumerate()
            .map(|(i, g)| {
                checkpoint(control, i)?;
                Ok(g.len())
            })
            .collect::<Result<_, HypothesisError>>()?,
    ))
}

fn validate_groups(
    groups: &[Vec<f64>],
    control: &ScientificExecutionControl,
) -> Result<(), HypothesisError> {
    if groups.len() < 2 {
        return Err(
            "variance test requires at least two groups with two finite observations each".into(),
        );
    }
    for (i, group) in groups.iter().enumerate() {
        checkpoint(control, i)?;
        if group.len() < 2 {
            return Err(
                "variance test requires at least two groups with two finite observations each"
                    .into(),
            );
        }
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            if !value.is_finite() {
                return Err(
                    "variance test requires at least two groups with two finite observations each"
                        .into(),
                );
            }
        }
    }
    Ok(())
}
fn median(values: &[f64], control: &ScientificExecutionControl) -> Result<f64, HypothesisError> {
    let mut v = Vec::with_capacity(values.len());
    for (i, value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        v.push(*value);
    }
    control.check()?;
    v.sort_by(f64::total_cmp);
    control.check()?;
    Ok(if v.len() % 2 == 0 {
        (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
    } else {
        v[v.len() / 2]
    })
}

fn result(
    method: &str,
    stat: f64,
    df1: f64,
    df2: f64,
    p: f64,
    n: Vec<usize>,
) -> ClassicalTestResult {
    ClassicalTestResult {
        method: method.into(),
        null_hypothesis: "all population variances are equal".into(),
        alternative: "at least one variance differs".into(),
        statistic_name: if method == "bartlett" {
            "chi_squared"
        } else {
            "F"
        }
        .into(),
        statistic: Some(stat),
        degrees_of_freedom: if df2 > 0.0 { vec![df1, df2] } else { vec![df1] },
        p_value: p.clamp(0.0, 1.0),
        estimate: None,
        standard_error: None,
        sample_sizes: n,
        details: Default::default(),
    }
}
