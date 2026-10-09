//! Tests for equality of group variances.
use super::{Error, Violation, checkpoint, failed, finite, invalid};
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::hypothesis::{ClassicalTestResult, VarianceHomogeneityTest as Input};

pub fn run(
    input: Input,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
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
) -> Result<ClassicalTestResult, Error> {
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
    finite(within)?;
    if within <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let df1 = (k - 1) as f64;
    let df2 = (n - k) as f64;
    let statistic = finite((between / df1) / (within / df2))?;
    control.check()?;
    // Reciprocal F swaps the degrees of freedom and avoids subtracting a CDF near one.
    let p = FisherSnedecor::new(df2, df1)
        .map_err(|_| failed())?
        .cdf(1.0 / statistic);
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
            .collect::<Result<_, Error>>()?,
    ))
}

fn bartlett(
    groups: Vec<Vec<f64>>,
    control: &ScientificExecutionControl,
) -> Result<ClassicalTestResult, Error> {
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
        let variance = finite(squares / df)?;
        if variance <= 0.0 {
            return Err(invalid(Violation::DataOutOfRange));
        }
        weighted_variance += df * variance;
        weighted_log += df * variance.ln();
        reciprocal_df += 1.0 / df;
    }
    let pooled = weighted_variance / (n - k) as f64;
    let numerator = (n - k) as f64 * pooled.ln() - weighted_log;
    let correction = 1.0 + (reciprocal_df - 1.0 / (n - k) as f64) / (3.0 * (k - 1) as f64);
    let statistic = finite(numerator / correction)?;
    let df = (k - 1) as f64;
    control.check()?;
    let p = ChiSquared::new(df).map_err(|_| failed())?.sf(statistic);
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
            .collect::<Result<_, Error>>()?,
    ))
}

fn validate_groups(groups: &[Vec<f64>], control: &ScientificExecutionControl) -> Result<(), Error> {
    if groups.len() < 2 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    for (i, group) in groups.iter().enumerate() {
        checkpoint(control, i)?;
        if group.len() < 2 {
            return Err(invalid(Violation::EmptyInput));
        }
        for (j, value) in group.iter().enumerate() {
            checkpoint(control, j)?;
            if !value.is_finite() {
                return Err(invalid(Violation::NonFiniteInput));
            }
        }
    }
    Ok(())
}
fn median(values: &[f64], control: &ScientificExecutionControl) -> Result<f64, Error> {
    let mut v = Vec::with_capacity(values.len());
    for (i, value) in values.iter().enumerate() {
        checkpoint(control, i)?;
        v.push(*value);
    }
    control.check()?;
    v.sort_by(f64::total_cmp);
    control.check()?;
    Ok(crate::descriptive::quantile_sorted(&v, 0.5))
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
