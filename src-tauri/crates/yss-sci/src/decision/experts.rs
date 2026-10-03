//! Descriptive summaries for one Delphi round; consultation process stays with researchers.
use super::data::*;
use crate::descriptive::quantile_sorted;
use yss_sci_contract::decision::experts::*;

pub fn delphi(columns: &[Vec<f64>], full_score: f64, control: &Control) -> Result<DelphiResult> {
    let (n, p) = dimensions(columns, &vec![false; columns.len()], control)?;
    if !full_score.is_finite() || columns.iter().flatten().any(|v| *v > full_score) {
        return Err(parameter());
    }
    let mut rows = Vec::with_capacity(p);
    for (j, x) in columns.iter().enumerate() {
        control.check()?;
        let mut sorted = x.clone();
        sorted.sort_unstable_by(f64::total_cmp);
        let scale = x
            .iter()
            .map(|v| v.abs())
            .fold(0., f64::max)
            .max(f64::MIN_POSITIVE);
        let scaled = x.iter().map(|v| v / scale).collect::<Vec<_>>();
        let (mean, sd) = if n == 1 {
            (scaled[0], None)
        } else {
            let (mean, sd) = moments(&scaled);
            (mean, Some(sd))
        };
        rows.push(DelphiItem {
            item: j + 1,
            mean: finite(mean * scale)?,
            standard_deviation: sd.map(|v| finite(v * scale)).transpose()?,
            coefficient_of_variation: sd
                .filter(|_| mean != 0.)
                .map(|v| finite(v / mean.abs()))
                .transpose()?,
            q1: quantile_sorted(&sorted, 0.25),
            median: quantile_sorted(&sorted, 0.5),
            q3: quantile_sorted(&sorted, 0.75),
            minimum: sorted[0],
            maximum: sorted[n - 1],
            full_score_percent: 100. * x.iter().filter(|&&v| v == full_score).count() as f64
                / n as f64,
        });
    }
    let (concordance, reason) = if n < 2 || p < 2 {
        (None, Some("insufficient_experts_or_items"))
    } else {
        let mut raters = Vec::with_capacity(n);
        let mut varying = false;
        for i in 0..n {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            let row = columns.iter().map(|x| x[i]).collect::<Vec<_>>();
            varying |= row.iter().any(|v| *v != row[0]);
            raters.push(row);
        }
        if varying {
            (Some(crate::association::kendall_w(&raters, control)?), None)
        } else {
            (None, Some("no_within_expert_variation"))
        }
    };
    control.check()?;
    Ok(DelphiResult {
        summary: DelphiSummary {
            experts: n,
            items: p,
            full_score,
            concordance,
            concordance_undefined_reason: reason,
        },
        rows,
    })
}
