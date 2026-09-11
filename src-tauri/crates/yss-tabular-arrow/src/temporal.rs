//! Project datetimes represent calendar/clock values, independent of the host timezone.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, PrimitiveArray, StringArray, make_array};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{
    ArrowTimestampType, DataType, FieldRef, Schema, TimeUnit, TimestampMicrosecondType,
    TimestampMillisecondType, TimestampNanosecondType, TimestampSecondType,
};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use chrono::{Offset, TimeZone};

use crate::TabularArrowError;

fn timezone_free_field(field: &FieldRef) -> FieldRef {
    Arc::new(
        field
            .as_ref()
            .clone()
            .with_data_type(timezone_free_data_type(field.data_type())),
    )
}

pub fn timezone_free_data_type(dtype: &DataType) -> DataType {
    match dtype {
        DataType::Timestamp(unit, _) => DataType::Timestamp(*unit, None),
        DataType::List(field) => DataType::List(timezone_free_field(field)),
        DataType::LargeList(field) => DataType::LargeList(timezone_free_field(field)),
        DataType::ListView(field) => DataType::ListView(timezone_free_field(field)),
        DataType::LargeListView(field) => DataType::LargeListView(timezone_free_field(field)),
        DataType::FixedSizeList(field, size) => {
            DataType::FixedSizeList(timezone_free_field(field), *size)
        }
        DataType::Struct(fields) => {
            DataType::Struct(fields.iter().map(timezone_free_field).collect())
        }
        DataType::Map(field, sorted) => {
            // A DST fold can reverse the clock ordering of previously sorted timestamp keys.
            let key_changes = matches!(field.data_type(), DataType::Struct(fields)
                if fields.first().is_some_and(|key| timezone_free_data_type(key.data_type()) != *key.data_type()));
            DataType::Map(timezone_free_field(field), *sorted && !key_changes)
        }
        DataType::Dictionary(keys, values) => {
            DataType::Dictionary(keys.clone(), Box::new(timezone_free_data_type(values)))
        }
        DataType::Union(fields, mode) => DataType::Union(
            fields
                .iter()
                .map(|(id, field)| (id, timezone_free_field(field)))
                .collect(),
            *mode,
        ),
        DataType::RunEndEncoded(ends, values) => {
            DataType::RunEndEncoded(timezone_free_field(ends), timezone_free_field(values))
        }
        dtype => dtype.clone(),
    }
}

pub fn timezone_free_schema(schema: &Schema) -> Schema {
    Schema::new_with_metadata(
        schema
            .fields()
            .iter()
            .map(timezone_free_field)
            .collect::<Vec<_>>(),
        schema.metadata().clone(),
    )
}

fn local_timestamp_array<T: ArrowTimestampType>(
    array: &dyn Array,
    timezone: &str,
    force: bool,
) -> Result<ArrayRef, TabularArrowError> {
    let array = array
        .as_any()
        .downcast_ref::<PrimitiveArray<T>>()
        .ok_or(TabularArrowError::InvalidValue)?;
    if timezone.is_empty() {
        return Ok(Arc::new(array.clone().with_timezone_opt(None::<String>)));
    }
    let timezone = timezone
        .parse::<arrow::array::timezone::Tz>()
        .map_err(|_| TabularArrowError::InvalidValue)?;
    let convert = |row| {
        if array.is_null(row) {
            return Ok(None);
        }
        let utc = array
            .value_as_datetime(row)
            .ok_or(TabularArrowError::InvalidValue)?;
        let offset = timezone.offset_from_utc_datetime(&utc).fix();
        let local = utc
            .checked_add_offset(offset)
            .ok_or(TabularArrowError::InvalidValue)?;
        // UTC is only the arithmetic encoding of a naive clock value here; no timezone is stored.
        let encoded = local.and_utc();
        let value = match T::UNIT {
            TimeUnit::Second => encoded.timestamp(),
            TimeUnit::Millisecond => encoded.timestamp_millis(),
            TimeUnit::Microsecond => encoded.timestamp_micros(),
            TimeUnit::Nanosecond => encoded
                .timestamp_nanos_opt()
                .ok_or(TabularArrowError::InvalidValue)?,
        };
        Ok(Some(value))
    };
    let values = (0..array.len())
        .map(|row| match convert(row) {
            Err(_) if force => Ok(None),
            value => value,
        })
        .collect::<Result<Vec<_>, TabularArrowError>>()?;
    Ok(Arc::new(PrimitiveArray::<T>::from_iter(values)))
}

