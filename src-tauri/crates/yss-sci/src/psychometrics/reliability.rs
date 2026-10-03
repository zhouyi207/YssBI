use super::*;

pub(super) struct ScaleStatistics {
    pub report: ReliabilityResult,
    pub totals: Vec<f64>,
    pub scale: f64,
}
pub fn reliability(columns: &[Vec<f64>], control: &Control) -> Result<ReliabilityResult> {
    Ok(analyze(columns, control)?.report)
}
pub(super) fn analyze(columns: &[Vec<f64>], control: &Control) -> Result<ScaleStatistics> {
    let Some(first) = columns.first() else {
        return Err(parameter());
    };
    validate(first, columns, control)?;
    let (n, p) = (first.len(), columns.len());
    if n < 2 || p < 2 {
        return Err(parameter());
    }
    let scale = columns
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0., f64::max)
        .max(f64::MIN_POSITIVE);
    let mut totals = vec![0.; n];
    let mut standardized_totals = vec![0.; n];
    let mut trace = 0.;
    let mut all_vary = true;
    let mut rows = Vec::with_capacity(p);
    // Alpha needs total-score variance and the diagonal variances, not a dense covariance matrix.
    for (j, column) in columns.iter().enumerate() {
        control.check()?;
        let scaled: Vec<_> = column.iter().map(|v| v / scale).collect();
        let (mean, variance) = moments(&scaled);
        trace += variance;
        all_vary &= variance > 0.;
        for (i, (&raw, total)) in scaled.iter().zip(&mut totals).enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            *total += raw;
            if variance > 0. {
                standardized_totals[i] += (raw - mean) / variance.sqrt();
            }
        }
        rows.push(ReliabilityItem {
            item: j + 1,
            mean: finite(mean * scale)?,
            standard_deviation: finite(variance.sqrt() * scale)?,
            corrected_item_total_correlation: None,
            alpha_if_deleted: None,
        });
    }
    let (total_mean, total_variance) = moments(&totals);
    for (j, column) in columns.iter().enumerate() {
        control.check()?;
        let mut item = Vec::with_capacity(n);
        let mut rest = Vec::with_capacity(n);
        for (i, (&raw, &total)) in column.iter().zip(&totals).enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            item.push(raw / scale);
            rest.push(total - raw / scale);
        }
        let (mean, variance) = moments(&item);
        let (rest_mean, rest_variance) = moments(&rest);
        let covariance = item
            .iter()
            .zip(&rest)
            .map(|(a, b)| (a - mean) * (b - rest_mean))
            .sum::<f64>()
            / (n - 1) as f64;
        rows[j].corrected_item_total_correlation = if variance > 0. && rest_variance > 0. {
            Some(finite(covariance / variance.sqrt() / rest_variance.sqrt())?.clamp(-1., 1.))
        } else {
            None
        };
        rows[j].alpha_if_deleted = alpha(p - 1, (trace - variance).max(0.), rest_variance)?;
    }
    let standardized_alpha = if all_vary {
        alpha(p, p as f64, moments(&standardized_totals).1)?
    } else {
        None
    };
    Ok(ScaleStatistics {
        report: ReliabilityResult {
            summary: ReliabilitySummary {
                method: "cronbach_alpha",
                observations: n,
                items: p,
                raw_alpha: alpha(p, trace, total_variance)?,
                standardized_alpha,
                total_mean: finite(total_mean * scale)?,
                total_standard_deviation: finite(total_variance.sqrt() * scale)?,
            },
            rows,
        },
        totals,
        scale,
    })
}
fn alpha(items: usize, trace: f64, variance: f64) -> Result<Option<f64>> {
    if items < 2 || variance <= 0. {
        Ok(None)
    } else {
        finite(items as f64 / (items - 1) as f64 * (1. - trace / variance)).map(Some)
    }
}
pub(super) fn moments(values: &[f64]) -> (f64, f64) {
    let anchor = values[0];
    let offset = values
        .iter()
        .map(|x| (x - anchor) / values.len() as f64)
        .sum::<f64>();
    let variance = if values.len() > 1 {
        values
            .iter()
            .map(|x| ((x - anchor) - offset).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64
    } else {
        0.
    };
    (anchor + offset, variance)
}
