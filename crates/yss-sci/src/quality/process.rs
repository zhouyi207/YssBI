use super::*;
pub(super) const MR_D2: f64 = 1.128;
pub(super) const MR_D4: f64 = 3.267;
pub(super) struct Process {
    pub values: Vec<f64>,
    pub scale: f64,
    pub mean: f64,
    pub standard_deviation: f64,
    pub mean_moving_range: f64,
}
impl Process {
    pub fn new(values: &[f64], references: &[f64], control: &Control) -> Result<Self> {
        validate(values, &[], control)?;
        if values.len() < 2 || references.iter().any(|v| !v.is_finite()) {
            return Err(parameter());
        }
        let scale = values
            .iter()
            .chain(references)
            .map(|v| v.abs())
            .fold(0., f64::max)
            .max(f64::MIN_POSITIVE);
        let values: Vec<_> = values.iter().map(|v| v / scale).collect();
        let anchor = values[0];
        let offset = values
            .iter()
            .map(|v| (v - anchor) / values.len() as f64)
            .sum::<f64>();
        let mean = anchor + offset;
        let mut sum_squares = 0.;
        let mut mean_moving_range = 0.;
        for (i, &v) in values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            sum_squares += ((v - anchor) - offset).powi(2);
            if i > 0 {
                mean_moving_range += (v - values[i - 1]).abs() / (values.len() - 1) as f64;
            }
        }
        let standard_deviation = (sum_squares / (values.len() - 1) as f64).sqrt();
        Ok(Self {
            values,
            scale,
            mean,
            standard_deviation,
            mean_moving_range,
        })
    }
    pub fn within_sigma(&self) -> f64 {
        self.mean_moving_range / MR_D2
    }
}
pub(super) fn pooled_sigma(
    values: &[f64],
    groups: &[usize],
    control: &Control,
) -> Result<(f64, usize)> {
    if values.len() != groups.len() {
        return Err(parameter());
    }
    let count = groups
        .iter()
        .copied()
        .max()
        .and_then(|v| v.checked_add(1))
        .ok_or_else(parameter)?;
    if count > values.len() / 2 {
        return Err(parameter());
    }
    let mut means = vec![0.; count];
    let mut sizes = vec![0usize; count];
    for (i, (&v, &g)) in values.iter().zip(groups).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        sizes[g] += 1;
        means[g] += (v - means[g]) / sizes[g] as f64;
    }
    if sizes.iter().any(|&n| n < 2) {
        return Err(parameter());
    }
    let mut ss = 0.;
    for (i, (&v, &g)) in values.iter().zip(groups).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        ss += (v - means[g]).powi(2);
    }
    Ok(((ss / (values.len() - count) as f64).sqrt(), count))
}
