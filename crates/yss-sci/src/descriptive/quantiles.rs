/// Type-7 sample quantile. Callers supply nonempty sorted finite values and p in [0,1].
pub(crate) fn quantile_sorted(values: &[f64], probability: f64) -> f64 {
    let position = probability * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    if fraction == 0.5 {
        return values[lower].midpoint(values[upper]);
    }
    values[lower] * (1.0 - fraction) + values[upper] * fraction
}
