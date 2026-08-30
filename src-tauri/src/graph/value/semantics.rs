use yss_data_contract::{DataType, DataValue};

/// Graph-owned queries over persisted values. This module contains no
/// persistence or runtime execution policy.
pub fn can_accept(target: &DataType, source: &DataType) -> bool {
    match (target, source) {
        (DataType::Any, _) | (_, DataType::Any) => true,
        (DataType::OneOf(targets), source) => targets.iter().any(|item| can_accept(item, source)),
        (target, DataType::OneOf(sources)) => sources.iter().any(|item| can_accept(target, item)),
        (DataType::Array(target), DataType::Array(source)) => can_accept(target, source),
        (DataType::DataSeries(target), DataType::DataSeries(source)) => can_accept(target, source),
        (DataType::Struct(target), DataType::Struct(source)) => target == source,
        (target, source) => target == source || can_convert(source, target),
    }
}

pub fn can_convert(source: &DataType, target: &DataType) -> bool {
    match (source, target) {
        (_, DataType::Any | DataType::String) => true,
        (DataType::OneOf(sources), target) => sources.iter().any(|item| can_convert(item, target)),
        (source, DataType::OneOf(targets)) => targets.iter().any(|item| can_convert(source, item)),
        (DataType::Array(source), DataType::Array(target)) => can_convert(source, target),
        (DataType::DataSeries(source), DataType::DataSeries(target)) => can_convert(source, target),
        (DataType::Boolean, DataType::Int64 | DataType::Float64)
        | (DataType::Int64, DataType::Float64)
        | (DataType::Int64, DataType::Boolean)
        | (DataType::Float64, DataType::Int64 | DataType::Boolean) => true,
        (source, target) => source == target,
    }
}

pub fn value_type(value: &DataValue) -> Option<DataType> {
    match value {
        DataValue::Boolean(_) => Some(DataType::Boolean),
        DataValue::Int64(_) => Some(DataType::Int64),
        DataValue::Float64(_) => Some(DataType::Float64),
        DataValue::String(_) => Some(DataType::String),
        DataValue::Array(values) => Some(DataType::Array(Box::new(
            values.iter().find_map(value_type).unwrap_or(DataType::Any),
        ))),
        DataValue::Object(_) => Some(DataType::Object),
        DataValue::DataFrame(_) => Some(DataType::DataFrame),
        DataValue::DataSeries(series) => Some(DataType::DataSeries(Box::new(
            series.element_type.clone().unwrap_or(DataType::Any),
        ))),
        DataValue::Struct { type_key, .. } => Some(DataType::Struct(type_key.clone())),
        DataValue::Null => None,
    }
}
