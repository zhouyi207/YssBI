//! Bounded alignment on a complete numeric or date grid using Arrow take indices.
use super::types::TimeValue;
use crate::data::{MAX_PREPARED_BYTES, PreparationError, check_size};
use arrow::array::{
    Array, ArrayRef, Date32Array, Float64Array, Int64Array, UInt64Array, make_array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(crate) fn time_numbers(values: &dyn Array) -> Result<Vec<i64>, PreparationError> {
    check_size(values.len(), 64)?;
    if values.null_count() != 0 {
        return Err(PreparationError::NullTime);
    }
    match values.data_type() {
        DataType::Int64 => Ok(values
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or(PreparationError::TimeType)?
            .values()
            .to_vec()),
        DataType::Date32 => Ok(values
            .as_any()
            .downcast_ref::<Date32Array>()
            .ok_or(PreparationError::TimeType)?
            .values()
            .iter()
            .map(|value| i64::from(*value))
            .collect()),
        _ => Err(PreparationError::TimeType),
    }
}

pub(crate) fn time_array(
    values: Vec<i64>,
    data_type: &DataType,
) -> Result<ArrayRef, PreparationError> {
    match data_type {
        DataType::Int64 => Ok(Arc::new(Int64Array::from(values))),
        DataType::Date32 => Ok(Arc::new(Date32Array::from(
            values
                .into_iter()
                .map(|value| i32::try_from(value).map_err(|_| PreparationError::Overflow))
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        _ => Err(PreparationError::TimeType),
    }
}

pub fn array_to_time_values(values: &dyn Array) -> Result<Vec<TimeValue>, PreparationError> {
    let numbers = time_numbers(values)?;
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).ok_or(PreparationError::Overflow)?;
    numbers
        .into_iter()
        .map(|value| match values.data_type() {
            DataType::Int64 => Ok(TimeValue::Num(value)),
            DataType::Date32 => epoch
                .checked_add_signed(chrono::Duration::days(value))
                .map(TimeValue::Date)
                .ok_or(PreparationError::Overflow),
            _ => Err(PreparationError::TimeType),
        })
        .collect()
}

pub fn time_values_to_array(times: &[TimeValue]) -> Result<ArrayRef, PreparationError> {
    check_size(times.len(), 16)?;
    let date = matches!(times.first(), Some(TimeValue::Date(_)));
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).ok_or(PreparationError::Overflow)?;
    let values = times
        .iter()
        .map(|time| match time {
            TimeValue::Num(value) if !date => Ok(*value),
            TimeValue::Date(value) if date => Ok(value.signed_duration_since(epoch).num_days()),
            _ => Err(PreparationError::TimeType),
        })
        .collect::<Result<Vec<_>, _>>()?;
    time_array(
        values,
        if date {
            &DataType::Date32
        } else {
            &DataType::Int64
        },
    )
}

pub fn check_no_duplicate_times(times: &dyn Array) -> Result<(), PreparationError> {
    let values = time_numbers(times)?;
    if values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(PreparationError::DuplicateTime);
    }
    Ok(())
}

pub fn infer_interval(times: &dyn Array) -> Result<i64, PreparationError> {
    let mut values = time_numbers(times)?;
    values.sort_unstable();
    values.dedup();
    values
        .windows(2)
        .map(|pair| {
            pair[1]
                .checked_sub(pair[0])
                .ok_or(PreparationError::Overflow)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|gaps| gaps.into_iter().min().unwrap_or(1))
}

pub fn align_series(
    times: &dyn Array,
    values: &Float64Array,
    interval: i64,
) -> Result<(ArrayRef, Float64Array), PreparationError> {
    if times.len() != values.len() {
        return Err(PreparationError::Length);
    }
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("time", times.data_type().clone(), true),
            Field::new("value", DataType::Float64, true),
        ])),
        vec![make_array(times.to_data()), Arc::new(values.clone())],
    )?;
    let aligned = align_batch(&batch, "time", interval)?;
    let values = aligned
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or(PreparationError::ValueType)?
        .clone();
    Ok((aligned.column(0).clone(), values))
}

pub fn align_batch(
    batch: &RecordBatch,
    time_column: &str,
    interval: i64,
) -> Result<RecordBatch, PreparationError> {
    if interval <= 0 {
        return Err(PreparationError::Interval);
    }
    let time_index = batch
        .schema()
        .index_of(time_column)
        .map_err(|_| PreparationError::Column)?;
    let times = time_numbers(batch.column(time_index).as_ref())?;
    let indexed = times
        .iter()
        .enumerate()
        .map(|(index, time)| (*time, index as u64))
        .collect::<BTreeMap<_, _>>();
    if times.len() != indexed.len() {
        return Err(PreparationError::DuplicateTime);
    }
    let lo = *indexed.first_key_value().ok_or(PreparationError::Empty)?.0;
    let hi = *indexed.last_key_value().ok_or(PreparationError::Empty)?.0;
    let rows = usize::try_from((i128::from(hi) - i128::from(lo)) / i128::from(interval) + 1)
        .map_err(|_| PreparationError::MemoryLimit)?;
    let output_bytes = rows
        .checked_mul(
            batch
                .num_columns()
                .checked_mul(8)
                .and_then(|value| value.checked_add(32))
                .ok_or(PreparationError::MemoryLimit)?,
        )
        .and_then(|bytes| bytes.checked_add(batch.get_array_memory_size()))
        .ok_or(PreparationError::MemoryLimit)?;
    if output_bytes > MAX_PREPARED_BYTES {
        return Err(PreparationError::MemoryLimit);
    }
    let grid = (0..rows)
        .map(|index| {
            i64::try_from(i128::from(lo) + index as i128 * i128::from(interval))
                .map_err(|_| PreparationError::Overflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let indices = UInt64Array::from_iter(grid.iter().map(|time| indexed.get(time).copied()));
    let time = time_array(grid, batch.column(time_index).data_type())?;
    let arrays = batch
        .columns()
        .iter()
        .enumerate()
        .map(|(index, values)| {
            if index == time_index {
                Ok(time.clone())
            } else {
                arrow::compute::take(values.as_ref(), &indices, None)
                    .map_err(PreparationError::from)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fields = batch
        .schema()
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| field.as_ref().clone().with_nullable(index != time_index))
        .collect::<Vec<_>>();
    Ok(RecordBatch::try_new(
        Arc::new(Schema::new_with_metadata(
            fields,
            batch.schema().metadata().clone(),
        )),
        arrays,
    )?)
}
