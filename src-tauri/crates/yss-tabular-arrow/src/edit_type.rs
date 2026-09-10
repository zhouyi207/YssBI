use crate::TabularArrowError;
use arrow::datatypes::{DataType, TimeUnit};

/// Parse a user-requested edit target, independently of Graph's coarse semantic types.
pub fn editable_data_type(source: &str) -> Result<DataType, TabularArrowError> {
    let source = source.trim();
    let basic = match source {
        "Boolean" => Some(DataType::Boolean),
        "Int8" => Some(DataType::Int8),
        "Int16" => Some(DataType::Int16),
        "Int32" => Some(DataType::Int32),
        "Int64" => Some(DataType::Int64),
        "UInt8" => Some(DataType::UInt8),
        "UInt16" => Some(DataType::UInt16),
        "UInt32" => Some(DataType::UInt32),
        "UInt64" => Some(DataType::UInt64),
        "Float32" => Some(DataType::Float32),
        "Float64" => Some(DataType::Float64),
        "String" | "Utf8" => Some(DataType::Utf8),
        "Date" | "Date32" => Some(DataType::Date32),
        "Datetime" | "DateTime" => Some(DataType::Timestamp(TimeUnit::Microsecond, None)),
        "Time" => Some(DataType::Time64(TimeUnit::Nanosecond)),
        "Categorical" => Some(DataType::Dictionary(
            Box::new(DataType::Int32),
            Box::new(DataType::Utf8),
        )),
        _ => None,
    };
    if let Some(data_type) = basic {
        return Ok(data_type);
    }
    if let Some(parameters) = source
        .strip_prefix("Decimal(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (precision, scale) = parameters
            .split_once(',')
            .ok_or(TabularArrowError::InvalidValue)?;
        let precision: u8 = precision
            .trim()
            .parse()
            .map_err(|_| TabularArrowError::InvalidValue)?;
        let scale: i8 = scale
            .trim()
            .parse()
            .map_err(|_| TabularArrowError::InvalidValue)?;
        if precision == 0 || precision > 76 || scale < 0 || scale as u8 > precision {
            return Err(TabularArrowError::InvalidValue);
        }
        return Ok(if precision <= 38 {
            DataType::Decimal128(precision, scale)
        } else {
            DataType::Decimal256(precision, scale)
        });
    }
    if let Some(parameters) = source
        .strip_prefix("Datetime(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (unit, timezone) = parameters
            .split_once(',')
            .map_or((parameters, None), |(unit, timezone)| {
                (unit, Some(timezone.trim()))
            });
        let unit = match unit.trim() {
            "s" => TimeUnit::Second,
            "ms" => TimeUnit::Millisecond,
            "us" => TimeUnit::Microsecond,
            "ns" => TimeUnit::Nanosecond,
            _ => return Err(TabularArrowError::InvalidValue),
        };
        if let Some(timezone) = timezone {
            timezone
                .parse::<arrow::array::timezone::Tz>()
                .map_err(|_| TabularArrowError::InvalidValue)?;
        }
        return Ok(DataType::Timestamp(unit, timezone.map(Into::into)));
    }
    Err(TabularArrowError::UnsupportedType)
}
