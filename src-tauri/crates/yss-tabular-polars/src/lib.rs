//! Polars materialization and value conversion for canonical tabular data.

mod edit;

pub use edit::{
    apply_operation, capture_column_data, capture_row_data, cast_column, dtype_from_string,
    dtype_to_string, reverse_operation,
};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use polars::prelude::{AnyValue, Column, DataFrame, DataType as PDataType, PlSmallStr, Series};
use serde_json::Value;

use yss_tabular_contract::{TabularColumn, TabularScalar, TabularSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TabularMaterializationError {
    #[error("tabular materialization failed")]
    BuildFailed,
}

pub fn to_dataframe(snapshot: &TabularSnapshot) -> Result<DataFrame, TabularMaterializationError> {
    if snapshot.columns().is_empty() {
        return DataFrame::new(0, Vec::<Column>::new())
            .map_err(|_| TabularMaterializationError::BuildFailed);
    }

    let series = snapshot
        .columns()
        .iter()
        .map(column_to_series)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TabularMaterializationError::BuildFailed)?;
    let columns = series.into_iter().map(Column::from).collect();
    DataFrame::new(snapshot.row_count(), columns)
        .map_err(|_| TabularMaterializationError::BuildFailed)
}

pub fn column_to_series(column: &TabularColumn) -> polars::prelude::PolarsResult<Series> {
    let values = column
        .values()
        .iter()
        .map(tabular_scalar_to_any_value)
        .collect::<Vec<_>>();
    Series::from_any_values(PlSmallStr::from(column.name().as_str()), &values, false)
}

pub fn tabular_scalar_to_any_value(value: &TabularScalar) -> AnyValue<'static> {
    match value {
        TabularScalar::Null => AnyValue::Null,
        TabularScalar::Bool(value) => AnyValue::Boolean(*value),
        TabularScalar::Integer(value) => AnyValue::Int64(*value),
        TabularScalar::Unsigned(value) => AnyValue::UInt64(*value),
        TabularScalar::Decimal(value) => AnyValue::Float64(value.as_f64()),
        TabularScalar::String(value) => AnyValue::StringOwned(value.to_string().into()),
    }
}

pub fn anyvalue_to_json(value: AnyValue<'_>) -> Value {
    use polars::prelude::TimeUnit;

    match value {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(value) => Value::Bool(value),
        AnyValue::String(value) => Value::String(value.to_owned()),
        AnyValue::StringOwned(value) => Value::String(value.to_string()),
        AnyValue::Int8(value) => serde_json::json!(value),
        AnyValue::Int16(value) => serde_json::json!(value),
        AnyValue::Int32(value) => serde_json::json!(value),
        AnyValue::Int64(value) => serde_json::json!(value),
        AnyValue::UInt8(value) => serde_json::json!(value),
        AnyValue::UInt16(value) => serde_json::json!(value),
        AnyValue::UInt32(value) => serde_json::json!(value),
        AnyValue::UInt64(value) => serde_json::json!(value),
        AnyValue::Float32(value) => serde_json::Number::from_f64(f64::from(value))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        AnyValue::Float64(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        AnyValue::Date(days) => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
                .expect("the Unix epoch is a valid date")
                .num_days_from_ce();
            NaiveDate::from_num_days_from_ce_opt(epoch + days)
                .map(|date| Value::String(date.format("%Y-%m-%d").to_string()))
                .unwrap_or_else(|| Value::String(days.to_string()))
        }
        AnyValue::Datetime(timestamp, unit, _) | AnyValue::DatetimeOwned(timestamp, unit, _) => {
            let units_per_second = match unit {
                TimeUnit::Nanoseconds => 1_000_000_000,
                TimeUnit::Microseconds => 1_000_000,
                TimeUnit::Milliseconds => 1_000,
            };
            let seconds = timestamp.div_euclid(units_per_second);
            let nanoseconds =
                timestamp.rem_euclid(units_per_second) * (1_000_000_000 / units_per_second);
            chrono::DateTime::from_timestamp(seconds, nanoseconds as u32)
                .map(|datetime| {
                    let formatted = datetime.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                    Value::String(
                        formatted
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_owned(),
                    )
                })
                .unwrap_or_else(|| Value::String(timestamp.to_string()))
        }
        AnyValue::Time(nanoseconds) => {
            let seconds = (nanoseconds / 1_000_000_000) as u32;
            Value::String(format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                (seconds % 3600) / 60,
                seconds % 60
            ))
        }
        _ => Value::String(value.to_string()),
    }
}

