use super::*;
use yss_sci_contract::association::{MAX_ASSOCIATION_CATEGORIES, RiditCategory, RiditResult};

pub fn ridit(
    sample: &[usize],
    reference: &[usize],
    categories: usize,
    alternative_kind: Alternative,
    continuity: bool,
    control: &Control,
) -> Result<RiditResult, Error> {
    control.check()?;
    if sample.is_empty()
        || reference.is_empty()
        || categories == 0
        || categories > MAX_ASSOCIATION_CATEGORIES
    {
        return Err(invalid());
    }
    let mut a = vec![0usize; categories];
    let mut b = vec![0usize; categories];
    for (values, counts) in [(sample, &mut a), (reference, &mut b)] {
        for (i, &value) in values.iter().enumerate() {
            checkpoint(control, i)?;
            if value >= categories {
                return Err(invalid());
            }
            counts[value] += 1;
        }
    }
    let n = sample.len() as f64;
    let m = reference.len() as f64;
    let mut previous = 0usize;
    let mut mean = Sum::default();
    let mut ties = Sum::default();
    let mut output = Vec::with_capacity(categories);
    for category in 0..categories {
        let score = (previous as f64 + 0.5 * b[category] as f64) / m;
        mean.add(score * a[category] as f64 / n);
        previous += b[category];
        let count = (a[category] + b[category]) as f64;
        ties.add(count * (count - 1.0) * (count + 1.0));
        output.push(RiditCategory {
            category,
            reference_count: b[category],
            sample_count: a[category],
            reference_proportion: b[category] as f64 / m,
            sample_proportion: a[category] as f64 / n,
            ridit: score,
        });
    }
    let mean = bounded(mean.total, 0.0, 1.0)?;
    let u = finite(n * m * mean)?;
    let total = n + m;
    let variance = n * m / 12.0 * (total + 1.0 - ties.total / (total * (total - 1.0)));
    let (statistic, p) = if variance > 0.0 {
        let difference = u - n * m / 2.0;
        let correction = if continuity {
            match alternative_kind {
                Alternative::TwoSided => 0.5 * difference.signum(),
                Alternative::Greater => 0.5,
                Alternative::Less => -0.5,
            }
        } else {
            0.0
        };
        let z = finite((difference - correction) / variance.sqrt())?;
        (Some(z), Some(normal_tail(z, alternative_kind)?))
    } else {
        (None, None)
    };
    control.check()?;
    Ok(RiditResult {
        method: "ridit",
        reference_observations: reference.len(),
        sample_observations: sample.len(),
        mean_ridit: mean,
        alternative: alternative(alternative_kind),
        continuity_correction: continuity,
        inference: TestInference {
            statistic_name: "z",
            statistic,
            degrees_of_freedom: vec![],
            p_value: p,
            method: "mann_whitney_normal_with_ties",
        },
        categories: output,
    })
}
