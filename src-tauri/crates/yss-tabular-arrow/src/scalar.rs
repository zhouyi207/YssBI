use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Decimal32Array, Decimal64Array, Decimal128Array,
    Decimal256Array, Float32Array, Float64Array, Int8Array, Int16Array, Int32Array, Int64Array,
    StringArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array, new_null_array,
};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::i256;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use serde_json::Value;
use yss_tabular_contract::{TabularScalar, TabularSnapshot};

use crate::{CategoryDomain, TabularArrowError};

/// Converts a bounded set of edited/literal values, rejecting overflow and incompatible values.
/// Exact Arrow batches should bypass this display conversion entirely.
pub fn json_to_array(field: &Field, values: &[Value]) -> Result<ArrayRef, TabularArrowError> {
    let dtype = field.data_type();
    let invalid = || TabularArrowError::InvalidValue;
    if !field.is_nullable() && values.iter().any(Value::is_null) {
        return Err(invalid());
    }
    macro_rules! integer {
        ($array:ty, $native:ty, $access:ident) => {{
            let parsed = values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::String(value) => {
                        value.parse::<$native>().map(Some).map_err(|_| invalid())
                    }
                    Value::Number(value) => value
                        .$access()
                        .and_then(|value| <$native>::try_from(value).ok())
                        .map(Some)
                        .ok_or_else(invalid),
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Arc::new(<$array>::from(parsed)) as ArrayRef
        }};
    }
    macro_rules! decimal {
        ($array:ty, $native:ty, $precision:expr, $scale:expr) => {{
            let parsed = values
                .iter()
                .map(|value| {
                    let text = match value {
                        Value::Null => return Ok(None),
                        Value::String(value) => value.clone(),
                        Value::Number(value) if value.is_i64() || value.is_u64() => {
                            value.to_string()
                        }
                        _ => return Err(invalid()),
                    };
                    decimal_coefficient(&text, *$precision, *$scale)?
                        .parse::<$native>()
                        .map(Some)
                        .map_err(|_| invalid())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Arc::new(
                <$array>::from(parsed)
                    .with_precision_and_scale(*$precision, *$scale)
                    .map_err(|_| invalid())?,
            ) as ArrayRef
        }};
    }
    let array: ArrayRef = match dtype {
        DataType::Null if values.iter().all(Value::is_null) => new_null_array(dtype, values.len()),
        DataType::Boolean => Arc::new(BooleanArray::from(
            values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::Bool(value) => Ok(Some(*value)),
                    Value::String(value) => match value.to_ascii_lowercase().as_str() {
                        "true" | "1" => Ok(Some(true)),
                        "false" | "0" => Ok(Some(false)),
                        _ => Err(invalid()),
                    },
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        DataType::Int8 => integer!(Int8Array, i8, as_i64),
        DataType::Int16 => integer!(Int16Array, i16, as_i64),
        DataType::Int32 => integer!(Int32Array, i32, as_i64),
        DataType::Int64 => integer!(Int64Array, i64, as_i64),
        DataType::UInt8 => integer!(UInt8Array, u8, as_u64),
        DataType::UInt16 => integer!(UInt16Array, u16, as_u64),
        DataType::UInt32 => integer!(UInt32Array, u32, as_u64),
        DataType::UInt64 => integer!(UInt64Array, u64, as_u64),
        DataType::Decimal32(precision, scale) => decimal!(Decimal32Array, i32, precision, scale),
        DataType::Decimal64(precision, scale) => decimal!(Decimal64Array, i64, precision, scale),
        DataType::Decimal128(precision, scale) => decimal!(Decimal128Array, i128, precision, scale),
        DataType::Decimal256(precision, scale) => decimal!(Decimal256Array, i256, precision, scale),
        DataType::Float32 | DataType::Float64 => {
            let parsed = values
                .iter()
                .map(|value| {
                    if value.is_null() {
                        return Ok(None);
                    }
                    let number = match value {
                        Value::Number(value) => value.as_f64(),
                        Value::String(value) => value.parse::<f64>().ok(),
                        _ => None,
                    }
                    .filter(|value| value.is_finite())
                    .ok_or_else(invalid)?;
                    if *dtype == DataType::Float32
                        && (number < f64::from(f32::MIN) || number > f64::from(f32::MAX))
                    {
                        return Err(invalid());
                    }
                    Ok(Some(number))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if *dtype == DataType::Float32 {
                Arc::new(Float32Array::from(
                    parsed
                        .into_iter()
                        .map(|v| v.map(|v| v as f32))
                        .collect::<Vec<_>>(),
                ))
            } else {
                Arc::new(Float64Array::from(parsed))
            }
        }
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {
            let strings = values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::String(value) => Ok(Some(value.clone())),
                    Value::Number(_) | Value::Bool(_) => Ok(Some(value.to_string())),
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            strict_cast(&StringArray::from(strings), dtype)?
        }
        DataType::Dictionary(_, _) => {
            let strings = values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::String(value) => Ok(Some(value.as_str())),
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            category_array(field, &StringArray::from(strings))?
        }
        // Display timestamps/decimals are text. Never turn them into f64 on their way to storage.
        DataType::Date32
        | DataType::Date64
        | DataType::Timestamp(_, _)
        | DataType::Time32(_)
        | DataType::Time64(_) => {
            let strings = values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(None),
                    Value::String(value) => Ok(Some(value.as_str())),
                    _ => Err(invalid()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            strict_cast(&StringArray::from(strings), dtype)?
        }
        _ => return Err(TabularArrowError::UnsupportedType),
    };
    Ok(array)
}

fn strict_cast(array: &dyn Array, dtype: &DataType) -> Result<ArrayRef, TabularArrowError> {
    cast_with_options(
        array,
        dtype,
        &CastOptions {
            safe: false,
            ..Default::default()
        },
    )
    .map_err(|_| TabularArrowError::InvalidValue)
}

/// IPC files require one dictionary per column. Re-key a batch by its committed label domain,
/// preserving the exact dictionary key/value types and without passing data through JSON.
pub fn normalize_batch_categories(batch: &RecordBatch) -> Result<RecordBatch, TabularArrowError> {
    let arrays = batch
        .schema()
        .fields()
        .iter()
        .zip(batch.columns())
        .map(|(field, array)| {
            if CategoryDomain::from_field(field)?.is_none() {
                return Ok(array.clone());
            }
            let strings = strict_cast(array.as_ref(), &DataType::Utf8)?;
            category_array(
                field,
                strings
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or(TabularArrowError::InvalidValue)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    RecordBatch::try_new_with_options(
        batch.schema(),
        arrays,
        &arrow::record_batch::RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
    )
    .map_err(|_| TabularArrowError::BuildFailed)
}

fn category_array(field: &Field, strings: &StringArray) -> Result<ArrayRef, TabularArrowError> {
    use arrow::array::DictionaryArray;
    use arrow::datatypes::{
        Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
    };
    let Some(domain) = CategoryDomain::from_field(field)? else {
        return strict_cast(strings, field.data_type());
    };
    let DataType::Dictionary(key_type, value_type) = field.data_type() else {
        return Err(TabularArrowError::InvalidSchema);
    };
    let indices: std::collections::BTreeMap<_, _> = domain
        .labels
        .iter()
        .enumerate()
        .map(|(index, label)| (label.as_str(), index))
        .collect();
    let keys = strings
        .iter()
        .map(|label| {
            label
                .map(|label| {
                    indices
                        .get(label)
                        .copied()
                        .ok_or(TabularArrowError::InvalidValue)
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let labels = strict_cast(&StringArray::from(domain.labels), value_type)?;
    macro_rules! dictionary {
        ($key:ty, $array:ty, $native:ty) => {{
            let keys = keys
                .into_iter()
                .map(|key| {
                    key.map(<$native>::try_from)
                        .transpose()
                        .map_err(|_| TabularArrowError::InvalidValue)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Arc::new(
                DictionaryArray::<$key>::try_new(<$array>::from(keys), labels)
                    .map_err(|_| TabularArrowError::InvalidValue)?,
            ) as ArrayRef
        }};
    }
    Ok(match key_type.as_ref() {
        DataType::Int8 => dictionary!(Int8Type, Int8Array, i8),
        DataType::Int16 => dictionary!(Int16Type, Int16Array, i16),
        DataType::Int32 => dictionary!(Int32Type, Int32Array, i32),
        DataType::Int64 => dictionary!(Int64Type, Int64Array, i64),
        DataType::UInt8 => dictionary!(UInt8Type, UInt8Array, u8),
        DataType::UInt16 => dictionary!(UInt16Type, UInt16Array, u16),
        DataType::UInt32 => dictionary!(UInt32Type, UInt32Array, u32),
        DataType::UInt64 => dictionary!(UInt64Type, UInt64Array, u64),
        _ => return Err(TabularArrowError::InvalidSchema),
    })
}

// Arrow's string-to-decimal cast permits truncating fractional digits. Edits must instead reject
// any nonzero discarded digit, including negative-scale and scientific-notation inputs.
fn decimal_coefficient(text: &str, precision: u8, scale: i8) -> Result<String, TabularArrowError> {
    let invalid = || TabularArrowError::InvalidValue;
    let (negative, unsigned) = if let Some(value) = text.strip_prefix('-') {
        (true, value)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let (mantissa, exponent) = if let Some(at) = unsigned.find(['e', 'E']) {
        (
            &unsigned[..at],
            unsigned[at + 1..].parse::<i32>().map_err(|_| invalid())?,
        )
    } else {
        (unsigned, 0)
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|digit| digit.is_ascii_digit())
    {
        return Err(invalid());
    }
    let digits = format!("{whole}{fraction}");
    let mut digits = digits.trim_start_matches('0').to_owned();
    if digits.is_empty() {
        return Ok("0".into());
    }
    let shift = i32::from(scale)
        .checked_add(exponent)
        .and_then(|value| value.checked_sub(i32::try_from(fraction.len()).ok()?))
        .ok_or_else(invalid)?;
    if shift < 0 {
        let discard = usize::try_from(shift.unsigned_abs()).map_err(|_| invalid())?;
        let retain = digits.len().checked_sub(discard).ok_or_else(invalid)?;
        if !digits.as_bytes()[retain..]
            .iter()
            .all(|digit| *digit == b'0')
        {
            return Err(invalid());
        }
        digits.truncate(retain);
    } else {
        let zeros = usize::try_from(shift).map_err(|_| invalid())?;
        if digits
            .len()
            .checked_add(zeros)
            .is_none_or(|len| len > usize::from(precision))
        {
            return Err(invalid());
        }
        digits.extend(std::iter::repeat_n('0', zeros));
    }
    if digits.len() > usize::from(precision) || digits.is_empty() {
        return Err(invalid());
    }
    if negative {
        digits.insert(0, '-');
    }
    Ok(digits)
}

/// Paging projection: JavaScript-safe integers stay numeric; wider integers and exact decimal,
/// temporal and category values use text. NaN/infinity have the existing UI null representation.
pub fn array_to_json(array: &dyn Array) -> Result<Vec<Value>, TabularArrowError> {
    use arrow::util::display::array_value_to_string;
    if matches!(array.data_type(), DataType::Dictionary(_, _)) {
        return array_to_json(strict_cast(array, &DataType::Utf8)?.as_ref());
    }
    (0..array.len())
        .map(|row| {
            if array.is_null(row) {
                return Ok(Value::Null);
            }
            macro_rules! native {
                ($array:ty) => {
                    serde_json::to_value(
                        array
                            .as_any()
                            .downcast_ref::<$array>()
                            .ok_or(TabularArrowError::InvalidValue)?
                            .value(row),
                    )
                    .map_err(|_| TabularArrowError::InvalidValue)
                };
            }
            match array.data_type() {
                DataType::Boolean => native!(BooleanArray),
                DataType::Int8 => native!(Int8Array),
                DataType::Int16 => native!(Int16Array),
                DataType::Int32 => native!(Int32Array),
                DataType::Int64 => serde_json::to_value(
                    TabularScalar::Integer(
                        array
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .ok_or(TabularArrowError::InvalidValue)?
                            .value(row),
                    )
                    .display_value(),
                )
                .map_err(|_| TabularArrowError::InvalidValue),
                DataType::UInt8 => native!(UInt8Array),
                DataType::UInt16 => native!(UInt16Array),
                DataType::UInt32 => native!(UInt32Array),
                DataType::UInt64 => serde_json::to_value(
                    TabularScalar::Unsigned(
                        array
                            .as_any()
                            .downcast_ref::<UInt64Array>()
                            .ok_or(TabularArrowError::InvalidValue)?
                            .value(row),
                    )
                    .display_value(),
                )
                .map_err(|_| TabularArrowError::InvalidValue),
                DataType::Float32 => native!(Float32Array),
                DataType::Float64 => native!(Float64Array),
                _ => array_value_to_string(array, row)
                    .map(Value::String)
                    .map_err(|_| TabularArrowError::InvalidValue),
            }
        })
        .collect()
}

/// Materialize a document literal, whose contract has no exact storage dtype. Mixed numeric
/// columns may widen only if all their values can be represented without integer precision loss.
pub fn to_record_batch(snapshot: &TabularSnapshot) -> Result<RecordBatch, TabularArrowError> {
    let mut fields = Vec::new();
    let mut arrays = Vec::new();
    for column in snapshot.columns() {
        let mut dtype = DataType::Null;
        for value in column.values() {
            let next = match value {
                TabularScalar::Null => continue,
                TabularScalar::Bool(_) => DataType::Boolean,
                TabularScalar::Integer(_) => DataType::Int64,
                TabularScalar::Unsigned(_) => DataType::UInt64,
                TabularScalar::Decimal(_) => DataType::Float64,
                TabularScalar::String(_) => DataType::Utf8,
            };
            dtype = match (&dtype, &next) {
                (DataType::Null, _) => next,
                (a, b) if a == b => next,
                (DataType::Int64, DataType::UInt64) | (DataType::UInt64, DataType::Int64) => {
                    DataType::UInt64
                }
                (a, b) if a.is_numeric() && b.is_numeric() => DataType::Float64,
                _ => return Err(TabularArrowError::InvalidValue),
            };
        }
        if dtype == DataType::UInt64
            && column
                .values()
                .iter()
                .any(|v| matches!(v, TabularScalar::Integer(v) if *v < 0))
        {
            if column
                .values()
                .iter()
                .any(|v| matches!(v, TabularScalar::Unsigned(v) if *v > i64::MAX as u64))
            {
                return Err(TabularArrowError::InvalidValue);
            }
            dtype = DataType::Int64;
        }
        if dtype == DataType::Float64
            && column.values().iter().any(|value| match value {
                TabularScalar::Integer(value) => (*value as f64) as i128 != i128::from(*value),
                TabularScalar::Unsigned(value) => (*value as f64) as u128 != u128::from(*value),
                _ => false,
            })
        {
            return Err(TabularArrowError::InvalidValue);
        }
        let field = Field::new(column.name().as_str(), dtype, true);
        let values = column
            .values()
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TabularArrowError::InvalidValue)?;
        arrays.push(json_to_array(&field, &values)?);
        fields.push(field);
    }
    RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays)
        .map_err(|_| TabularArrowError::BuildFailed)
}