/// Converts one JSON value to a Polars value while preserving the target dtype.
pub fn json_to_anyvalue(
    value: &Value,
    dtype: &PDataType,
) -> Result<AnyValue<'static>, TabularMaterializationError> {
    match value {
        Value::Null => Ok(AnyValue::Null),
        Value::Bool(value) => match dtype {
            PDataType::Boolean => Ok(AnyValue::Boolean(*value)),
            PDataType::String => Ok(AnyValue::StringOwned(value.to_string().into())),
            _ => Err(TabularMaterializationError::BuildFailed),
        },
        Value::Number(number) => match dtype {
            PDataType::Int8 => number
                .as_i64()
                .and_then(|value| i8::try_from(value).ok())
                .map(AnyValue::Int8)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::Int16 => number
                .as_i64()
                .and_then(|value| i16::try_from(value).ok())
                .map(AnyValue::Int16)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::Int32 => number
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .map(AnyValue::Int32)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::Int64 => number
                .as_i64()
                .map(AnyValue::Int64)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::UInt8 => number
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .map(AnyValue::UInt8)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::UInt16 => number
                .as_u64()
                .and_then(|value| u16::try_from(value).ok())
                .map(AnyValue::UInt16)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::UInt32 => number
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .map(AnyValue::UInt32)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::UInt64 => number
                .as_u64()
                .map(AnyValue::UInt64)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::Float32 => number
                .as_f64()
                .filter(|value| {
                    value.is_finite() && *value >= f32::MIN as f64 && *value <= f32::MAX as f64
                })
                .map(|value| AnyValue::Float32(value as f32))
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::Float64 => number
                .as_f64()
                .filter(|value| value.is_finite())
                .map(AnyValue::Float64)
                .ok_or(TabularMaterializationError::BuildFailed),
            PDataType::String => Ok(AnyValue::StringOwned(number.to_string().into())),
            _ => Err(TabularMaterializationError::BuildFailed),
        },
        Value::String(value) => {
            if value.is_empty() {
                return Ok(AnyValue::Null);
            }
            match dtype {
                PDataType::Float32 => value
                    .parse::<f32>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .map(AnyValue::Float32)
                    .ok_or(TabularMaterializationError::BuildFailed),
                PDataType::Float64 => value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .map(AnyValue::Float64)
                    .ok_or(TabularMaterializationError::BuildFailed),
                PDataType::Int8 => value
                    .parse::<i8>()
                    .map(AnyValue::Int8)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::Int16 => value
                    .parse::<i16>()
                    .map(AnyValue::Int16)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::Int32 => value
                    .parse::<i32>()
                    .map(AnyValue::Int32)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::Int64 => value
                    .parse::<i64>()
                    .map(AnyValue::Int64)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::UInt8 => value
                    .parse::<u8>()
                    .map(AnyValue::UInt8)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::UInt16 => value
                    .parse::<u16>()
                    .map(AnyValue::UInt16)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::UInt32 => value
                    .parse::<u32>()
                    .map(AnyValue::UInt32)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::UInt64 => value
                    .parse::<u64>()
                    .map(AnyValue::UInt64)
                    .map_err(|_| TabularMaterializationError::BuildFailed),
                PDataType::Boolean => match value.to_ascii_lowercase().as_str() {
                    "true" | "1" => Ok(AnyValue::Boolean(true)),
                    "false" | "0" => Ok(AnyValue::Boolean(false)),
                    _ => Err(TabularMaterializationError::BuildFailed),
                },
                PDataType::String | PDataType::Categorical(_, _) => {
                    Ok(AnyValue::StringOwned(value.clone().into()))
                }
                PDataType::Date => {
                    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .map_err(|_| TabularMaterializationError::BuildFailed)?;
                    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
                        .ok_or(TabularMaterializationError::BuildFailed)?
                        .num_days_from_ce();
                    Ok(AnyValue::Date(date.num_days_from_ce() - epoch))
                }
                PDataType::Datetime(_, _) => {
                    let datetime = if let Ok(datetime) =
                        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
                    {
                        datetime
                    } else {
                        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
                            .map_err(|_| TabularMaterializationError::BuildFailed)?;
                        date.and_hms_opt(0, 0, 0)
                            .ok_or(TabularMaterializationError::BuildFailed)?
                    };
                    let timestamp =
                        chrono::DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc)
                            .timestamp_micros();
                    Ok(AnyValue::DatetimeOwned(
                        timestamp,
                        polars::prelude::TimeUnit::Microseconds,
                        None,
                    ))
                }
                _ => Err(TabularMaterializationError::BuildFailed),
            }
        }
        _ => Err(TabularMaterializationError::BuildFailed),
    }
}

#[cfg(test)]
mod tests;
