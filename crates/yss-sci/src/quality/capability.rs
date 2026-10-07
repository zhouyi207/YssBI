use super::process::{Process, pooled_sigma};
use super::*;
use statrs::distribution::{ContinuousCDF, Normal};
pub fn process_capability(
    values: &[f64],
    groups: Option<&[usize]>,
    limits: CapabilityLimits,
    control: &Control,
) -> Result<CapabilityResult> {
    control.check()?;
    if !limits.lower.is_finite() || !limits.upper.is_finite() || limits.lower >= limits.upper {
        return Err(parameter());
    }
    let p = Process::new(
        values,
        &[limits.lower, limits.upper, limits.target],
        control,
    )?;
    let (sigma, subgroups) = if let Some(groups) = groups {
        let (s, n) = pooled_sigma(&p.values, groups, control)?;
        (s, Some(n))
    } else {
        (p.within_sigma(), None)
    };
    let (lower, upper, target) = (
        limits.lower / p.scale,
        limits.upper / p.scale,
        limits.target / p.scale,
    );
    let ratio = |numerator: f64, denominator: f64| {
        if denominator > 0. {
            finite(numerator / denominator).map(Some)
        } else {
            Ok(None)
        }
    };
    let width = upper - lower;
    let nearest = (upper - p.mean).min(p.mean - lower);
    let normal = Normal::new(0., 1.).map_err(|_| failed())?;
    let ppm = |s: f64| {
        if s > 0. {
            Some(1e6 * (normal.cdf((lower - p.mean) / s) + normal.sf((upper - p.mean) / s)))
        } else {
            None
        }
    };
    let mut below = 0;
    let mut above = 0;
    for (i, &v) in values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        below += usize::from(v < limits.lower);
        above += usize::from(v > limits.upper);
    }
    Ok(CapabilityResult {
        observations: values.len(),
        within_method: if subgroups.is_some() {
            "pooled_subgroup_sample_sd"
        } else {
            "moving_range_d2"
        },
        subgroups,
        lower_specification: limits.lower,
        upper_specification: limits.upper,
        target: limits.target,
        mean: finite(p.mean * p.scale)?,
        overall_standard_deviation: finite(p.standard_deviation * p.scale)?,
        within_standard_deviation: finite(sigma * p.scale)?,
        cp: ratio(width, 6. * sigma)?,
        cpk: ratio(nearest, 3. * sigma)?,
        pp: ratio(width, 6. * p.standard_deviation)?,
        ppk: ratio(nearest, 3. * p.standard_deviation)?,
        cpm: ratio(width, 6. * p.standard_deviation.hypot(p.mean - target))?,
        below_specification: below,
        above_specification: above,
        observed_outside_percent: 100. * (below + above) as f64 / values.len() as f64,
        within_normal_ppm: ppm(sigma),
        overall_normal_ppm: ppm(p.standard_deviation),
    })
}
