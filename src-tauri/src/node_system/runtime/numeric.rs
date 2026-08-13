use super::{KernelError, NumericSeriesView, checked_int64_to_f64};
use crate::project::{NumericTolerance, StatisticalMissingValuePolicy};
use num_traits::Float;
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericValue {
    Int64(i64),
    Float64(f64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericError(Box<str>);

impl NumericError {
    fn new(message: impl Into<Box<str>>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for NumericError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NumericError {}

pub fn approximately_equal(left: f64, right: f64, tolerance: NumericTolerance) -> bool {
    approximately_equal_float(left, right, tolerance.absolute, tolerance.relative)
}

fn approximately_equal_float<T: Float>(left: T, right: T, absolute: T, relative: T) -> bool {
    if left.is_nan() || right.is_nan() {
        return false;
    }
    if left == right {
        return true;
    }
    if left.is_infinite() || right.is_infinite() {
        return false;
    }
    let difference = (left - right).abs();
    difference <= absolute.max(relative * left.abs().max(right.abs()))
}

pub fn approximately_zero(value: f64, tolerance: NumericTolerance) -> bool {
    value.is_finite() && value.abs() <= tolerance.absolute
}

pub fn numeric_equal(
    left: NumericValue,
    right: NumericValue,
    tolerance: NumericTolerance,
) -> Result<bool, NumericError> {
    match (left, right) {
        (NumericValue::Int64(left), NumericValue::Int64(right)) => Ok(left == right),
        (NumericValue::Float64(left), NumericValue::Float64(right)) => {
            Ok(approximately_equal(left, right, tolerance))
        }
        (NumericValue::Int64(left), NumericValue::Float64(right)) => Ok(approximately_equal(
            checked_int64_to_f64(left).map_err(|error| NumericError::new(error.to_string()))?,
            right,
            tolerance,
        )),
        (NumericValue::Float64(left), NumericValue::Int64(right)) => Ok(approximately_equal(
            left,
            checked_int64_to_f64(right).map_err(|error| NumericError::new(error.to_string()))?,
            tolerance,
        )),
    }
}

pub fn numeric_ordering(left: NumericValue, right: NumericValue) -> Result<Ordering, NumericError> {
    match (left, right) {
        (NumericValue::Int64(left), NumericValue::Int64(right)) => Ok(left.cmp(&right)),
        (NumericValue::Float64(left), NumericValue::Float64(right)) => ordered_f64(left, right),
        (NumericValue::Int64(left), NumericValue::Float64(right)) => ordered_f64(
            checked_int64_to_f64(left).map_err(|error| NumericError::new(error.to_string()))?,
            right,
        ),
        (NumericValue::Float64(left), NumericValue::Int64(right)) => ordered_f64(
            left,
            checked_int64_to_f64(right).map_err(|error| NumericError::new(error.to_string()))?,
        ),
    }
}

fn ordered_f64(left: f64, right: f64) -> Result<Ordering, NumericError> {
    left.partial_cmp(&right)
        .ok_or_else(|| NumericError::new("NaN has no numeric ordering"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListwiseRows {
    columns: Box<[Box<[f64]>]>,
    original_row_count: usize,
    dropped_null_count: usize,
    dropped_nan_count: usize,
}

impl ListwiseRows {
    pub fn columns(&self) -> &[Box<[f64]>] {
        &self.columns
    }

    pub fn original_row_count(&self) -> usize {
        self.original_row_count
    }

    pub fn used_row_count(&self) -> usize {
        self.columns
            .first()
            .map_or(self.original_row_count, |column| column.len())
    }

    pub fn dropped_null_count(&self) -> usize {
        self.dropped_null_count
    }

    pub fn dropped_nan_count(&self) -> usize {
        self.dropped_nan_count
    }
}

pub fn listwise_numeric_rows(inputs: &[NumericSeriesView]) -> Result<ListwiseRows, KernelError> {
    prepare_numeric_rows(inputs, StatisticalMissingValuePolicy::Listwise)
}

pub fn prepare_numeric_rows(
    inputs: &[NumericSeriesView],
    policy: StatisticalMissingValuePolicy,
) -> Result<ListwiseRows, KernelError> {
    let row_count = inputs.first().map_or(0, numeric_series_length);
    for (input_index, input) in inputs.iter().enumerate().skip(1) {
        let actual = numeric_series_length(input);
        if actual != row_count {
            return Err(KernelError::new(format!(
                "numeric input {input_index} has {actual} rows; expected {row_count}"
            )));
        }
    }

    let mut keep = vec![true; row_count];
    let mut null_rows = vec![false; row_count];
    let mut nan_rows = vec![false; row_count];
    for (input_index, input) in inputs.iter().enumerate() {
        inspect_input(
            input,
            input_index,
            policy,
            &mut keep,
            &mut null_rows,
            &mut nan_rows,
        )?;
    }

    let columns = inputs
        .iter()
        .map(|input| collect_column(input, &keep))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let dropped_null_count = null_rows.iter().filter(|missing| **missing).count();
    let dropped_nan_count = nan_rows
        .iter()
        .zip(&null_rows)
        .filter(|(nan, null)| **nan && !**null)
        .count();
    Ok(ListwiseRows {
        columns,
        original_row_count: row_count,
        dropped_null_count,
        dropped_nan_count,
    })
}

fn numeric_series_length(input: &NumericSeriesView) -> usize {
    match input {
        NumericSeriesView::Int64(view) => view.values().len(),
        NumericSeriesView::Float64(view) => view.values().len(),
    }
}

fn inspect_input(
    input: &NumericSeriesView,
    input_index: usize,
    policy: StatisticalMissingValuePolicy,
    keep: &mut [bool],
    null_rows: &mut [bool],
    nan_rows: &mut [bool],
) -> Result<(), KernelError> {
    match input {
        NumericSeriesView::Int64(view) => {
            for (row, value) in view.values().iter().enumerate() {
                if value.is_none() {
                    record_missing(input_index, row, "Null", policy, keep, null_rows)?;
                }
            }
        }
        NumericSeriesView::Float64(view) => {
            for (row, value) in view.values().iter().enumerate() {
                match value {
                    None => record_missing(input_index, row, "Null", policy, keep, null_rows)?,
                    Some(value) if value.is_nan() => {
                        record_missing(input_index, row, "NaN", policy, keep, nan_rows)?;
                    }
                    Some(value) if value.is_infinite() => {
                        let kind = if value.is_sign_positive() {
                            "positive infinity"
                        } else {
                            "negative infinity"
                        };
                        return Err(KernelError::new(format!(
                            "numeric input {input_index} contains {kind} at row {row}"
                        )));
                    }
                    Some(_) => {}
                }
            }
        }
    }
    Ok(())
}

fn record_missing(
    input_index: usize,
    row: usize,
    kind: &'static str,
    policy: StatisticalMissingValuePolicy,
    keep: &mut [bool],
    rows: &mut [bool],
) -> Result<(), KernelError> {
    if policy == StatisticalMissingValuePolicy::Reject {
        return Err(KernelError::new(format!(
            "numeric input {input_index} contains {kind} at row {row}"
        )));
    }
    keep[row] = false;
    rows[row] = true;
    Ok(())
}

fn collect_column(input: &NumericSeriesView, keep: &[bool]) -> Result<Box<[f64]>, KernelError> {
    match input {
        NumericSeriesView::Int64(view) => view
            .values()
            .iter()
            .zip(keep)
            .filter(|(_, keep)| **keep)
            .map(|(value, _)| checked_int64_to_f64(value.expect("kept Int64 rows are present")))
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice),
        NumericSeriesView::Float64(view) => Ok(view
            .values()
            .iter()
            .zip(keep)
            .filter(|(_, keep)| **keep)
            .map(|(value, _)| value.expect("kept Float64 rows are finite"))
            .collect::<Vec<_>>()
            .into_boxed_slice()),
    }
}
