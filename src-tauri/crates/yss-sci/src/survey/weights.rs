use super::*;
pub fn sampling_weights(
    values: &[f64],
    probabilities: bool,
    control: &Control,
) -> Result<SurveyWeights> {
    control.check()?;
    if values.is_empty() {
        return Err(parameter());
    }
    let mut weights = Vec::with_capacity(values.len());
    for (i, &v) in values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if !v.is_finite() || v <= 0. || (probabilities && v > 1.) {
            return Err(parameter());
        }
        weights.push(finite(if probabilities { 1. / v } else { v })?);
    }
    let summary = summary(&weights, control)?;
    Ok(SurveyWeights { weights, summary })
}
pub(super) fn summary(weights: &[f64], control: &Control) -> Result<WeightSummary> {
    control.check()?;
    if weights.is_empty() || weights.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(parameter());
    }
    let maximum = weights.iter().copied().fold(0., f64::max);
    let minimum = weights.iter().copied().fold(f64::INFINITY, f64::min);
    let scaled_sum = weights.iter().map(|v| v / maximum).sum::<f64>();
    let squares = weights.iter().map(|v| (v / maximum).powi(2)).sum::<f64>();
    let kish_effective_n = finite(scaled_sum * scaled_sum / squares)?;
    let weight_design_effect = finite(weights.len() as f64 / kish_effective_n)?;
    Ok(WeightSummary {
        observations: weights.len(),
        sum: finite(scaled_sum * maximum)?,
        minimum,
        maximum,
        kish_effective_n,
        weight_design_effect,
        coefficient_of_variation: (weight_design_effect - 1.).max(0.).sqrt(),
    })
}
