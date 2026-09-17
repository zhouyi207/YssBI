//! Runtime values shared by kernels and their callers, independent of graph identities.

use std::collections::BTreeMap;

use thiserror::Error;

use yss_data_contract::DataValue;
use yss_tabular_contract::TabularScalar;

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Annotated(std::sync::Arc<AnnotatedRuntimeValue>),
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

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedRuntimeValue {
    value: RuntimeValue,
    metadata: yss_data_contract::ConversionMetadata,
}

impl AnnotatedRuntimeValue {
    pub fn value(&self) -> &RuntimeValue {
        &self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum RuntimeValueError {
    #[error("runtime value is not representable")]
    Unrepresentable,
    #[error("runtime numeric value is not finite")]
    NonFinite,
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
    /// Retain conversion metadata when primitive carriers cannot express the meaning.
    /// An annotation contains only a scalar or flat scalar list, never another annotation or a handle.
    pub fn with_metadata(
        self,
        metadata: yss_data_contract::ConversionMetadata,
    ) -> Result<Self, RuntimeValueError> {
        let scalars = match &self {
            Self::List(values) => values.as_ref(),
            value => std::slice::from_ref(value),
        };
        for value in scalars {
            match value {
                Self::Null
                | Self::Bool(_)
                | Self::Integer(_)
                | Self::Unsigned(_)
                | Self::String(_) => {}
                Self::Decimal(value) if value.is_finite() => {}
                Self::Decimal(_) => return Err(RuntimeValueError::NonFinite),
                _ => return Err(RuntimeValueError::Unrepresentable),
            }
        }
        if matches!(
            metadata.semantic.kind,
            yss_data_contract::SemanticType::Numeric
                | yss_data_contract::SemanticType::Text
                | yss_data_contract::SemanticType::Binary
        ) {
            return Ok(self);
        }
        Ok(Self::Annotated(std::sync::Arc::new(
            AnnotatedRuntimeValue {
                value: self,
                metadata,
            },
        )))
    }

    pub fn metadata(&self) -> Option<&yss_data_contract::ConversionMetadata> {
        match self {
            Self::Annotated(value) => Some(&value.metadata),
            _ => None,
        }
    }

    pub fn unannotated(&self) -> &Self {
        match self {
            Self::Annotated(value) => value.value(),
            value => value,
        }
    }

    pub(crate) fn tabular_scalar(
        &self,
    ) -> Result<yss_tabular_contract::TabularScalar, RuntimeValueError> {
        Ok(match self.unannotated() {
            Self::Null => TabularScalar::Null,
            Self::Bool(v) => TabularScalar::Bool(*v),
            Self::Integer(v) => TabularScalar::Integer(*v),
            Self::Unsigned(v) => TabularScalar::Unsigned(*v),
            Self::Decimal(v) => {
                TabularScalar::Decimal((*v).try_into().map_err(|_| RuntimeValueError::NonFinite)?)
            }
            Self::String(v) => TabularScalar::String(v.clone()),
            _ => return Err(RuntimeValueError::Unrepresentable),
        })
    }

    pub(crate) fn numeric_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.tabular_scalar()
            .ok()?
            .compare(&other.tabular_scalar().ok()?)
    }

    pub(crate) fn semantic_eq(&self, other: &Self) -> bool {
        use RuntimeValue::{Decimal, Integer, Unsigned};
        match (self.unannotated(), other.unannotated()) {
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
            (left, right) => left == right,
        }
    }
}
