//! Runtime containers shared by kernels and callers, independent of graph identities.

use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;
use yss_data_contract::DataValue;
use yss_data_contract::TabularScalar;

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Annotated(Arc<AnnotatedRuntimeValue>),
    Scalar(TabularScalar),
    List(Arc<[RuntimeValue]>),
    Record(Arc<BTreeMap<Box<str>, RuntimeValue>>),
    Resource(Box<str>),
    Relation(yss_relational_contract::RelationHandle),
    Grouped(Arc<yss_relational_contract::GroupedRelationHandle>),
    Series(yss_relational_contract::SeriesHandle),
    LinearRegression(Arc<crate::LinearRegressionValue>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotatedRuntimeValue {
    value: RuntimeValue,
    metadata: Arc<yss_data_contract::ConversionMetadata>,
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

impl From<TabularScalar> for RuntimeValue {
    fn from(value: TabularScalar) -> Self {
        Self::Scalar(value)
    }
}

impl TryFrom<&DataValue> for RuntimeValue {
    type Error = RuntimeValueError;

    fn try_from(value: &DataValue) -> Result<Self, Self::Error> {
        Ok(match value {
            DataValue::Null => Self::Scalar(TabularScalar::Null),
            DataValue::Bool(value) => Self::Scalar(TabularScalar::Bool(*value)),
            DataValue::Integer(value) => Self::Scalar(TabularScalar::Integer(*value)),
            DataValue::Unsigned(value) => Self::Scalar(TabularScalar::Unsigned(*value)),
            DataValue::Decimal(value) => Self::float64(
                value
                    .as_str()
                    .parse()
                    .map_err(|_| RuntimeValueError::NonFinite)?,
            )?,
            DataValue::String(value) => Self::Scalar(TabularScalar::String(value.clone())),
            DataValue::Bytes(values) => Self::List(
                values
                    .iter()
                    .map(|value| Self::Scalar(TabularScalar::Unsigned(u64::from(*value))))
                    .collect(),
            ),
            DataValue::List(values) => Self::List(
                values
                    .iter()
                    .map(Self::try_from)
                    .collect::<Result<_, _>>()?,
            ),
            DataValue::Object(values) => Self::Record(Arc::new(
                values
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), Self::try_from(value)?)))
                    .collect::<Result<_, RuntimeValueError>>()?,
            )),
        })
    }
}

impl TryFrom<serde_json::Value> for RuntimeValue {
    type Error = RuntimeValueError;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        Self::from_json_checked(value, &mut || Ok(())).map_err(|error| match error {
            crate::KernelError::NonFiniteResult => RuntimeValueError::NonFinite,
            _ => RuntimeValueError::Unrepresentable,
        })
    }
}

impl RuntimeValue {
    pub(crate) fn from_json_checked(
        value: serde_json::Value,
        check: &mut impl FnMut() -> Result<(), crate::KernelError>,
    ) -> Result<Self, crate::KernelError> {
        check()?;
        Ok(match value {
            serde_json::Value::Null => TabularScalar::Null.into(),
            serde_json::Value::Bool(value) => TabularScalar::Bool(value).into(),
            serde_json::Value::String(value) => TabularScalar::String(value.into()).into(),
            serde_json::Value::Number(value) => {
                if let Some(number) = value.as_i64() {
                    TabularScalar::Integer(number).into()
                } else if let Some(number) = value.as_u64() {
                    TabularScalar::Unsigned(number).into()
                } else {
                    Self::float64(value.as_f64().ok_or(crate::KernelError::NonFiniteResult)?)
                        .map_err(|_| crate::KernelError::NonFiniteResult)?
                }
            }
            serde_json::Value::Array(values) => Self::List(
                values
                    .into_iter()
                    .map(|value| Self::from_json_checked(value, check))
                    .collect::<Result<_, _>>()?,
            ),
            serde_json::Value::Object(values) => Self::Record(Arc::new(
                values
                    .into_iter()
                    .map(|(key, value)| {
                        Ok((key.into_boxed_str(), Self::from_json_checked(value, check)?))
                    })
                    .collect::<Result<_, crate::KernelError>>()?,
            )),
        })
    }

