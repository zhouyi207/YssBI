//! Tabular input preparation, outside the numerical algorithm crate.
pub mod panel;
pub mod time_series;

pub const MAX_PREPARED_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PreparationError {
    #[error("unsupported time column type")]
    TimeType,
    #[error("unsupported value column type")]
    ValueType,
    #[error("time column contains null")]
    NullTime,
    #[error("time column contains duplicate values")]
    DuplicateTime,
    #[error("input is empty")]
    Empty,
    #[error("columns have different lengths")]
    Length,
    #[error("interval or window is invalid")]
    Interval,
    #[error("time arithmetic overflow")]
    Overflow,
    #[error("tabular preparation exceeds its memory limit")]
    MemoryLimit,
    #[error("input column is unavailable")]
    Column,
    #[error("panel input preparation failed")]
    Panel,
    #[error("Arrow input preparation failed")]
    Arrow(#[from] arrow::error::ArrowError),
}

pub(crate) fn check_size(rows: usize, bytes_per_row: usize) -> Result<(), PreparationError> {
    if rows
        .checked_mul(bytes_per_row)
        .is_none_or(|bytes| bytes > MAX_PREPARED_BYTES)
    {
        return Err(PreparationError::MemoryLimit);
    }
    Ok(())
}
