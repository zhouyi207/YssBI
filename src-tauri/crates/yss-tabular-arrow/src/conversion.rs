//! Prepared semantic conversion shared by materialized values and lazy expressions.

use crate::semantic::PreparedSemanticValidator;
use crate::{TabularArrowError, column_semantic, lossless_cast, with_column_semantic};
use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, StringArray, UInt64Array,
    new_null_array,
};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, Field, TimeUnit};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};
use yss_data_contract::{
    ColumnSemantic, ConversionMetadata, DatetimeRepresentation, NumericConstraints,
    NumericRepresentation, SemanticConversion, SemanticType, SemanticValue, TemporalPrecision,
    TemporalType,
};
use yss_tabular_contract::TabularScalar;

pub struct ConvertedValues {
    pub values: Vec<TabularScalar>,
    pub metadata: ConversionMetadata,
}

/// Import primitive values with their declared meaning or retained conversion metadata.
/// A bare categorical literal establishes its unordered domain; explicit domains stay fixed.
pub fn materialized_column(
    name: &str,
    values: &[TabularScalar],
    metadata: Option<&ConversionMetadata>,
) -> Result<(Field, ArrayRef), TabularArrowError> {
    let mut array = crate::scalar::scalars_to_array(values)?;
    if array.data_type() == &DataType::Null
        && let Some(metadata) = metadata
    {
        let dtype = match metadata.semantic.kind {
            SemanticType::Numeric => DataType::Float64,
            SemanticType::Binary => DataType::Boolean,
            SemanticType::Datetime => metadata
                .temporal
                .map(temporal_type)
                .unwrap_or(DataType::Timestamp(TimeUnit::Microsecond, None)),
            _ => DataType::Utf8,
        };
        array = arrow::array::new_null_array(&dtype, values.len());
    }
    if let Some(temporal) = metadata.and_then(|metadata| metadata.temporal) {
        array = lossless_cast(array.as_ref(), &temporal_type(temporal), false)?;
    } else if metadata.is_some_and(|metadata| metadata.semantic.kind == SemanticType::Datetime)
        && !array.data_type().is_temporal()
    {
        array = lossless_cast(
            array.as_ref(),
            &DataType::Timestamp(TimeUnit::Microsecond, None),
            false,
        )?;
    }
    let mut field = Field::new(name, array.data_type().clone(), array.null_count() != 0);
    if let Some(metadata) = metadata {
        let semantic = if metadata.semantic.kind == SemanticType::Categorical
            && metadata.semantic.values.is_empty()
        {
            // Bare categorical literals declare their meaning without a codebook. Establish
            // an unordered domain once; explicit domains and ordinal levels stay authoritative.
            let codes = cast(array.as_ref(), &DataType::Utf8)?;
            let codes = codes
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or(TabularArrowError::UnsupportedType)?;
            let mut distinct = std::collections::BTreeSet::new();
            for code in codes.iter().flatten() {
                distinct.insert(code);
                if distinct.len() > 65_536 {
                    return Err(TabularArrowError::InvalidValue);
                }
            }
            let mut semantic = metadata.semantic.clone();
            semantic.values = distinct
                .into_iter()
                .map(|code| SemanticValue {
                    value: code.into(),
                    label: code.into(),
                })
                .collect();
            semantic
        } else if metadata.semantic.kind == SemanticType::Binary
            && metadata.semantic.values.is_empty()
            && array.data_type() == &DataType::Boolean
        {
            crate::column_semantic(&field)?
        } else {
            metadata.semantic.clone()
        };
        field = with_column_semantic(field, &semantic)?;
        crate::validate_semantic_array(&field, array.as_ref())?;
    }
    Ok((field, array))
}

pub fn convert_semantic_values(
    values: &[TabularScalar],
    source_metadata: Option<&ConversionMetadata>,
    spec: &SemanticConversion,
) -> Result<ConvertedValues, TabularArrowError> {
    let (field, array) = materialized_column("value", values, source_metadata)?;
    let conversion = PreparedConversion::new(&field, spec)?;
    let result = conversion.convert(array.as_ref())?;
    let result = if conversion.metadata.temporal.is_some() {
        cast(result.as_ref(), &DataType::Utf8)?
    } else {
        result
    };
    let values = materialized_values(result.as_ref())?;
    Ok(ConvertedValues {
        values,
        metadata: conversion.metadata,
    })
}