    /// Whether the value contains only computed values, without relational expressions.
    pub fn is_immediate(&self) -> bool {
        match self {
            Self::Relation(_) | Self::Grouped(_) | Self::Series(_) | Self::Resource(_) => false,
            Self::List(values) => values.iter().all(Self::is_immediate),
            Self::Record(values) => values.values().all(Self::is_immediate),
            Self::Annotated(_) | Self::Scalar(_) | Self::LinearRegression(_) => true,
        }
    }

    pub fn float64(value: f64) -> Result<Self, RuntimeValueError> {
        Ok(Self::Scalar(TabularScalar::Float64(
            value.try_into().map_err(|_| RuntimeValueError::NonFinite)?,
        )))
    }

    /// Check the carrier; Graph's resolved types and producers own element semantics.
    pub fn matches_carrier(&self, expected: &yss_data_contract::ValueType) -> bool {
        use yss_data_contract::{SemanticType, ValueType};
        match expected {
            ValueType::Any => true,
            ValueType::OneOf(types) => types.iter().any(|ty| self.matches_carrier(ty)),
            ValueType::Scalar(semantic) => {
                if let Some(metadata) = self.metadata() {
                    return metadata.semantic.kind == *semantic
                        && matches!(self.unannotated(), Self::Scalar(_));
                }
                matches!(
                    (semantic, self.unannotated()),
                    (_, Self::Scalar(TabularScalar::Null))
                        | (
                            SemanticType::Numeric,
                            Self::Scalar(
                                TabularScalar::Integer(_)
                                    | TabularScalar::Unsigned(_)
                                    | TabularScalar::Float64(_),
                            ),
                        )
                        | (SemanticType::Binary, Self::Scalar(TabularScalar::Bool(_)))
                        | (SemanticType::Text, Self::Scalar(TabularScalar::String(_)))
                        | (
                            SemanticType::Categorical
                                | SemanticType::Ordinal
                                | SemanticType::Datetime
                                | SemanticType::Identifier,
                            Self::Scalar(_),
                        )
                )
            }
            ValueType::DataFrame => matches!(self, Self::Relation(_)),
            ValueType::DataSeries(_) => {
                matches!(self.unannotated(), Self::List(_) | Self::Series(_))
            }
            ValueType::Array(_) => matches!(self.unannotated(), Self::List(_)),
            ValueType::Object => matches!(self, Self::Record(_)),
            ValueType::Struct(id) if id == yss_data_contract::GROUPED_DATAFRAME_TYPE_ID => {
                matches!(self, Self::Grouped(_))
            }
            ValueType::Struct(_) => matches!(
                self,
                Self::LinearRegression(_) | Self::Record(_) | Self::Resource(_)
            ),
        }
    }

    /// Annotate materialized scalar values only; annotations never wrap handles or annotations.
    pub fn with_metadata(
        self,
        metadata: impl Into<Arc<yss_data_contract::ConversionMetadata>>,
    ) -> Result<Self, RuntimeValueError> {
        let scalars = match &self {
            Self::List(values) => values.as_ref(),
            value => std::slice::from_ref(value),
        };
        if scalars
            .iter()
            .any(|value| !matches!(value, Self::Scalar(_)))
        {
            return Err(RuntimeValueError::Unrepresentable);
        }
        let metadata = metadata.into();
        if metadata.dummy_base_level.is_none()
            && matches!(
                metadata.semantic.kind,
                yss_data_contract::SemanticType::Numeric | yss_data_contract::SemanticType::Text
            )
        {
            return Ok(self);
        }
        Ok(Self::Annotated(Arc::new(AnnotatedRuntimeValue {
            value: self,
            metadata,
        })))
    }

    pub fn metadata(&self) -> Option<&yss_data_contract::ConversionMetadata> {
        self.shared_metadata().map(Arc::as_ref)
    }

    pub(crate) fn shared_metadata(&self) -> Option<&Arc<yss_data_contract::ConversionMetadata>> {
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

    pub(crate) fn tabular_scalar(&self) -> Result<TabularScalar, RuntimeValueError> {
        match self.unannotated() {
            Self::Scalar(value) => Ok(value.clone()),
            _ => Err(RuntimeValueError::Unrepresentable),
        }
    }
}
