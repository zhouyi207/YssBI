//! Percentage change; zero denominators and null operands produce null.
use crate::data::{PreparationError, check_size};
use arrow::array::{Array, Float64Array};

pub fn ts_pct_change(values: &Float64Array, lag: usize) -> Result<Float64Array, PreparationError> {
    check_size(values.len(), 16)?;
    Ok(Float64Array::from_iter((0..values.len()).map(|row| {
        let previous = row.checked_sub(lag)?;
        if values.is_null(row) || values.is_null(previous) || values.value(previous) == 0.0 {
            None
        } else {
            Some((values.value(row) - values.value(previous)) / values.value(previous))
        }
    })))
}
