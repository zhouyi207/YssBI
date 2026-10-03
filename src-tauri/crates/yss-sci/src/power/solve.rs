use super::*;
pub fn compute(model: PowerModel, options: PowerOptions, control: &Control) -> Result<PowerResult> {
    control.check()?;
    let minimum = design::validate(model, options)?;
    let (sample_size, target_power) = match options.request {
        PowerRequest::Power { sample_size } => {
            if sample_size < minimum || sample_size > design::MAX_EXACT_SIZE {
                return Err(parameter());
            }
            (sample_size, None)
        }
        PowerRequest::SampleSize { target_power } => {
            if !target_power.is_finite() || target_power <= 0. || target_power >= 1. {
                return Err(parameter());
            }
            (
                solve(model, options, minimum, target_power, control)?,
                Some(target_power),
            )
        }
    };
    let power = evaluate::power(
        model,
        sample_size,
        options.alpha,
        options.alternative,
        control,
    )?;
    let (unit, total_observations) = design::layout(model, sample_size)?;
    Ok(PowerResult {
        model,
        method: evaluate::method(model).into(),
        alpha: options.alpha,
        alternative: options.alternative,
        sample_size,
        sample_unit: unit.into(),
        total_observations,
        power,
        type_ii_error: 1. - power,
        target_power,
        expected_events: if let PowerModel::Survival { event_fraction, .. } = model {
            Some(finite(sample_size as f64 * event_fraction)?)
        } else {
            None
        },
    })
}
fn solve(
    model: PowerModel,
    options: PowerOptions,
    minimum: usize,
    target: f64,
    control: &Control,
) -> Result<usize> {
    let reaches = |n| {
        evaluate::power(model, n, options.alpha, options.alternative, control).map(|p| p >= target)
    };
    if reaches(minimum)? {
        return Ok(minimum);
    }
    if !design::increasing(model, options.alternative) {
        return Err(parameter());
    }
    let (_, multiplier) = design::layout(model, 1)?;
    let maximum = design::MAX_EXACT_SIZE / multiplier;
    let (mut low, mut high) = (minimum, minimum);
    loop {
        control.check()?;
        high = high.saturating_mul(2).min(maximum);
        if high <= low {
            return Err(parameter());
        }
        if reaches(high)? {
            break;
        }
        low = high;
    }
    while high - low > 1 {
        control.check()?;
        let mid = low + (high - low) / 2;
        if reaches(mid)? {
            high = mid;
        } else {
            low = mid;
        }
    }
    Ok(high)
}
