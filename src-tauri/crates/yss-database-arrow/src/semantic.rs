use std::collections::BTreeSet;

use arrow::array::{Array, ArrayRef, StringArray};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, Field};
pub use yss_data_contract::{ColumnSemantic, NumericConstraints, SemanticType, SemanticValue};

use crate::{CategoryDomain, TabularArrowError};

const SEMANTIC: &str = "yssbi.semantic";

/// Legacy/unannotated fields receive a conservative initial meaning. Cardinality is never
/// consulted for strings; an explicit dictionary domain is the only category inference.
pub fn column_semantic(field: &Field) -> Result<ColumnSemantic, TabularArrowError> {
    if let Some(encoded) = field.metadata().get(SEMANTIC) {
        let semantic =
            serde_json::from_str(encoded).map_err(|_| TabularArrowError::InvalidSchema)?;
        validate_semantic(field, &semantic)?;
        return Ok(semantic);
    }
    let mut semantic = ColumnSemantic::new(match field.data_type() {
        dtype if dtype.is_numeric() => SemanticType::Numeric,
        DataType::Boolean => SemanticType::Binary,
        DataType::Date32
        | DataType::Date64
        | DataType::Timestamp(..)
        | DataType::Time32(..)
        | DataType::Time64(..) => SemanticType::Datetime,
        DataType::Dictionary(..) => SemanticType::Categorical,
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => SemanticType::Text,
        _ => SemanticType::Identifier,
    });
    if field.data_type() == &DataType::Boolean {
        semantic.values = ["false", "true"]
            .map(|value| SemanticValue {
                value: value.into(),
                label: value.into(),
            })
            .to_vec();
    }
    if let Some(domain) = CategoryDomain::from_field(field)? {
        semantic.kind = if domain.ordered {
            SemanticType::Ordinal
        } else {
            SemanticType::Categorical
        };
        semantic.values = domain
            .labels
            .into_iter()
            .map(|value| SemanticValue {
                label: value.clone(),
                value,
            })
            .collect();
    }
    Ok(semantic)
}

pub fn with_column_semantic(
    field: Field,
    semantic: &ColumnSemantic,
) -> Result<Field, TabularArrowError> {
    validate_semantic(&field, semantic)?;
    let mut metadata = field.metadata().clone();
    metadata.insert(
        SEMANTIC.into(),
        serde_json::to_string(semantic).map_err(|_| TabularArrowError::InvalidSchema)?,
    );
    Ok(field.with_metadata(metadata))
}

pub fn is_numeric_field(field: &Field) -> bool {
    column_semantic(field).is_ok_and(|semantic| semantic.kind == SemanticType::Numeric)
        && field.data_type().is_numeric()
}

fn strings(array: &dyn Array) -> Result<StringArray, TabularArrowError> {
    cast_with_options(
        array,
        &DataType::Utf8,
        &CastOptions {
            safe: false,
            ..Default::default()
        },
    )
    .map_err(|_| TabularArrowError::InvalidValue)?
    .as_any()
    .downcast_ref::<StringArray>()
    .cloned()
    .ok_or(TabularArrowError::UnsupportedType)
}

fn value_array(field: &Field, value: &str) -> Result<ArrayRef, TabularArrowError> {
    // Avoid recursing through semantic validation while decoding the metadata itself.
    let mut metadata = field.metadata().clone();
    metadata.remove(SEMANTIC);
    crate::json_to_array(
        &field.clone().with_metadata(metadata),
        &[serde_json::Value::String(value.into())],
    )
}

