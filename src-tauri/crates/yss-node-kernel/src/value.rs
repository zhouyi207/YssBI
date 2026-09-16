//! Runtime values shared by kernels and their callers, independent of graph identities.

use std::collections::BTreeMap;

use thiserror::Error;

use yss_data_contract::{DataValue, ValueType};

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
    Relation(yss_relational_contract::RelationHandle),
    Series(yss_relational_contract::SeriesHandle),
    Ols(std::sync::Arc<yss_sci_contract::scientific::OlsResult>),
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
    pub(crate) fn numeric_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use RuntimeValue::{Decimal, Integer, Unsigned};
        // i128 represents every supported integer. A finite float is truncated
        // before comparison, so integers never round through f64. Saturation
        // beyond i128 is safe here: both integer variants are strictly inside it.
        fn integer_decimal(integer: i128, decimal: f64) -> Option<std::cmp::Ordering> {
            decimal.is_finite().then(|| {
                integer.cmp(&(decimal as i128)).then_with(|| {
                    0.0_f64
                        .partial_cmp(&decimal.fract())
                        .expect("finite fraction")
                })
            })
        }
        match (self, other) {
            (Integer(a), Integer(b)) => Some(a.cmp(b)),
            (Unsigned(a), Unsigned(b)) => Some(a.cmp(b)),
            (Integer(a), Unsigned(b)) => Some(i128::from(*a).cmp(&i128::from(*b))),
            (Unsigned(a), Integer(b)) => Some(i128::from(*a).cmp(&i128::from(*b))),
            (Integer(a), Decimal(b)) => integer_decimal(i128::from(*a), *b),
            (Unsigned(a), Decimal(b)) => integer_decimal(i128::from(*a), *b),
            (Decimal(a), Integer(b)) => integer_decimal(i128::from(*b), *a).map(|o| o.reverse()),
            (Decimal(a), Unsigned(b)) => integer_decimal(i128::from(*b), *a).map(|o| o.reverse()),
            (Decimal(a), Decimal(b)) if a.is_finite() && b.is_finite() => a.partial_cmp(b),
            _ => None,
        }
    }

    pub(crate) fn semantic_eq(&self, other: &Self) -> bool {
        use RuntimeValue::{Decimal, Integer, Unsigned};
        match (self, other) {
            (Integer(_) | Unsigned(_) | Decimal(_), Integer(_) | Unsigned(_) | Decimal(_)) => {
                self.numeric_cmp(other) == Some(std::cmp::Ordering::Equal)
            }
            (Self::List(a), Self::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.semantic_eq(b))
            }
            (Self::Record(a), Self::Record(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(key, a)| b.get(key).is_some_and(|b| a.semantic_eq(b)))
            }
            _ => self == other,
        }
    }

    pub fn coerce_to(self, target: &ValueType) -> Result<Self, RuntimeValueError> {
        use yss_data_contract::SemanticType;
        match target {
            ValueType::Any => Ok(self),
            ValueType::Scalar(SemanticType::Numeric) => match self {
                Self::Integer(_) | Self::Unsigned(_) => Ok(self),
                Self::Decimal(value) if value.is_finite() => Ok(Self::Decimal(value)),
                Self::Bool(value) => Ok(Self::Integer(i64::from(value))),
                Self::String(value) => {
                    if let Ok(integer) = value.parse::<i64>() {
                        return Ok(Self::Integer(integer));
                    }
                    if let Ok(integer) = value.parse::<u64>() {
                        return Ok(Self::Unsigned(integer));
                    }
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .map(Self::Decimal)
                        .ok_or(RuntimeValueError::InvalidCoercion)
                }
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            ValueType::Scalar(SemanticType::Binary) => match self {
                Self::Bool(_) | Self::Null => Ok(self),
                Self::Integer(0) | Self::Unsigned(0) => Ok(Self::Bool(false)),
                Self::Integer(1) | Self::Unsigned(1) => Ok(Self::Bool(true)),
                Self::Decimal(0.0) => Ok(Self::Bool(false)),
                Self::Decimal(1.0) => Ok(Self::Bool(true)),
                Self::String(value) if value.as_ref() == "true" => Ok(Self::Bool(true)),
                Self::String(value) if value.as_ref() == "false" => Ok(Self::Bool(false)),
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            ValueType::Scalar(SemanticType::Text) => match self {
                Self::String(_) | Self::Null => Ok(self),
                Self::Bool(value) => Ok(Self::String(value.to_string().into())),
                Self::Integer(value) => Ok(Self::String(value.to_string().into())),
                Self::Unsigned(value) => Ok(Self::String(value.to_string().into())),
                Self::Decimal(value) if value.is_finite() => {
                    Ok(Self::String(value.to_string().into()))
                }
                _ => Err(RuntimeValueError::InvalidCoercion),
            },
            _ => Err(RuntimeValueError::InvalidCoercion),
        }
    }
}
