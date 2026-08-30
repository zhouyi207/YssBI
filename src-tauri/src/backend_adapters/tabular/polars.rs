use chrono::{Datelike, NaiveDate, NaiveDateTime, Utc};
use polars::prelude::{AnyValue, Column, DataFrame, DataType as PDataType, PlSmallStr, Series};
use serde_json::Value;

use yss_tabular_contract::{TabularColumnName, TabularScalar, TabularSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TabularMaterializationError {
    #[error("tabular column type is unsupported")]
    UnsupportedColumnType { column: TabularColumnName },
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
        .map(values_to_series)
        .collect::<Result<Vec<_>, _>>()?;
    let columns = series.into_iter().map(Column::from).collect();
    DataFrame::new(snapshot.row_count(), columns)
        .map_err(|_| TabularMaterializationError::BuildFailed)
}

fn values_to_series(
    column: &yss_tabular_contract::TabularColumn,
) -> Result<Series, TabularMaterializationError> {
    let dtype = infer_polars_dtype(column.values());
    let values = column
        .values()
        .iter()
        .map(|value| scalar_to_json(value).and_then(|value| json_to_anyvalue(&value, &dtype)))
        .collect::<Result<Vec<_>, _>>()?;
    Series::from_any_values(PlSmallStr::from(column.name().as_str()), &values, false)
        .map_err(|_| TabularMaterializationError::BuildFailed)
}

fn infer_polars_dtype(values: &[TabularScalar]) -> PDataType {
    let mut saw_int = false;
    let mut saw_float = false;
    let mut saw_bool = false;
    let mut saw_string = false;
    let mut non_null = 0usize;

    for value in values {
        match value {
            TabularScalar::Null => continue,
            TabularScalar::Bool(_) => saw_bool = true,
            TabularScalar::Integer(_) => saw_int = true,
            TabularScalar::Unsigned(_) => saw_int = true,
            TabularScalar::Decimal(_) => saw_float = true,
            TabularScalar::String(_) => saw_string = true,
        }
        non_null += 1;
    }

    if non_null == 0 {
        return PDataType::String;
    }
    if saw_string || (saw_bool as u8 + saw_int as u8 + saw_float as u8) > 1 {
        return PDataType::String;
    }
    if saw_bool {
        PDataType::Boolean
    } else if saw_float {
        PDataType::Float64
    } else if saw_int {
        PDataType::Int64
    } else {
        PDataType::String
    }
}

fn scalar_to_json(value: &TabularScalar) -> Result<Value, TabularMaterializationError> {
    match value {
        TabularScalar::Null => Ok(Value::Null),
        TabularScalar::Bool(value) => Ok(Value::Bool(*value)),
        TabularScalar::Integer(value) => Ok(serde_json::json!(value)),
        TabularScalar::Unsigned(value) => i64::try_from(*value)
            .map(|value| serde_json::json!(value))
            .map_err(|_| TabularMaterializationError::BuildFailed),
        TabularScalar::Decimal(value) if value.as_f64().is_finite() => {
            serde_json::Number::from_f64(value.as_f64())
                .map(Value::Number)
                .ok_or(TabularMaterializationError::BuildFailed)
        }
        TabularScalar::Decimal(_) => Err(TabularMaterializationError::BuildFailed),
        TabularScalar::String(value) => Ok(Value::String(value.to_string())),
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
                    Ok(AnyValue::Date((date.num_days_from_ce() - epoch) as i32))
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