/// Drop the zone while retaining the wall clock, units, nulls, and field metadata.
/// A plain Arrow cast would retain the UTC-based integers and change the visible clock.
pub fn timezone_free_array(array: &dyn Array) -> Result<ArrayRef, TabularArrowError> {
    timezone_free_array_impl(array, false)
}

fn timezone_free_array_impl(array: &dyn Array, force: bool) -> Result<ArrayRef, TabularArrowError> {
    let target = timezone_free_data_type(array.data_type());
    if &target == array.data_type() {
        return Ok(make_array(array.to_data()));
    }
    if let DataType::Timestamp(unit, Some(timezone)) = array.data_type() {
        return match unit {
            TimeUnit::Second => {
                local_timestamp_array::<TimestampSecondType>(array, timezone, force)
            }
            TimeUnit::Millisecond => {
                local_timestamp_array::<TimestampMillisecondType>(array, timezone, force)
            }
            TimeUnit::Microsecond => {
                local_timestamp_array::<TimestampMicrosecondType>(array, timezone, force)
            }
            TimeUnit::Nanosecond => {
                local_timestamp_array::<TimestampNanosecondType>(array, timezone, force)
            }
        };
    }
    let data = array.to_data();
    let children = data
        .child_data()
        .iter()
        .map(|child| {
            timezone_free_array_impl(make_array(child.clone()).as_ref(), force)
                .map(|array| array.to_data())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let data = data
        .into_builder()
        .data_type(target)
        .child_data(children)
        .build()
        .map_err(|_| TabularArrowError::InvalidValue)?;
    Ok(make_array(data))
}

pub fn timezone_free_batch(batch: &RecordBatch) -> Result<RecordBatch, TabularArrowError> {
    let schema = timezone_free_schema(&batch.schema());
    if &schema == batch.schema().as_ref() {
        return Ok(batch.clone());
    }
    let arrays = batch
        .columns()
        .iter()
        .map(|array| timezone_free_array(array.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    RecordBatch::try_new_with_options(
        Arc::new(schema),
        arrays,
        &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )
    .map_err(|_| TabularArrowError::BuildFailed)
}

/// Only temporal callers use this: ordinary strings are never rewritten as dates.
pub fn timezone_free_text(value: &str) -> Result<&str, TabularArrowError> {
    let value = value.trim();
    let start = if value.as_bytes().get(4) == Some(&b'-') && value.as_bytes().get(7) == Some(&b'-')
    {
        match value.as_bytes().get(10) {
            Some(b'T' | b't' | b' ') => 11,
            _ => return Ok(value),
        }
    } else if value.as_bytes().get(2) == Some(&b':') {
        3
    } else {
        return Ok(value);
    };
    if let Some(naive) = value.strip_suffix(['Z', 'z']) {
        return Ok(naive);
    }
    if let Some(index) = value[start..].find(['+', '-']) {
        let index = start + index;
        value[index..]
            .parse::<arrow::array::timezone::Tz>()
            .map_err(|_| TabularArrowError::InvalidValue)?;
        return Ok(value[..index].trim_end());
    }
    if let Some(index) = value[start..].rfind(' ') {
        let index = start + index;
        let suffix = value[index..].trim();
        if suffix.parse::<arrow::array::timezone::Tz>().is_ok() {
            return Ok(value[..index].trim_end());
        }
    }
    Ok(value)
}

pub fn datetime_strings_without_timezone(
    strings: &StringArray,
    dtype: &DataType,
) -> Result<ArrayRef, TabularArrowError> {
    cast_temporal_without_timezone(strings, dtype, false)
}

/// Dataset casts share the same clock semantics as CSV and cell edits; forced failures stay null.
pub fn cast_temporal_without_timezone(
    array: &dyn Array,
    dtype: &DataType,
    force: bool,
) -> Result<ArrayRef, TabularArrowError> {
    let options = CastOptions {
        safe: force,
        ..Default::default()
    };
    let array = if matches!(
        array.data_type(),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
    ) {
        let strings = cast_with_options(array, &DataType::Utf8, &options)
            .map_err(|_| TabularArrowError::InvalidValue)?;
        let strings = strings
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(TabularArrowError::InvalidValue)?;
        let values = strings
            .iter()
            .map(|value| match value.map(timezone_free_text).transpose() {
                Err(_) if force => Ok(None),
                value => value,
            })
            .collect::<Result<Vec<_>, _>>()?;
        Arc::new(StringArray::from(values)) as ArrayRef
    } else {
        timezone_free_array_impl(array, force)?
    };
    cast_with_options(array.as_ref(), &timezone_free_data_type(dtype), &options)
        .map_err(|_| TabularArrowError::InvalidValue)
}
