//! Tabular input preparation, outside the numerical algorithm crate.
pub mod panel;
pub mod time_series;

/// Materialize an admitted stream once, then align without dropping off-grid observations.
pub fn align_batches(
    batches: &[arrow::record_batch::RecordBatch],
    time: &str,
    entity: Option<&str>,
    interval: i64,
    max_bytes: usize,
) -> Result<arrow::record_batch::RecordBatch, PreparationError> {
    let schema = batches.first().ok_or(PreparationError::Empty)?.schema();
    let retained = batches.iter().try_fold(0usize, |bytes, batch| {
        bytes
            .checked_add(batch.get_array_memory_size())
            .ok_or(PreparationError::MemoryLimit)
    })?;
    if retained
        .checked_mul(3)
        .is_none_or(|bytes| bytes > max_bytes.min(MAX_PREPARED_BYTES))
    {
        return Err(PreparationError::MemoryLimit);
    }
    let batch = arrow::compute::concat_batches(&schema, batches)?;
    match entity {
        Some(entity) => {
            panel::align_batch_with_limit(&batch, entity, time, Some(interval), max_bytes)
        }
        None => time_series::align::align_batch_with_limit(&batch, time, interval, max_bytes),
    }
}

pub(crate) fn check_alignment_size(
    batch: &arrow::record_batch::RecordBatch,
    rows: usize,
    max_bytes: usize,
) -> Result<(), PreparationError> {
    // Includes the retained input, concatenation, index maps and variable-width fields.
    let width = batch.columns().iter().try_fold(64usize, |width, column| {
        let bytes = match column.data_type() {
            arrow::datatypes::DataType::Utf8 => column
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .map(|v| {
                    v.iter()
                        .flatten()
                        .map(str::len)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(16)
                })
                .unwrap_or(32),
            arrow::datatypes::DataType::LargeUtf8 => column
                .as_any()
                .downcast_ref::<arrow::array::LargeStringArray>()
                .map(|v| {
                    v.iter()
                        .flatten()
                        .map(str::len)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(16)
                })
                .unwrap_or(32),
            data_type
                if data_type.is_primitive()
                    || matches!(
                        data_type,
                        arrow::datatypes::DataType::Boolean | arrow::datatypes::DataType::Null
                    ) =>
            {
                32
            }
            _ => column.get_array_memory_size().saturating_add(32),
        };
        width
            .checked_add(bytes)
            .ok_or(PreparationError::MemoryLimit)
    })?;
    if rows
        .checked_mul(width)
        .and_then(|n| n.checked_add(batch.get_array_memory_size().checked_mul(3)?))
        .is_none_or(|n| n > max_bytes.min(MAX_PREPARED_BYTES))
    {
        return Err(PreparationError::MemoryLimit);
    }
    Ok(())
}

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
