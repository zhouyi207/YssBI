//! Positional and time-aware differences over bounded numeric inputs.
use crate::data::{PreparationError, check_size};
use arrow::array::{Array, Float64Array};

pub fn ts_diff(values: &Float64Array, lag: usize) -> Result<Float64Array, PreparationError> {
    check_size(values.len(), 16)?;
    Ok(Float64Array::from_iter((0..values.len()).map(|row| {
        let previous = row.checked_sub(lag)?;
        if values.is_null(row) || values.is_null(previous) {
            None
        } else {
            Some(values.value(row) - values.value(previous))
        }
    })))
}

pub fn ts_diff_with_time(
    times: &dyn Array,
    values: &Float64Array,
    lag: usize,
    interval: i64,
) -> Result<Float64Array, PreparationError> {
    if times.len() != values.len() {
        return Err(PreparationError::Length);
    }
    if interval <= 0 {
        return Err(PreparationError::Interval);
    }
    check_size(values.len(), 32)?;
    let times = super::align::time_numbers(times)?;
    let gap = interval
        .checked_mul(i64::try_from(lag).map_err(|_| PreparationError::Overflow)?)
        .ok_or(PreparationError::Overflow)?;
    Ok(Float64Array::from_iter((0..values.len()).map(|row| {
        let previous = row.checked_sub(lag)?;
        if times[row].checked_sub(times[previous]) != Some(gap)
            || values.is_null(row)
            || values.is_null(previous)
        {
            None
        } else {
            Some(values.value(row) - values.value(previous))
        }
    })))
}