fn validate_semantic(field: &Field, semantic: &ColumnSemantic) -> Result<(), TabularArrowError> {
    use SemanticType::*;
    let dtype = field.data_type();
    let valid = match semantic.kind {
        Numeric => dtype.is_numeric(),
        Datetime => matches!(
            dtype,
            DataType::Date32
                | DataType::Date64
                | DataType::Timestamp(_, None)
                | DataType::Time32(_)
                | DataType::Time64(_)
        ),
        Text => matches!(
            dtype,
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View | DataType::Dictionary(..)
        ),
        Categorical | Ordinal | Binary | Identifier => true,
    };
    if !valid
        || (semantic.kind != Numeric && semantic.numeric.is_some())
        || (semantic.kind != Binary && semantic.positive_value.is_some())
        || (!matches!(semantic.kind, Categorical | Ordinal | Binary) && !semantic.values.is_empty())
        || (semantic.kind == Binary && semantic.values.len() != 2)
        || semantic.values.len() > 65_536
    {
        return Err(TabularArrowError::InvalidValue);
    }
    let mut seen = BTreeSet::new();
    for value in &semantic.values {
        let array = value_array(field, &value.value)?;
        let canonical = strings(array.as_ref())?;
        if canonical.is_null(0)
            || canonical.value(0) != value.value
            || !seen.insert(value.value.as_str())
        {
            return Err(TabularArrowError::InvalidValue);
        }
    }
    if semantic
        .positive_value
        .as_ref()
        .is_some_and(|positive| !seen.contains(positive.as_str()))
    {
        return Err(TabularArrowError::InvalidValue);
    }
    if let Some(constraints) = &semantic.numeric {
        let lower = constraints
            .minimum
            .as_ref()
            .map(|value| value_array(field, value))
            .transpose()?;
        let upper = constraints
            .maximum
            .as_ref()
            .map(|value| value_array(field, value))
            .transpose()?;
        if let (Some(lower), Some(upper)) = (lower, upper) {
            let invalid = arrow::compute::kernels::cmp::gt(&lower, &upper)
                .map_err(|_| TabularArrowError::InvalidValue)?;
            if invalid.value(0) {
                return Err(TabularArrowError::InvalidValue);
            }
        }
    }
    Ok(())
}

/// Validate actual values at import/edit/semantic-change/cast boundaries. Nulls are missing
/// observations, so they never add a category or participate in a binary value mapping.
pub fn validate_semantic_array(field: &Field, array: &dyn Array) -> Result<(), TabularArrowError> {
    if !field.metadata().contains_key(SEMANTIC) {
        return Ok(());
    }
    let semantic = column_semantic(field)?;
    PreparedSemanticValidator::new(field, &semantic)?.validate(array)
}

/// Definitions and bounds are checked once; only actual values are checked per batch.
#[derive(Debug, Default)]
pub(crate) struct PreparedSemanticValidator {
    allowed: Option<std::collections::HashSet<String>>,
    integer: bool,
    bounds: Vec<(ArrayRef, bool)>,
}

impl PreparedSemanticValidator {
    pub(crate) fn new(field: &Field, semantic: &ColumnSemantic) -> Result<Self, TabularArrowError> {
        // Keep unannotated dictionary import behavior: its domain is installed after streaming.
        if !field.metadata().contains_key(SEMANTIC) {
            return Ok(Self::default());
        }
        let allowed = matches!(
            semantic.kind,
            SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
        )
        .then(|| {
            semantic
                .values
                .iter()
                .map(|value| value.value.clone())
                .collect()
        });
        let mut bounds = Vec::new();
        let mut integer = false;
        if let Some(constraints) = &semantic.numeric {
            integer = constraints.integer;
            for (bound, minimum) in [
                (constraints.minimum.as_ref(), true),
                (constraints.maximum.as_ref(), false),
            ] {
                if let Some(bound) = bound {
                    bounds.push((value_array(field, bound)?, minimum));
                }
            }
        }
        Ok(Self {
            allowed,
            integer,
            bounds,
        })
    }

    pub(crate) fn validate(&self, array: &dyn Array) -> Result<(), TabularArrowError> {
        if let Some(allowed) = &self.allowed
            && strings(array)?
                .iter()
                .flatten()
                .any(|value| !allowed.contains(value))
        {
            return Err(TabularArrowError::InvalidValue);
        }
        if self.integer && array.data_type().is_floating() {
            let numbers = cast_with_options(
                array,
                &DataType::Float64,
                &CastOptions {
                    safe: false,
                    ..Default::default()
                },
            )
            .map_err(|_| TabularArrowError::InvalidValue)?;
            let numbers = numbers
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or(TabularArrowError::InvalidValue)?;
            if numbers
                .iter()
                .flatten()
                .any(|value| !value.is_finite() || value.fract() != 0.0)
            {
                return Err(TabularArrowError::InvalidValue);
            }
        } else if self.integer && !array.data_type().is_integer() {
            // A decimal representation retains exact text; no f64 rounding is used to decide
            // whether a wide decimal has a fractional component.
            for value in strings(array)?.iter().flatten() {
                if value.contains(['e', 'E'])
                    || value
                        .split_once('.')
                        .is_some_and(|(_, fraction)| fraction.chars().any(|digit| digit != '0'))
                {
                    return Err(TabularArrowError::InvalidValue);
                }
            }
        }
        if !self.bounds.is_empty() {
            let data = arrow::array::make_array(array.to_data());
            for (bound, minimum) in &self.bounds {
                let bound = arrow::array::Scalar::new(bound.clone());
                let invalid = if *minimum {
                    arrow::compute::kernels::cmp::lt(&data, &bound)
                } else {
                    arrow::compute::kernels::cmp::gt(&data, &bound)
                }
                .map_err(|_| TabularArrowError::InvalidValue)?;
                if invalid.iter().flatten().any(|invalid| invalid) {
                    return Err(TabularArrowError::InvalidValue);
                }
            }
        }
        Ok(())
    }
}

