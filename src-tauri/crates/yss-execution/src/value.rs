use std::collections::BTreeMap;

use thiserror::Error;

use yss_data_contract::{DataType, DataValue};

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(f64),
    String(Box<str>),
    List(Box<[RuntimeValue]>),
    Record(BTreeMap<Box<str>, RuntimeValue>),
    Resource(Box<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum RuntimeValueError {
    #[error("runtime value is not representable")]
    Unrepresentable,
    #[error("runtime numeric value is not finite")]
    NonFinite,
    #[error("runtime value cannot be coerced to the requested type")]
    InvalidCoercion,
}

impl TryFrom<&DataValue> for RuntimeValue {
    type Error = RuntimeValueError;

    fn try_from(value: &DataValue) -> Result<Self, Self::Error> {
        match value {
            DataValue::Null => Ok(Self::Null),
            DataValue::Boolean(value) => Ok(Self::Bool(*value)),
            DataValue::Int64(value) => Ok(Self::Integer(*value)),
            DataValue::Float64(value) if value.is_finite() => Ok(Self::Decimal(*value)),
            DataValue::Float64(_) => Err(RuntimeValueError::NonFinite),
            DataValue::String(value) => Ok(Self::String(value.clone().into_boxed_str())),
            DataValue::Array(values) => values
                .iter()
                .map(Self::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map(|values| Self::List(values.into_boxed_slice())),
            DataValue::Object(values) => values
                .iter()
                .map(|(key, value)| Ok((key.clone().into_boxed_str(), Self::try_from(value)?)))
                .collect::<Result<BTreeMap<_, _>, RuntimeValueError>>()
                .map(Self::Record),
            DataValue::DataFrame(id) => Ok(Self::Resource(id.clone().into_boxed_str())),
            DataValue::DataSeries(series) => Ok(Self::Resource(series.id.clone().into_boxed_str())),
            DataValue::Struct { handle_id, .. } => {
                Ok(Self::Resource(handle_id.clone().into_boxed_str()))
            }
        }
    }
}

impl RuntimeValue {
    pub fn coerce_to(self, target: &DataType) -> Result<Self, RuntimeValueError> {
        match target {
            DataType::Any => Ok(self),
            DataType::Boolean => match self {
                Self::Bool(_) => Ok(self),
                Self::Integer(value) => Ok(Self::Bool(value != 0)),
                Self::Decimal(value) => Ok(Self::Bool(value != 0.0)),
                Self::String(value) => Ok(Self::Bool(!value.is_empty())),
                Self::Null => Ok(Self::Bool(false)),
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            DataType::Int64 => match self {
                Self::Integer(_) => Ok(self),
                Self::Unsigned(value) => i64::try_from(value)
                    .map(Self::Integer)
                    .map_err(|_| RuntimeValueError::InvalidCoercion),
                Self::Decimal(value) if value.is_finite() => Ok(Self::Integer(value as i64)),
                Self::Bool(value) => Ok(Self::Integer(i64::from(value))),
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            DataType::Float64 => match self {
                Self::Decimal(_) => Ok(self),
                Self::Integer(value) => Ok(Self::Decimal(value as f64)),
                Self::Unsigned(value) => Ok(Self::Decimal(value as f64)),
                Self::Bool(value) => Ok(Self::Decimal(if value { 1.0 } else { 0.0 })),
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            DataType::String
            | DataType::Date
            | DataType::Datetime
            | DataType::Time
            | DataType::Categorical => Ok(Self::String(match self {
                Self::String(value) => value,
                Self::Bool(value) => value.to_string().into_boxed_str(),
                Self::Integer(value) => value.to_string().into_boxed_str(),
                Self::Unsigned(value) => value.to_string().into_boxed_str(),
                Self::Decimal(value) => value.to_string().into_boxed_str(),
                Self::Null => "null".into(),
                _ => return Err(RuntimeValueError::InvalidCoercion),
            })),
            _ => Err(RuntimeValueError::InvalidCoercion),
        }
    }
}
