use super::*;
use std::collections::BTreeMap;
pub fn range_analysis(
    response: &[f64],
    factors: &[Vec<usize>],
    maximize: bool,
    control: &Control,
) -> Result<RangeAnalysisResult> {
    validate(response, &[], control)?;
    if factors.is_empty() {
        return Err(parameter());
    }
    let n = response.len();
    let scale = response
        .iter()
        .map(|v| v.abs())
        .fold(0., f64::max)
        .max(f64::MIN_POSITIVE);
    let mut rows = Vec::new();
    let mut effects = Vec::with_capacity(factors.len());
    let mut margins = Vec::with_capacity(factors.len());
    for (j, factor) in factors.iter().enumerate() {
        control.check()?;
        let (counts, sums) = aggregate(response, factor, scale, control)?;
        let means: Vec<_> = counts
            .iter()
            .zip(&sums)
            .map(|(&count, &sum)| sum / count as f64)
            .collect();
        let low = means.iter().copied().fold(f64::INFINITY, f64::min);
        let high = means.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let tolerance = 64. * f64::EPSILON * low.abs().max(high.abs());
        let range = if high - low <= tolerance {
            0.
        } else {
            finite((high - low) * scale)?
        };
        let best = if maximize { high } else { low };
        let optimal_levels = means
            .iter()
            .enumerate()
            .filter_map(|(l, &v)| ((v - best).abs() <= tolerance).then_some(l + 1))
            .collect();
        for (l, (&count, (&sum, &mean))) in counts.iter().zip(sums.iter().zip(&means)).enumerate() {
            if l.is_multiple_of(1024) {
                control.check()?;
            }
            rows.push(RangeLevel {
                factor: j + 1,
                level: l + 1,
                observations: count,
                total: finite(sum * scale)?,
                mean: finite(mean * scale)?,
            });
        }
        effects.push(FactorRange {
            factor: j + 1,
            levels: counts.len(),
            range,
            balanced: counts.iter().all(|v| *v == counts[0]),
            optimal_levels,
        });
        margins.push(counts);
    }
    let pairwise_orthogonal = if factors.len() > 1 {
        Some(orthogonal(factors, &margins, control)?)
    } else {
        None
    };
    let equal_level_counts = effects.iter().all(|e| e.levels == effects[0].levels);
    Ok(RangeAnalysisResult {
        summary: RangeAnalysisSummary {
            observations: n,
            maximize,
            pairwise_orthogonal,
            equal_level_counts,
            factors: effects,
        },
        rows,
    })
}
fn aggregate(
    response: &[f64],
    factor: &[usize],
    scale: f64,
    control: &Control,
) -> Result<(Vec<usize>, Vec<f64>)> {
    if factor.len() != response.len() {
        return Err(parameter());
    }
    let levels = factor
        .iter()
        .copied()
        .max()
        .and_then(|v| v.checked_add(1))
        .ok_or_else(parameter)?;
    if levels > response.len() {
        return Err(parameter());
    }
    let mut counts = vec![0; levels];
    let mut sums = vec![0.; levels];
    let mut correction = vec![0.; levels];
    for (i, (&value, &code)) in response.iter().zip(factor).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        counts[code] += 1;
        let adjusted = value / scale - correction[code];
        let next = sums[code] + adjusted;
        correction[code] = (next - sums[code]) - adjusted;
        sums[code] = next;
    }
    if counts.contains(&0) {
        return Err(parameter());
    }
    Ok((counts, sums))
}
fn orthogonal(factors: &[Vec<usize>], margins: &[Vec<usize>], control: &Control) -> Result<bool> {
    for (j, x) in factors.iter().enumerate() {
        for (k, y) in factors[..j].iter().enumerate() {
            control.check()?;
            let mut cells = BTreeMap::<(usize, usize), usize>::new();
            for (i, (&a, &b)) in x.iter().zip(y).enumerate() {
                if i.is_multiple_of(1024) {
                    control.check()?;
                }
                *cells.entry((a, b)).or_default() += 1;
            }
            if Some(cells.len()) != margins[j].len().checked_mul(margins[k].len()) {
                return Ok(false);
            }
            for ((a, b), count) in cells {
                if count as u128 * x.len() as u128 != margins[j][a] as u128 * margins[k][b] as u128
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}