fn materialized_values(array: &dyn Array) -> Result<Vec<TabularScalar>, TabularArrowError> {
    macro_rules! values {
        ($array:ty, $variant:ident) => {
            if let Some(array) = array.as_any().downcast_ref::<$array>() {
                return Ok(array
                    .iter()
                    .map(|value| {
                        value.map_or(TabularScalar::Null, |value| {
                            TabularScalar::$variant(value.into())
                        })
                    })
                    .collect());
            }
        };
    }
    values!(BooleanArray, Bool);
    values!(Int64Array, Integer);
    values!(UInt64Array, Unsigned);
    values!(StringArray, String);
    if let Some(array) = array.as_any().downcast_ref::<Float64Array>() {
        return array
            .iter()
            .map(|value| match value {
                None => Ok(TabularScalar::Null),
                Some(value) => Ok(TabularScalar::Decimal(
                    value
                        .try_into()
                        .map_err(|_| TabularArrowError::InvalidValue)?,
                )),
            })
            .collect();
    }
    Err(TabularArrowError::UnsupportedType)
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ConversionAction {
    Copy,
    Cast,
    Lossless,
    NumericText,
    BinaryText,
    BinaryNumeric,
    BinaryCodes { positive: String, negative: String },
    CalendarText,
    CalendarFormat(String),
}

/// Immutable field/validation preparation. Execution only processes the current batch.
#[derive(Debug)]
pub struct PreparedConversion {
    source: Field,
    output: Arc<Field>,
    metadata: ConversionMetadata,
    source_validation: PreparedSemanticValidator,
    output_validation: PreparedSemanticValidator,
    binary_input: Option<String>,
    action: ConversionAction,
}

// Compiled lookup tables are derived from this complete execution identity. Inactive
// configuration is deliberately absent, so equivalent expressions can share work.
impl PartialEq for PreparedConversion {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.output == other.output
            && self.binary_input == other.binary_input
            && self.action == other.action
    }
}
impl Eq for PreparedConversion {}
impl Hash for PreparedConversion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.output.hash(state);
        self.binary_input.hash(state);
        self.action.hash(state);
    }
}

impl PreparedConversion {
    pub fn new(source: &Field, spec: &SemanticConversion) -> Result<Self, TabularArrowError> {
        let meaning = column_semantic(source)?;
        let (output, semantic) = target_field(source, spec, &meaning)?;
        let source_validation = PreparedSemanticValidator::new(source, &meaning)?;
        let output_validation = PreparedSemanticValidator::new(&output, &semantic)?;
        let binary_input = if meaning.kind == SemanticType::Binary
            && (source.data_type() != &DataType::Boolean || meaning.positive_value.is_some())
            && matches!(spec.target, SemanticType::Numeric | SemanticType::Binary)
            && (spec.target != SemanticType::Binary || spec.domain.values.is_empty())
        {
            Some(
                meaning
                    .positive_value
                    .ok_or(TabularArrowError::InvalidValue)?,
            )
        } else {
            None
        };
        let input = if binary_input.is_some() {
            &DataType::Boolean
        } else {
            source.data_type()
        };
        let action = match spec.target {
            SemanticType::Text if input.is_temporal() => ConversionAction::CalendarText,
            SemanticType::Text => ConversionAction::Cast,
            SemanticType::Numeric if is_textual(input) => ConversionAction::NumericText,
            SemanticType::Numeric => ConversionAction::Lossless,
            SemanticType::Binary if !spec.domain.values.is_empty() => {
                let positive = spec
                    .domain
                    .positive_value
                    .clone()
                    .ok_or(TabularArrowError::InvalidValue)?;
                let negative = spec
                    .domain
                    .values
                    .iter()
                    .find(|value| value.value != positive)
                    .ok_or(TabularArrowError::InvalidValue)?
                    .value
                    .clone();
                ConversionAction::BinaryCodes { positive, negative }
            }
            SemanticType::Binary if input == &DataType::Boolean => ConversionAction::Copy,
            SemanticType::Binary if is_textual(input) => ConversionAction::BinaryText,
            SemanticType::Binary => ConversionAction::BinaryNumeric,
            SemanticType::Identifier | SemanticType::Categorical | SemanticType::Ordinal => {
                if input == output.data_type() {
                    ConversionAction::Copy
                } else {
                    ConversionAction::Cast
                }
            }
            SemanticType::Datetime if !spec.format.is_empty() && is_textual(input) => {
                ConversionAction::CalendarFormat(spec.format.clone())
            }
            SemanticType::Datetime => ConversionAction::Lossless,
        };
        let metadata = ConversionMetadata {
            semantic,
            temporal: temporal_metadata(output.data_type()),
        };
        Ok(Self {
            source: source.clone(),
            output: Arc::new(output),
            metadata,
            source_validation,
            output_validation,
            binary_input,
            action,
        })
    }

