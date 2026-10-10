//! Pooled-ANOVA or Welch pair contrasts, followed by one family-wise adjustment.
use super::intervals::{critical, critical_tail, validate_confidence};
use crate::regression::models::common::{Result, finite, parameter};
use statrs::distribution::StudentsT;
use std::collections::BTreeMap;
use yss_sci_contract::{execution::*, inference::*};

#[derive(Default)]
struct Moments {
    count: usize,
    scale: f64,
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
        if value.abs() > m.scale {
            let ratio = m.scale / value.abs();
            m.mean *= ratio;
            m.sum_squares = (m.sum_squares * ratio) * ratio;
            m.scale = value.abs();
        }
        let value = if m.scale > 0. { value / m.scale } else { 0. };
        let delta = value - m.mean;
        m.mean = finite(m.mean + delta / m.count as f64)?;
        m.sum_squares = finite(m.sum_squares + delta * (value - m.mean))?;
    }
    Ok(moments)
}
fn comparison_critical(confidence: f64, family_size: usize, df: f64) -> Result<f64> {
    if family_size == 1 {
        critical(confidence, Some(df))
    } else {
        critical_tail((1. - confidence) / 2. / family_size as f64, Some(df))
    }
}
fn pooled_deviation(
    groups: &[ComparisonGroup],
    control: &ScientificExecutionControl,
) -> Result<f64> {
    let scale = groups
        .iter()
        .filter_map(|g| g.standard_deviation)
        .fold(0., f64::max);
    if scale <= 0. {
        return Err(parameter());
    }
    let (mut weight, mut variance) = (0., 0.);
    for (i, group) in groups.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if let Some(sd) = group.standard_deviation {
            let next = (group.observations - 1) as f64;
            weight += next;
            variance += ((sd / scale).powi(2) - variance) * (next / weight);
        }
    }
    finite(scale * variance.sqrt())
}
fn welch_standard_error(a: &ComparisonGroup, b: &ComparisonGroup) -> Result<(f64, f64)> {
    let sa = a.standard_deviation.ok_or_else(parameter)?;
    let sb = b.standard_deviation.ok_or_else(parameter)?;
    let scale = sa.max(sb);
    if scale <= 0. {
        return Err(parameter());
    }
    let va = (sa / scale).powi(2) / a.observations as f64;
    let vb = (sb / scale).powi(2) / b.observations as f64;
    let total = va + vb;
    // Relative variance contributions keep Welch's fourth powers dimensionless.
    let df = finite(
        ((va / total).powi(2) / (a.observations - 1) as f64
            + (vb / total).powi(2) / (b.observations - 1) as f64)
            .recip(),
    )?;
    Ok((finite(scale * total.sqrt())?, df))
}
fn contrast(
    a: &ComparisonGroup,
    b: &ComparisonGroup,
    se: f64,
    df: f64,
    q: f64,
) -> Result<PairwiseRow> {
    if se <= 0. || !se.is_finite() {
        return Err(parameter());
    }
    let estimate = finite(a.mean - b.mean)?;
    let statistic = finite(estimate / se)?;
    let distribution = StudentsT::new(0., 1., df).map_err(|_| parameter())?;
    let p_value = finite(crate::distribution::student_t_probability(
        &distribution,
        statistic,
        yss_sci_contract::hypothesis::Alternative::TwoSided,
    ))?;
    Ok(PairwiseRow {
        group_a: a.group,
        group_b: b.group,
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
    validate_confidence(options.confidence_level)?;
    let moments = group_moments(y, groups, control)?;
    let k = moments.len();
    if k < 2 {
        return Err(parameter());
    }
    let count = k.checked_mul(k - 1).ok_or_else(parameter)? / 2;
    let adjusted = options.adjustment == ComparisonAdjustment::Bonferroni;
    let family_size = if adjusted { count } else { 1 };
    let groups = moments
        .into_iter()
        .enumerate()
        .map(|(i, (group, m))| {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            Ok(ComparisonGroup {
                group: group + 1,
                observations: m.count,
                mean: finite(m.mean * m.scale)?,
                standard_deviation: if m.count > 1 {
                    Some(finite(
                        (m.sum_squares / (m.count - 1) as f64).sqrt() * m.scale,
                    )?)
                } else {
                    None
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let pooled = if options.equal_variances {
        if y.len() <= k {
            return Err(parameter());
        }
        let df = (y.len() - k) as f64;
        Some((
            pooled_deviation(&groups, control)?,
            df,
            comparison_critical(options.confidence_level, family_size, df)?,
        ))
    } else {
        None
    };
    let mut rows = Vec::with_capacity(count);
    for (i, a) in groups.iter().enumerate() {
        for b in &groups[i + 1..] {
            control.check()?;
            let (se, df, q) = if let Some((sd, df, q)) = pooled {
                (
                    sd * (1. / a.observations as f64 + 1. / b.observations as f64).sqrt(),
                    df,
                    q,
                )
            } else {
                let (se, df) = welch_standard_error(a, b)?;
                (
                    se,
                    df,
                    comparison_critical(options.confidence_level, family_size, df)?,
                )
            };
            rows.push(contrast(a, b, se, df, q)?);
        }
    }
    adjust(&mut rows, options.adjustment, control)?;
    Ok(PairwiseResult {
        summary: PairwiseSummary {
            observations: y.len(),
            groups,
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
