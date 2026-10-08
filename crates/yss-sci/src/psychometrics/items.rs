use super::reliability::{analyze, moments};
use super::*;
use crate::{descriptive::quantile_sorted, hypothesis::sample_mean};
use yss_sci_contract::hypothesis::{Alternative, ClassicalHypothesisTest, HypothesisError};
pub fn item_analysis(
    columns: &[Vec<f64>],
    tail_fraction: f64,
    control: &Control,
) -> Result<DiscriminationResult> {
    control.check()?;
    if !tail_fraction.is_finite() || tail_fraction <= 0. || tail_fraction >= 0.5 {
        return Err(parameter());
    }
    let analyzed = analyze(columns, control)?;
    let mut sorted = analyzed.totals.clone();
    sorted.sort_by(f64::total_cmp);
    let low = quantile_sorted(&sorted, tail_fraction);
    let high = quantile_sorted(&sorted, 1. - tail_fraction);
    let groups: Vec<i8> = analyzed
        .totals
        .iter()
        .map(|&v| {
            if v < low || (low < high && v == low) {
                -1
            } else if v > high || (low < high && v == high) {
                1
            } else {
                0
            }
        })
        .collect();
    let low_count = groups.iter().filter(|&&g| g == -1).count();
    let high_count = groups.iter().filter(|&&g| g == 1).count();
    let mut rows = Vec::with_capacity(columns.len());
    for (column, reliability) in columns.iter().zip(analyzed.report.rows) {
        control.check()?;
        let mut lower = Vec::with_capacity(low_count);
        let mut upper = Vec::with_capacity(high_count);
        for (i, (&v, &g)) in column.iter().zip(&groups).enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if g == -1 {
                lower.push(v / analyzed.scale)
            } else if g == 1 {
                upper.push(v / analyzed.scale)
            }
        }
        let lm = (!lower.is_empty()).then(|| moments(&lower));
        let hm = (!upper.is_empty()).then(|| moments(&upper));
        let inference =
            if lower.len() >= 2 && upper.len() >= 2 && lm.unwrap().1 + hm.unwrap().1 > 0. {
                Some(
                    sample_mean::run(
                        ClassicalHypothesisTest::Independent {
                            first: upper,
                            second: lower,
                            equal_variance: false,
                            alternative: Alternative::TwoSided,
                        },
                        control,
                    )
                    .map_err(|error| match error {
                        HypothesisError::Execution(error) => error,
                        _ => failed(),
                    })?,
                )
            } else {
                None
            };
        rows.push(DiscriminationItem {
            reliability,
            low_mean: lm.map(|m| finite(m.0 * analyzed.scale)).transpose()?,
            high_mean: hm.map(|m| finite(m.0 * analyzed.scale)).transpose()?,
            t_statistic: inference.as_ref().and_then(|t| t.statistic),
            degrees_of_freedom: inference.as_ref().map(|t| t.degrees_of_freedom[0]),
            p_value: inference.as_ref().map(|t| t.p_value),
        });
    }
    let scores = analyzed
        .totals
        .iter()
        .zip(&groups)
        .enumerate()
        .map(|(i, (&total, &group))| {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            Ok(ItemScore {
                observation: i + 1,
                total: finite(total * analyzed.scale)?,
                group,
            })
        })
        .collect::<Result<_>>()?;
    Ok(DiscriminationResult {
        summary: DiscriminationSummary {
            reliability: analyzed.report.summary,
            tail_fraction,
            low_cutoff: finite(low * analyzed.scale)?,
            high_cutoff: finite(high * analyzed.scale)?,
            low_count,
            high_count,
            tail_test: "welch_high_minus_low_two_sided",
        },
        rows,
        scores,
    })
}