    pub fn field(&self) -> &Arc<Field> {
        &self.output
    }

    pub fn convert(&self, array: &dyn Array) -> Result<ArrayRef, TabularArrowError> {
        if self.source.data_type() != array.data_type() {
            return Err(TabularArrowError::InvalidSchema);
        }
        if array.data_type() == &DataType::Null {
            return Ok(new_null_array(self.output.data_type(), array.len()));
        }
        self.source_validation.validate(array)?;
        finite(array)?;
        let boolean;
        let array = if let Some(positive) = &self.binary_input {
            boolean = BooleanArray::from_iter(
                strings(array)?
                    .iter()
                    .map(|value| value.map(|value| value == positive)),
            );
            &boolean as &dyn Array
        } else {
            array
        };
        let target = self.output.data_type();
        let result = match &self.action {
            ConversionAction::Copy => arrow::array::make_array(array.to_data()),
            ConversionAction::Cast => cast(array, target)?,
            ConversionAction::Lossless => lossless_cast(array, target, false)?,
            ConversionAction::CalendarText => {
                cast(crate::timezone_free_array(array)?.as_ref(), target)?
            }
            ConversionAction::NumericText => {
                let text = strings(array)?;
                let trimmed = StringArray::from_iter(text.iter().map(|value| value.map(str::trim)));
                let converted = cast(&trimmed, target)?;
                finite(converted.as_ref())?;
                let rendered = strings(converted.as_ref())?;
                for (original, rendered) in trimmed.iter().zip(rendered.iter()) {
                    if let Some(original) = original {
                        let original =
                            decimal_identity(original).ok_or(TabularArrowError::InvalidValue)?;
                        if Some(original) != rendered.and_then(decimal_identity) {
                            return Err(TabularArrowError::InvalidValue);
                        }
                    }
                }
                converted
            }
            ConversionAction::BinaryCodes { positive, negative } => {
                let values = strings(array)?
                    .iter()
                    .map(|value| match value {
                        None => Ok(None),
                        Some(value) if value == positive => Ok(Some(true)),
                        Some(value) if value == negative => Ok(Some(false)),
                        _ => Err(TabularArrowError::InvalidValue),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Arc::new(BooleanArray::from(values))
            }
            ConversionAction::BinaryText => {
                let values = strings(array)?
                    .iter()
                    .map(|value| match value.map(str::trim) {
                        None => Ok(None),
                        Some("true" | "1") => Ok(Some(true)),
                        Some("false" | "0") => Ok(Some(false)),
                        _ => Err(TabularArrowError::InvalidValue),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Arc::new(BooleanArray::from(values))
            }
            ConversionAction::BinaryNumeric => {
                let values = lossless_cast(array, &DataType::Float64, false)?;
                let values = values
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or(TabularArrowError::InvalidValue)?;
                let values = values
                    .iter()
                    .map(|value| match value {
                        None => Ok(None),
                        Some(0.0) => Ok(Some(false)),
                        Some(1.0) => Ok(Some(true)),
                        _ => Err(TabularArrowError::InvalidValue),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Arc::new(BooleanArray::from(values))
            }
            ConversionAction::CalendarFormat(format) => {
                let parsed = strings(array)?
                    .iter()
                    .map(|value| value.map(|value| parse_calendar(value, format)).transpose())
                    .collect::<Result<Vec<_>, _>>()?;
                lossless_cast(&StringArray::from(parsed), target, false)?
            }
        };
        finite(result.as_ref())?;
        self.output_validation.validate(result.as_ref())?;
        Ok(result)
    }
}

fn target_field(
    source: &Field,
    spec: &SemanticConversion,
    meaning: &ColumnSemantic,
) -> Result<(Field, ColumnSemantic), TabularArrowError> {
    if !spec.domain.is_valid() || spec.format.len() > 256 {
        return Err(TabularArrowError::InvalidValue);
    }
    if spec.target == SemanticType::Binary
        && !spec.domain.values.is_empty()
        && (spec.domain.values.len() != 2 || spec.domain.positive_value.is_none())
    {
        return Err(TabularArrowError::InvalidValue);
    }
    if spec.target == SemanticType::Datetime
        && chrono::format::StrftimeItems::new(&spec.format)
            .any(|item| matches!(item, chrono::format::Item::Error))
    {
        return Err(TabularArrowError::InvalidValue);
    }
    if meaning.kind == SemanticType::Binary
        && matches!(spec.target, SemanticType::Numeric | SemanticType::Binary)
        && source.data_type() != &DataType::Boolean
        && meaning.positive_value.is_none()
        && (spec.target != SemanticType::Binary || spec.domain.values.is_empty())
    {
        return Err(TabularArrowError::InvalidValue);
    }
    if spec.target == SemanticType::Numeric
        && meaning.kind == SemanticType::Numeric
        && spec.numeric == NumericRepresentation::Auto
    {
        return Ok((source.clone(), meaning.clone()));
    }
    let dtype = match spec.target {
        SemanticType::Numeric => match spec.numeric {
            NumericRepresentation::Integer => DataType::Int64,
            NumericRepresentation::Real => DataType::Float64,
            NumericRepresentation::Auto if meaning.kind == SemanticType::Binary => DataType::Int64,
            NumericRepresentation::Auto if source.data_type().is_numeric() => {
                source.data_type().clone()
            }
            NumericRepresentation::Auto => DataType::Float64,
        },
        SemanticType::Text => DataType::Utf8,
        SemanticType::Binary => DataType::Boolean,
        SemanticType::Identifier | SemanticType::Categorical | SemanticType::Ordinal => {
            match source.data_type() {
                DataType::Null => DataType::Utf8,
                DataType::Dictionary(_, values) => values.as_ref().clone(),
                dtype => dtype.clone(),
            }
        }
        SemanticType::Datetime
            if spec.datetime == DatetimeRepresentation::Auto
                && source.data_type().is_temporal() =>
        {
            crate::timezone_free_data_type(source.data_type())
        }
        SemanticType::Datetime => temporal_type(resolve_temporal(spec.datetime, spec.precision)),
    };
    let mut semantic = ColumnSemantic::new(spec.target);
    if matches!(
        spec.target,
        SemanticType::Categorical | SemanticType::Ordinal
    ) {
        semantic.values = if !spec.domain.values.is_empty() {
            spec.domain.values.clone()
        } else if matches!(
            meaning.kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        ) {
            meaning.values.clone()
        } else {
            return Err(TabularArrowError::InvalidValue);
        };
        if semantic.values.is_empty() {
            return Err(TabularArrowError::InvalidValue);
        }
        if spec.target == SemanticType::Ordinal
            && spec.domain.values.is_empty()
            && meaning.kind != SemanticType::Ordinal
        {
            return Err(TabularArrowError::InvalidValue);
        }
    }
    if spec.target == SemanticType::Datetime
        && !(source.data_type().is_temporal()
            || is_textual(source.data_type())
            || source.data_type() == &DataType::Null)
    {
        return Err(TabularArrowError::UnsupportedType);
    }
    if matches!(spec.target, SemanticType::Numeric | SemanticType::Binary)
        && source.data_type().is_temporal()
    {
        return Err(TabularArrowError::UnsupportedType);
    }
    if spec.target == SemanticType::Binary {
        semantic.values = ["false", "true"]
            .map(|value| SemanticValue {
                value: value.into(),
                label: value.into(),
            })
            .to_vec();
        semantic.positive_value = Some("true".into());
    }
    if spec.target == SemanticType::Numeric && spec.numeric == NumericRepresentation::Integer {
        semantic.numeric = Some(NumericConstraints {
            integer: true,
            ..Default::default()
        });
    }
    let field = with_column_semantic(
        Field::new(
            source.name(),
            dtype,
            source.is_nullable() || source.data_type() == &DataType::Null,
        ),
        &semantic,
    )?;
    Ok((field, semantic))
}

fn cast(array: &dyn Array, target: &DataType) -> Result<ArrayRef, TabularArrowError> {
    cast_with_options(
        array,
        target,
        &CastOptions {
            safe: false,
            ..Default::default()
        },
    )
    .map_err(|_| TabularArrowError::InvalidValue)
}

fn is_textual(dtype: &DataType) -> bool {
    match dtype {
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => true,
        DataType::Dictionary(_, value) => is_textual(value),
        _ => false,
    }
}

fn resolve_temporal(kind: DatetimeRepresentation, precision: TemporalPrecision) -> TemporalType {
    match kind {
        DatetimeRepresentation::Date => TemporalType::Date,
        DatetimeRepresentation::Time => TemporalType::Time(precision),
        DatetimeRepresentation::Auto | DatetimeRepresentation::Datetime => {
            TemporalType::Datetime(precision)
        }
    }
}

fn temporal_type(kind: TemporalType) -> DataType {
    let (time, precision) = match kind {
        TemporalType::Date => return DataType::Date32,
        TemporalType::Time(precision) => (true, precision),
        TemporalType::Datetime(precision) => (false, precision),
    };
    let unit = match precision {
        TemporalPrecision::Seconds => TimeUnit::Second,
        TemporalPrecision::Milliseconds => TimeUnit::Millisecond,
        TemporalPrecision::Microseconds => TimeUnit::Microsecond,
        TemporalPrecision::Nanoseconds => TimeUnit::Nanosecond,
    };
    if time {
        match unit {
            TimeUnit::Second | TimeUnit::Millisecond => DataType::Time32(unit),
            _ => DataType::Time64(unit),
        }
    } else {
        DataType::Timestamp(unit, None)
    }
}

fn temporal_metadata(dtype: &DataType) -> Option<TemporalType> {
    let (time, unit) = match dtype {
        DataType::Date32 | DataType::Date64 => return Some(TemporalType::Date),
        DataType::Time32(unit) | DataType::Time64(unit) => (true, unit),
        DataType::Timestamp(unit, _) => (false, unit),
        _ => return None,
    };
    let precision = match unit {
        TimeUnit::Second => TemporalPrecision::Seconds,
        TimeUnit::Millisecond => TemporalPrecision::Milliseconds,
        TimeUnit::Microsecond => TemporalPrecision::Microseconds,
        TimeUnit::Nanosecond => TemporalPrecision::Nanoseconds,
    };
    Some(if time {
        TemporalType::Time(precision)
    } else {
        TemporalType::Datetime(precision)
    })
}

fn parse_calendar(value: &str, format: &str) -> Result<String, TabularArrowError> {
    let value = value.trim();
    if let Ok(value) = chrono::DateTime::parse_from_str(value, format) {
        return Ok(value
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%S%.f")
            .to_string());
    }
    if let Ok(value) = chrono::NaiveDateTime::parse_from_str(value, format) {
        return Ok(value.format("%Y-%m-%dT%H:%M:%S%.f").to_string());
    }
    if let Ok(value) = chrono::NaiveDate::parse_from_str(value, format) {
        return Ok(value.format("%Y-%m-%d").to_string());
    }
    chrono::NaiveTime::parse_from_str(value, format)
        .map(|value| value.format("%H:%M:%S%.f").to_string())
        .map_err(|_| TabularArrowError::InvalidValue)
}

fn strings(array: &dyn Array) -> Result<StringArray, TabularArrowError> {
    cast(array, &DataType::Utf8)?
        .as_any()
        .downcast_ref::<StringArray>()
        .cloned()
        .ok_or(TabularArrowError::UnsupportedType)
}

fn finite(array: &dyn Array) -> Result<(), TabularArrowError> {
    if array.data_type().is_floating() {
        let values = cast(array, &DataType::Float64)?;
        let values = values
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or(TabularArrowError::InvalidValue)?;
        if values.iter().flatten().any(|value| !value.is_finite()) {
            return Err(TabularArrowError::InvalidValue);
        }
    }
    Ok(())
}

/// Compare decimal spellings after parsing: allow 001, 1.0 and 1e0 to mean the same
/// value, without allowing 9007199254740993 to round through a floating representation.
fn decimal_identity(value: &str) -> Option<(bool, String, i64)> {
    let value = value.trim();
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix(['-', '+']).unwrap_or(value);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (unsigned, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.len() + fraction.len() == 0
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0".into(), 0));
    }
    let significant = digits.trim_end_matches('0');
    let exponent = exponent
        .checked_sub(i64::try_from(fraction.len()).ok()?)?
        .checked_add(i64::try_from(digits.len() - significant.len()).ok()?)?;
    Some((negative, significant.into(), exponent))
}