/// A physical cast keeps both category identities and explicit ordinal order. A cast that
/// merges two codes or loses representation/precision is rejected by the cast boundary.
pub fn cast_column_semantic(
    source: &Field,
    target: &Field,
) -> Result<ColumnSemantic, TabularArrowError> {
    let mut semantic = column_semantic(source)?;
    let positive = semantic.positive_value.clone();
    for value in &mut semantic.values {
        let original = value.value.clone();
        let array = value_array(source, &original)?;
        let converted = lossless_cast(array.as_ref(), target.data_type(), false)?;
        value.value = strings(converted.as_ref())?.value(0).to_owned();
        if positive.as_deref() == Some(&original) {
            semantic.positive_value = Some(value.value.clone());
        }
    }
    if let Some(numeric) = &mut semantic.numeric {
        for bound in [&mut numeric.minimum, &mut numeric.maximum]
            .into_iter()
            .flatten()
        {
            let array = value_array(source, bound)?;
            let converted = lossless_cast(array.as_ref(), target.data_type(), false)?;
            *bound = strings(converted.as_ref())?.value(0).to_owned();
        }
    }
    validate_semantic(target, &semantic)?;
    Ok(semantic)
}

/// Strict casts also reject successful Arrow casts that truncate fractional values, narrow
/// time precision, overflow, or merge textual identifiers such as "001" and "1".
pub fn lossless_cast(
    array: &dyn Array,
    target: &DataType,
    force: bool,
) -> Result<ArrayRef, TabularArrowError> {
    if array.data_type() == target {
        return Ok(arrow::array::make_array(array.to_data()));
    }
    let options = CastOptions {
        safe: force,
        ..Default::default()
    };
    let converted = if target.is_temporal() {
        crate::cast_temporal_without_timezone(array, target, force)?
    } else {
        cast_with_options(array, target, &options).map_err(|_| TabularArrowError::InvalidValue)?
    };
    // Parsing calendar text deliberately removes source zones without shifting wall time.
    // Validity is checked by the temporal parser; exact temporal-to-temporal casts still
    // round-trip so changing the unit cannot silently lose precision.
    let calendar_text = target.is_temporal()
        && matches!(
            array.data_type(),
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
        );
    if calendar_text {
        let original = strings(array)?;
        let converted_text = strings(converted.as_ref())?;
        for row in 0..array.len() {
            if array.is_null(row) || (force && converted.is_null(row)) {
                continue;
            }
            if converted.is_null(row)
                || calendar_identity(crate::timezone_free_text(original.value(row))?)?
                    != calendar_identity(converted_text.value(row))?
            {
                return Err(TabularArrowError::InvalidValue);
            }
        }
    } else {
        let restored = cast_with_options(converted.as_ref(), array.data_type(), &options)
            .map_err(|_| TabularArrowError::InvalidValue)?;
        let original = strings(array)?;
        let restored = strings(restored.as_ref())?;
        for row in 0..array.len() {
            if array.is_null(row) || (force && converted.is_null(row)) {
                continue;
            }
            if restored.is_null(row) || original.value(row) != restored.value(row) {
                return Err(TabularArrowError::InvalidValue);
            }
        }
    }
    Ok(converted)
}

fn calendar_identity(value: &str) -> Result<String, TabularArrowError> {
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(value) = chrono::NaiveDateTime::parse_from_str(value, format) {
            return Ok(value.to_string());
        }
    }
    if let Ok(value) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return value
            .and_hms_opt(0, 0, 0)
            .map(|value| value.to_string())
            .ok_or(TabularArrowError::InvalidValue);
    }
    chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
        .map(|value| value.to_string())
        .map_err(|_| TabularArrowError::InvalidValue)
}
