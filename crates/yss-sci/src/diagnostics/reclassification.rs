//! Paired binary-outcome NRI and IDI point estimates, with explicit risk categories.
use crate::regression::models::common::{Result, finite, parameter, validate};
use yss_sci_contract::{diagnostics::model::*, execution::ScientificExecutionControl as Control};

pub fn nri_idi(
    outcome: &[f64],
    reference: &[f64],
    new: &[f64],
    mode: ReclassificationMode,
    thresholds: &[f64],
    control: &Control,
) -> Result<ReclassificationResult> {
    control.check()?;
    if outcome.len() != reference.len() || outcome.len() != new.len() {
        return Err(crate::regression::models::common::invalid(
            yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        ));
    }
    validate(outcome, &[], control)?;
    if outcome.iter().any(|v| *v != 0.0 && *v != 1.0)
        || reference
            .iter()
            .chain(new)
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        || (mode == ReclassificationMode::Continuous && !thresholds.is_empty())
        || (mode == ReclassificationMode::Categorical && thresholds.is_empty())
        || thresholds
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v >= 1.0)
        || thresholds.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(parameter());
    }
    let mut counts = [0, 0];
    let mut up = [0, 0];
    let mut down = [0, 0];
    let mut reference_sum = [0.0, 0.0];
    let mut new_sum = [0.0, 0.0];
    for i in 0..outcome.len() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let group = outcome[i] as usize;
        counts[group] += 1;
        reference_sum[group] += reference[i];
        new_sum[group] += new[i];
        let ordering = match mode {
            ReclassificationMode::Continuous => new[i]
                .partial_cmp(&reference[i])
                .expect("finite probabilities"),
            ReclassificationMode::Categorical => {
                let category = |p: f64| thresholds.partition_point(|&cut| p >= cut);
                category(new[i]).cmp(&category(reference[i]))
            }
        };
        match ordering {
            std::cmp::Ordering::Greater => up[group] += 1,
            std::cmp::Ordering::Less => down[group] += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    if counts.contains(&0) {
        return Err(parameter());
    }
    let describe = |g: usize| ReclassificationCounts {
        observations: counts[g],
        upward: up[g],
        downward: down[g],
        unchanged: counts[g] - up[g] - down[g],
    };
    let nri_events = (up[1] as f64 - down[1] as f64) / counts[1] as f64;
    let nri_nonevents = (down[0] as f64 - up[0] as f64) / counts[0] as f64;
    let slope = |sum: [f64; 2]| sum[1] / counts[1] as f64 - sum[0] / counts[0] as f64;
    let reference_slope = finite(slope(reference_sum))?;
    let new_slope = finite(slope(new_sum))?;
    Ok(ReclassificationResult {
        mode,
        thresholds: thresholds.to_vec(),
        events: describe(1),
        nonevents: describe(0),
        nri_events,
        nri_nonevents,
        nri: nri_events + nri_nonevents,
        discrimination_slope_reference: reference_slope,
        discrimination_slope_new: new_slope,
        idi: new_slope - reference_slope,
    })
}
