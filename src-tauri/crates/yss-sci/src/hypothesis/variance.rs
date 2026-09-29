//! Tests for equality of group variances.
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use yss_sci_contract::hypothesis::{ClassicalTestResult, VarianceHomogeneityTest as Input};

pub fn run(input: Input) -> Result<ClassicalTestResult, String> {
    match input {
        Input::Levene { groups } => levene(groups, false),
        Input::BrownForsythe { groups } => levene(groups, true),
        Input::Bartlett { groups } => bartlett(groups),
    }
}

fn levene(groups: Vec<Vec<f64>>, median_center: bool) -> Result<ClassicalTestResult, String> {
    validate_groups(&groups)?;
    let centers = groups
        .iter()
        .map(|g| {
            if median_center {
                median(g)
            } else {
                g.iter().sum::<f64>() / g.len() as f64
            }
        })
        .collect::<Vec<_>>();
    let deviations = groups
        .iter()
        .zip(centers)
        .map(|(g, c)| g.iter().map(|x| (x - c).abs()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let n = deviations.iter().map(Vec::len).sum::<usize>();
    let k = groups.len();
    let means = deviations
        .iter()
        .map(|g| g.iter().sum::<f64>() / g.len() as f64)
        .collect::<Vec<_>>();
    let grand = deviations.iter().flatten().sum::<f64>() / n as f64;
    let between = deviations
        .iter()
        .zip(&means)
        .map(|(g, m)| g.len() as f64 * (m - grand).powi(2))
        .sum::<f64>();
    let within = deviations
        .iter()
        .zip(&means)
        .map(|(g, m)| g.iter().map(|z| (z - m).powi(2)).sum::<f64>())
        .sum::<f64>();
    if within <= 0.0 {
        return Err("Levene within-group deviation is zero".into());
    }
    let df1 = (k - 1) as f64;
    let df2 = (n - k) as f64;
    let statistic = (between / df1) / (within / df2);
    let p = 1.0
        - FisherSnedecor::new(df1, df2)
            .map_err(|_| "invalid Levene F distribution")?
            .cdf(statistic);
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
        groups.iter().map(Vec::len).collect(),
    ))
}

fn bartlett(groups: Vec<Vec<f64>>) -> Result<ClassicalTestResult, String> {
    validate_groups(&groups)?;
    let k = groups.len();
    let n = groups.iter().map(Vec::len).sum::<usize>();
    let variances = groups
        .iter()
        .map(|g| {
            let m = g.iter().sum::<f64>() / g.len() as f64;
            g.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (g.len() - 1) as f64
        })
        .collect::<Vec<_>>();
    if variances.iter().any(|v| *v <= 0.0 || !v.is_finite()) {
        return Err("Bartlett requires positive finite group variances".into());
    }
    let pooled = groups
        .iter()
        .zip(&variances)
        .map(|(g, v)| (g.len() - 1) as f64 * v)
        .sum::<f64>()
        / (n - k) as f64;
    let numerator = (n - k) as f64 * pooled.ln()
        - groups
            .iter()
            .zip(&variances)
            .map(|(g, v)| (g.len() - 1) as f64 * v.ln())
            .sum::<f64>();
    let correction = 1.0
        + (groups
            .iter()
            .map(|g| 1.0 / (g.len() - 1) as f64)
            .sum::<f64>()
            - 1.0 / (n - k) as f64)
            / (3.0 * (k - 1) as f64);
    let statistic = numerator / correction;
    let df = (k - 1) as f64;
    let p = 1.0
        - ChiSquared::new(df)
            .map_err(|_| "invalid Bartlett chi-square")?
            .cdf(statistic);
    Ok(result(
        "bartlett",
        statistic,
        df,
        0.0,
        p,
        groups.iter().map(Vec::len).collect(),
    ))
}

fn validate_groups(groups: &[Vec<f64>]) -> Result<(), String> {
    if groups.len() < 2
        || groups
            .iter()
            .any(|g| g.len() < 2 || g.iter().any(|x| !x.is_finite()))
    {
        Err("variance test requires at least two groups with two finite observations each".into())
    } else {
        Ok(())
    }
}
fn median(values: &[f64]) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    if v.len() % 2 == 0 {
        (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
    } else {
        v[v.len() / 2]
    }
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
        statistic: stat,
        degrees_of_freedom: if df2 > 0.0 { vec![df1, df2] } else { vec![df1] },
        p_value: p.clamp(0.0, 1.0),
        estimate: None,
        standard_error: None,
        sample_sizes: n,
        details: Default::default(),
    }
}
