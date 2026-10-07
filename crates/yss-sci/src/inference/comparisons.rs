//! Pooled-ANOVA or Welch pair contrasts, followed by one family-wise adjustment.
use super::intervals::{critical, critical_tail};
use crate::regression::models::common::{Result, finite, parameter};
use statrs::distribution::{ContinuousCDF, StudentsT};
use std::collections::BTreeMap;
use yss_sci_contract::{execution::*, inference::*};

#[derive(Default)]
struct Moments {
    count: usize,
    mean: f64,
    sum_squares: f64,
}

fn group_moments(
    y: &[f64],
    groups: &[usize],
    control: &ScientificExecutionControl,
) -> Result<BTreeMap<usize, Moments>> {
    control.check()?;
    if y.is_empty() || y.len() != groups.len() {
        return Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        });
    }
    let mut moments = BTreeMap::<usize, Moments>::new();
    for (i, (&value, &group)) in y.iter().zip(groups).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if !value.is_finite() || group == usize::MAX {
            return Err(parameter());
        }
        let m = moments.entry(group).or_default();
        m.count += 1;
        let delta = value - m.mean;
        m.mean = finite(m.mean + delta / m.count as f64)?;
        m.sum_squares = finite(m.sum_squares + delta * (value - m.mean))?;
    }
    Ok(moments)
}
fn contrast(
    a: &Moments,
    b: &Moments,
    pooled: Option<(f64, f64)>,
    tail: f64,
) -> Result<PairwiseRow> {
    let (variance, df) = if let Some((mse, df)) = pooled {
        (mse * (1. / a.count as f64 + 1. / b.count as f64), df)
    } else {
        if a.count < 2 || b.count < 2 {
            return Err(parameter());
        }
        let v1 = a.sum_squares / (a.count - 1) as f64 / a.count as f64;
        let v2 = b.sum_squares / (b.count - 1) as f64 / b.count as f64;
        let total = v1 + v2;
        let df =
            total.powi(2) / (v1.powi(2) / (a.count - 1) as f64 + v2.powi(2) / (b.count - 1) as f64);
        (total, df)
    };
    if variance <= 0. || !variance.is_finite() {
        return Err(parameter());
    }
    let estimate = finite(a.mean - b.mean)?;
    let se = variance.sqrt();
    let statistic = finite(estimate / se)?;
    let distribution = StudentsT::new(0., 1., df).map_err(|_| parameter())?;
    let q = critical_tail(tail, Some(df))?;
    let p_value = finite(2. * distribution.sf(statistic.abs()))?;
    Ok(PairwiseRow {
        group_a: 0,
        group_b: 0,
        estimate,
        standard_error: se,
        degrees_of_freedom: df,
        statistic,
        p_value,
        adjusted_p_value: p_value,
        lower: finite(estimate - q * se)?,
        upper: finite(estimate + q * se)?,
    })
}
fn adjust(
    rows: &mut [PairwiseRow],
    method: ComparisonAdjustment,
    control: &ScientificExecutionControl,
) -> Result<()> {
    let m = rows.len();
    match method {
        ComparisonAdjustment::None => {}
        ComparisonAdjustment::Bonferroni => {
            for row in rows {
                control.check()?;
                row.adjusted_p_value = (row.p_value * m as f64).min(1.);
            }
        }
        ComparisonAdjustment::Holm => {
            let mut order = (0..m).collect::<Vec<_>>();
            order.sort_by(|&a, &b| rows[a].p_value.total_cmp(&rows[b].p_value));
            let mut maximum = 0_f64;
            for (rank, index) in order.into_iter().enumerate() {
                control.check()?;
                maximum = maximum.max((m - rank) as f64 * rows[index].p_value).min(1.);
                rows[index].adjusted_p_value = maximum;
            }
        }
    }
    Ok(())
}
pub fn pairwise(
    y: &[f64],
    groups: &[usize],
    options: PairwiseOptions,
    control: &ScientificExecutionControl,
) -> Result<PairwiseResult> {
    critical(options.confidence_level, None)?;
    let moments = group_moments(y, groups, control)?;
    let k = moments.len();
    if k < 2 {
        return Err(parameter());
    }
    let count = k.checked_mul(k - 1).ok_or_else(parameter)? / 2;
    let pooled = if options.equal_variances {
        if y.len() <= k {
            return Err(parameter());
        }
        let df = (y.len() - k) as f64;
        Some((
            finite(moments.values().map(|m| m.sum_squares).sum::<f64>() / df)?,
            df,
        ))
    } else {
        None
    };
    let adjusted = options.adjustment == ComparisonAdjustment::Bonferroni;
    let tail = (1. - options.confidence_level) / 2. / if adjusted { count as f64 } else { 1. };
    let groups = moments.into_iter().collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(count);
    for (i, (a, ma)) in groups.iter().enumerate() {
        for (b, mb) in &groups[i + 1..] {
            control.check()?;
            let mut row = contrast(ma, mb, pooled, tail)?;
            row.group_a = a + 1;
            row.group_b = b + 1;
            rows.push(row);
        }
    }
    adjust(&mut rows, options.adjustment, control)?;
    let summaries = groups
        .into_iter()
        .map(|(group, m)| {
            Ok(ComparisonGroup {
                group: group + 1,
                observations: m.count,
                mean: m.mean,
                standard_deviation: if m.count > 1 {
                    Some(finite((m.sum_squares / (m.count - 1) as f64).sqrt())?)
                } else {
                    None
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PairwiseResult {
        summary: PairwiseSummary {
            observations: y.len(),
            groups: summaries,
            method: if options.equal_variances {
                "pooled_anova_t"
            } else {
                "welch_t"
            },
            adjustment: options.adjustment,
            confidence_level: options.confidence_level,
            comparisons: count,
            intervals_adjusted: adjusted,
        },
        rows,
    })
}
