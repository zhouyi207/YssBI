use super::{Artifact, ArtifactKind, ArtifactValueKind, KernelError, RuntimeValue};
use crate::graph::protocol::Value;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataSeriesElementType {
    Int64,
    Float64,
    String,
    Boolean,
    Date,
    Datetime,
    Categorical,
}

impl fmt::Display for DataSeriesElementType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Int64 => "Int64",
            Self::Float64 => "Float64",
            Self::String => "String",
            Self::Boolean => "Boolean",
            Self::Date => "Date",
            Self::Datetime => "Datetime",
            Self::Categorical => "Categorical",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSeriesMetadata {
    pub element_type: DataSeriesElementType,
    pub length: usize,
    pub null_count: usize,
    pub name: Option<Box<str>>,
    pub format: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullPolicy {
    Propagate,
    Skip,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSeriesContractError(Box<str>);

impl DataSeriesContractError {
    fn new(message: impl Into<Box<str>>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DataSeriesContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for DataSeriesContractError {}

#[derive(Debug, Clone)]
pub struct DataSeriesBuilder {
    element_type: DataSeriesElementType,
    values: Box<[Value]>,
    name: Option<Box<str>>,
    format: Option<Box<str>>,
}

impl DataSeriesBuilder {
    pub fn new(element_type: DataSeriesElementType) -> Self {
        Self {
            element_type,
            values: Box::default(),
            name: None,
            format: None,
        }
    }

    pub fn values(mut self, values: impl Into<Box<[Value]>>) -> Self {
        self.values = values.into();
        self
    }

    pub fn name(mut self, name: impl Into<Box<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn format(mut self, format: impl Into<Box<str>>) -> Self {
        self.format = Some(format.into());
        self
    }

    pub fn build(self, kind: ArtifactKind) -> Result<Artifact, DataSeriesContractError> {
        let metadata = DataSeriesMetadata {
            element_type: self.element_type,
            length: self.values.len(),
            null_count: self
                .values
                .iter()
                .filter(|value| matches!(value, Value::Null))
                .count(),
            name: self.name,
            format: self.format,
        };
        Artifact::new_data_series(kind, metadata, self.values)
    }
}

pub(crate) fn validate_data_series_values(
    metadata: &DataSeriesMetadata,
    values: &[Value],
) -> Result<(), DataSeriesContractError> {
    if metadata.length != values.len() {
        return Err(DataSeriesContractError::new(format!(
            "DataSeries metadata length {} does not match {} values",
            metadata.length,
            values.len()
        )));
    }
    let null_count = values
        .iter()
        .filter(|value| matches!(value, Value::Null))
        .count();
    if metadata.null_count != null_count {
        return Err(DataSeriesContractError::new(format!(
            "DataSeries metadata null count {} does not match {null_count} nulls",
            metadata.null_count
        )));
    }
    for (index, value) in values.iter().enumerate() {
        if !value_matches(metadata.element_type, value) {
            return Err(DataSeriesContractError::new(format!(
                "DataSeries {} element at index {index} has incompatible {} storage",
                metadata.element_type,
                value_storage_name(value)
            )));
        }
    }
    Ok(())
}

fn value_matches(element_type: DataSeriesElementType, value: &Value) -> bool {
    match (element_type, value) {
        (_, Value::Null)
        | (DataSeriesElementType::Int64, Value::Integer(_))
        | (DataSeriesElementType::Float64, Value::Decimal(_))
        | (DataSeriesElementType::String, Value::String(_))
        | (DataSeriesElementType::Boolean, Value::Bool(_))
        | (DataSeriesElementType::Date, Value::String(_))
        | (DataSeriesElementType::Datetime, Value::String(_))
        | (DataSeriesElementType::Categorical, Value::String(_)) => true,
        (DataSeriesElementType::Float64, Value::String(value)) => is_float64_special_value(value),
        _ => false,
    }
}

fn is_float64_special_value(value: &str) -> bool {
    matches!(value, "NaN" | "Infinity" | "-Infinity")
}

fn value_storage_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "Null",
        Value::Bool(_) => "Boolean",
        Value::Integer(_) => "Int64",
        Value::Unsigned(_) => "Unsigned",
        Value::Decimal(_) => "Decimal",
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::List(_) => "List",
        Value::Object(_) => "Object",
    }
}

pub fn validate_data_series_type_expr(
    metadata: &DataSeriesMetadata,
    type_expr: &crate::graph::protocol::TypeExpr,
) -> Result<(), DataSeriesContractError> {
    use crate::graph::protocol::TypeExpr;
    match type_expr {
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == crate::graph::protocol::DATA_SERIES_CONSTRUCTOR_ID
            && arguments.len() == 1 =>
        {
            validate_element_type_expr(metadata.element_type, &arguments[0])
        }
        TypeExpr::Union(members) => {
            if members
                .iter()
                .any(|member| validate_data_series_type_expr(metadata, member).is_ok())
            {
                Ok(())
            } else {
                Err(DataSeriesContractError::new(format!(
                    "DataSeries contract does not accept {} metadata",
                    metadata.element_type
                )))
            }
        }
        _ => Err(DataSeriesContractError::new(
            "DataSeries contract must use canonical core.data_series<T>",
        )),
    }
}

fn validate_element_type_expr(
    actual: DataSeriesElementType,
    type_expr: &crate::graph::protocol::TypeExpr,
) -> Result<(), DataSeriesContractError> {
    use crate::graph::protocol::TypeExpr;
    let expected = match type_expr {
        TypeExpr::Concrete(id) => match id.as_str() {
            "core.int64" => Some(DataSeriesElementType::Int64),
            "core.float64" => Some(DataSeriesElementType::Float64),
            "core.string" => Some(DataSeriesElementType::String),
            "core.bool" => Some(DataSeriesElementType::Boolean),
            "core.date" => Some(DataSeriesElementType::Date),
            "core.datetime" => Some(DataSeriesElementType::Datetime),
            "core.categorical" => Some(DataSeriesElementType::Categorical),
            _ => None,
        },
        TypeExpr::Union(members) => {
            if members
                .iter()
                .any(|member| validate_element_type_expr(actual, member).is_ok())
            {
                return Ok(());
            }
            None
        }
        _ => None,
    };
    match expected {
        Some(expected) if expected == actual => Ok(()),
        Some(expected) => Err(DataSeriesContractError::new(format!(
            "DataSeries contract expects {expected}, received {actual} metadata"
        ))),
        None => Err(DataSeriesContractError::new(
            "DataSeries contract has an unsupported element type",
        )),
    }
}

pub fn require_data_series(value: &RuntimeValue) -> Result<&Artifact, KernelError> {
    match value {
        RuntimeValue::Artifact(artifact)
            if artifact.value_kind() == ArtifactValueKind::DataSeries =>
        {
            Ok(artifact)
        }
        RuntimeValue::Artifact(_) => Err(KernelError::new(
            "expected DataSeries Artifact, received sequence Artifact",
        )),
        RuntimeValue::Scalar(_) => Err(KernelError::new(
            "expected DataSeries Artifact, received scalar",
        )),
        RuntimeValue::Stream(_) => Err(KernelError::new(
            "expected DataSeries Artifact, received stream",
        )),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Int64SeriesView {
    metadata: DataSeriesMetadata,
    values: Box<[Option<i64>]>,
}

impl Int64SeriesView {
    pub fn metadata(&self) -> &DataSeriesMetadata {
        &self.metadata
    }

    pub fn values(&self) -> &[Option<i64>] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Float64SeriesView {
    metadata: DataSeriesMetadata,
    values: Box<[Option<f64>]>,
}

impl Float64SeriesView {
    pub fn metadata(&self) -> &DataSeriesMetadata {
        &self.metadata
    }

    pub fn values(&self) -> &[Option<f64>] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringSeriesView {
    metadata: DataSeriesMetadata,
    values: Box<[Option<Box<str>>]>,
}

impl StringSeriesView {
    pub fn metadata(&self) -> &DataSeriesMetadata {
        &self.metadata
    }

    pub fn values(&self) -> &[Option<Box<str>>] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BooleanSeriesView {
    metadata: DataSeriesMetadata,
    values: Box<[Option<bool>]>,
}

impl BooleanSeriesView {
    pub fn metadata(&self) -> &DataSeriesMetadata {
        &self.metadata
    }

    pub fn values(&self) -> &[Option<bool>] {
        &self.values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NumericSeriesView {
    Int64(Int64SeriesView),
    Float64(Float64SeriesView),
}

pub fn numeric_series(
    artifact: &Artifact,
    policy: NullPolicy,
) -> Result<NumericSeriesView, KernelError> {
    let metadata = require_metadata(artifact)?;
    match metadata.element_type {
        DataSeriesElementType::Int64 => Ok(NumericSeriesView::Int64(Int64SeriesView {
            metadata: metadata.clone(),
            values: read_values(artifact, policy, |value| match value {
                Value::Integer(value) => Some(value),
                _ => None,
            })?,
        })),
        DataSeriesElementType::Float64 => Ok(NumericSeriesView::Float64(Float64SeriesView {
            metadata: metadata.clone(),
            values: read_values(artifact, policy, |value| match value {
                Value::Decimal(value) => value.as_str().parse().ok(),
                Value::String(value) if is_float64_special_value(&value) => {
                    value.as_ref().parse().ok()
                }
                _ => None,
            })?,
        })),
        actual => Err(type_error("numeric", actual)),
    }
}

pub fn string_series(
    artifact: &Artifact,
    policy: NullPolicy,
) -> Result<StringSeriesView, KernelError> {
    let metadata = require_metadata(artifact)?;
    if metadata.element_type != DataSeriesElementType::String {
        return Err(type_error("String", metadata.element_type));
    }
    Ok(StringSeriesView {
        metadata: metadata.clone(),
        values: read_values(artifact, policy, |value| match value {
            Value::String(value) => Some(value),
            _ => None,
        })?,
    })
}

pub fn boolean_series(
    artifact: &Artifact,
    policy: NullPolicy,
) -> Result<BooleanSeriesView, KernelError> {
    let metadata = require_metadata(artifact)?;
    if metadata.element_type != DataSeriesElementType::Boolean {
        return Err(type_error("Boolean", metadata.element_type));
    }
    Ok(BooleanSeriesView {
        metadata: metadata.clone(),
        values: read_values(artifact, policy, |value| match value {
            Value::Bool(value) => Some(value),
            _ => None,
        })?,
    })
}

fn require_metadata(artifact: &Artifact) -> Result<&DataSeriesMetadata, KernelError> {
    artifact
        .data_series_metadata()
        .ok_or_else(|| KernelError::new("expected DataSeries Artifact, received sequence Artifact"))
}

fn read_values<T>(
    artifact: &Artifact,
    policy: NullPolicy,
    convert: impl Fn(Value) -> Option<T>,
) -> Result<Box<[Option<T>]>, KernelError> {
    let metadata = require_metadata(artifact)?;
    let mut output = Vec::with_capacity(metadata.length);
    let mut null_count = 0_usize;
    for (index, value) in artifact
        .cursor()
        .map_err(|error| KernelError::new(error.to_string()))?
        .enumerate()
    {
        let value = value.map_err(|error| KernelError::new(error.to_string()))?;
        if matches!(value, Value::Null) {
            null_count += 1;
            match policy {
                NullPolicy::Propagate => output.push(None),
                NullPolicy::Skip => {}
                NullPolicy::Reject => {
                    return Err(KernelError::new(format!(
                        "DataSeries contains null at index {index}"
                    )));
                }
            }
        } else {
            let storage = value_storage_name(&value);
            let converted = convert(value).ok_or_else(|| {
                KernelError::new(format!(
                    "DataSeries {} element at index {index} has incompatible {storage} storage",
                    metadata.element_type
                ))
            })?;
            output.push(Some(converted));
        }
    }
    let observed_length = match policy {
        NullPolicy::Skip => output.len() + null_count,
        NullPolicy::Propagate | NullPolicy::Reject => output.len(),
    };
    if observed_length != metadata.length || null_count != metadata.null_count {
        return Err(KernelError::new(
            "DataSeries storage does not match authoritative metadata",
        ));
    }
    Ok(output.into_boxed_slice())
}

fn type_error(expected: &str, actual: DataSeriesElementType) -> KernelError {
    KernelError::new(format!("expected {expected} DataSeries, received {actual}"))
}

pub fn checked_int64_to_f64(value: i64) -> Result<f64, KernelError> {
    const MAX_EXACT_INTEGER: i64 = 1_i64 << 53;
    if (-MAX_EXACT_INTEGER..=MAX_EXACT_INTEGER).contains(&value) {
        Ok(value as f64)
    } else {
        Err(KernelError::new(format!(
            "Int64 value {value} cannot be represented exactly as Float64"
        )))
    }
}
