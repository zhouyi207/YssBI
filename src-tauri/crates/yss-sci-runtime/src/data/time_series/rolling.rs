//! Rolling means preserve nulls when any member of a complete window is null.
use crate::data::{PreparationError, check_size};
use arrow::array::{Array, Float64Array};

pub fn rolling_mean(
    values: &Float64Array,
    window: usize,
) -> Result<Float64Array, PreparationError> {
    if window == 0 {
        return Err(PreparationError::Interval);
    }
    check_size(values.len(), 16)?;
    Ok(Float64Array::from_iter((0..values.len()).map(|row| {
        let start = (row + 1).checked_sub(window)?;
        let mut sum = 0.0;
        for index in start..=row {
            if values.is_null(index) {
                return None;
            }
            sum += values.value(index);
        }
        Some(sum / window as f64)
    })))
}
