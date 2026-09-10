//! Lag on a complete time grid; gaps remain null.
use crate::data::PreparationError;
use arrow::array::{Array, ArrayRef, Float64Array};

pub fn ts_lag(
    times: &dyn Array,
    values: &Float64Array,
    lag: usize,
    interval: i64,
) -> Result<(ArrayRef, Float64Array, Float64Array), PreparationError> {
    let (times, aligned) = super::align::align_series(times, values, interval)?;
    let shifted = Float64Array::from_iter((0..aligned.len()).map(|row| {
        let previous = row.checked_sub(lag)?;
        (!aligned.is_null(previous)).then(|| aligned.value(previous))
    }));
    Ok((times, aligned, shifted))
}
