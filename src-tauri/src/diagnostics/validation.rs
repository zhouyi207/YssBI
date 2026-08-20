use serde_json::Value;
use thiserror::Error;

use super::dto::{DiagnosticDomain, DiagnosticFields, DiagnosticLevel, FrontendDiagnosticEntryDto};
use super::limits::DiagnosticLimits;
use super::sanitizer::{
    sanitize_event, sanitize_fields, sanitize_message, sanitize_source, sanitize_target,
};

// This transport-level batch policy is independent from per-record collection limits.
pub(super) const MAX_FRONTEND_DIAGNOSTIC_BATCH: usize = 256;

#[derive(Debug, Clone)]
pub(crate) struct ValidatedFrontendDiagnostic {
    pub level: DiagnosticLevel,
    pub domain: DiagnosticDomain,
    pub target: String,
    pub event: Option<String>,
    pub message: String,
    pub source: Option<String>,
    pub fields: DiagnosticFields,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct FrontendDiagnosticValidationError {
    message: String,
}

impl FrontendDiagnosticValidationError {
    fn batch(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn entry(index: usize, message: impl Into<String>) -> Self {
        Self::batch(format!(
            "Frontend diagnostic entry {index} is invalid: {}",
            message.into()
        ))
    }
}

pub(crate) fn validate_frontend_batch(
    entries: Vec<FrontendDiagnosticEntryDto>,
) -> Result<Vec<ValidatedFrontendDiagnostic>, FrontendDiagnosticValidationError> {
    if entries.is_empty() {
        return Err(FrontendDiagnosticValidationError::batch(
            "Frontend diagnostic batch must contain at least one entry",
        ));
    }
    if entries.len() > MAX_FRONTEND_DIAGNOSTIC_BATCH {
        return Err(FrontendDiagnosticValidationError::batch(format!(
            "Frontend diagnostic batch exceeds the {MAX_FRONTEND_DIAGNOSTIC_BATCH} entry limit"
        )));
    }

    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| validate_entry(index, entry))
        .collect()
}

fn validate_entry(
    index: usize,
    entry: FrontendDiagnosticEntryDto,
) -> Result<ValidatedFrontendDiagnostic, FrontendDiagnosticValidationError> {
    validate_required_text(
        index,
        "target",
        &entry.target,
        DiagnosticLimits::MAX_TARGET_BYTES,
        false,
    )?;
    validate_required_text(
        index,
        "message",
        &entry.message,
        DiagnosticLimits::MAX_MESSAGE_BYTES,
        true,
    )?;
    validate_optional_text(
        index,
        "event",
        entry.event.as_deref(),
        DiagnosticLimits::MAX_EVENT_BYTES,
        false,
    )?;
    validate_optional_text(
        index,
        "source",
        entry.source.as_deref(),
        DiagnosticLimits::MAX_SOURCE_BYTES,
        false,
    )?;
    validate_fields(index, &entry.fields)?;

    Ok(ValidatedFrontendDiagnostic {
        level: entry.level,
        domain: entry.domain,
        target: sanitize_target(&entry.target),
        event: entry.event.as_deref().map(sanitize_event),
        message: sanitize_message(&entry.message),
        source: entry.source.as_deref().map(sanitize_source),
        fields: sanitize_fields(entry.fields),
    })
}

fn validate_required_text(
    index: usize,
    field: &str,
    value: &str,
    max_bytes: usize,
    allow_line_breaks: bool,
) -> Result<(), FrontendDiagnosticValidationError> {
    if value.trim().is_empty() {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!("{field} must not be empty"),
        ));
    }
    validate_text(index, field, value, max_bytes, allow_line_breaks)
}

fn validate_optional_text(
    index: usize,
    field: &str,
    value: Option<&str>,
    max_bytes: usize,
    allow_line_breaks: bool,
) -> Result<(), FrontendDiagnosticValidationError> {
    let Some(value) = value else {
        return Ok(());
    };
    validate_required_text(index, field, value, max_bytes, allow_line_breaks)
}

fn validate_text(
    index: usize,
    field: &str,
    value: &str,
    max_bytes: usize,
    allow_line_breaks: bool,
) -> Result<(), FrontendDiagnosticValidationError> {
    if value.len() > max_bytes {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!("{field} exceeds the {max_bytes} byte limit"),
        ));
    }
    if value.chars().any(|character| {
        character.is_control() && !(allow_line_breaks && matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!("{field} contains control characters"),
        ));
    }
    Ok(())
}

fn validate_fields(
    index: usize,
    fields: &DiagnosticFields,
) -> Result<(), FrontendDiagnosticValidationError> {
    if fields.len() > DiagnosticLimits::MAX_FIELD_COUNT {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!(
                "fields exceeds the {} field limit",
                DiagnosticLimits::MAX_FIELD_COUNT
            ),
        ));
    }
    let encoded = serde_json::to_vec(fields).map_err(|error| {
        FrontendDiagnosticValidationError::entry(index, format!("fields is not JSON: {error}"))
    })?;
    if encoded.len() > DiagnosticLimits::MAX_FIELDS_BYTES {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!(
                "fields exceeds the {} byte limit",
                DiagnosticLimits::MAX_FIELDS_BYTES
            ),
        ));
    }

    let mut value_count = 0;
    for (key, value) in fields {
        validate_required_text(
            index,
            "field name",
            key,
            DiagnosticLimits::MAX_FIELD_KEY_BYTES,
            false,
        )?;
        validate_field_value(index, value, 1, &mut value_count)?;
    }
    Ok(())
}

fn validate_field_value(
    index: usize,
    value: &Value,
    depth: usize,
    value_count: &mut usize,
) -> Result<(), FrontendDiagnosticValidationError> {
    *value_count += 1;
    if *value_count > DiagnosticLimits::MAX_FIELD_VALUES {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!(
                "fields exceeds the {} value limit",
                DiagnosticLimits::MAX_FIELD_VALUES
            ),
        ));
    }
    if depth > DiagnosticLimits::MAX_FIELD_DEPTH {
        return Err(FrontendDiagnosticValidationError::entry(
            index,
            format!(
                "fields exceeds the maximum nesting depth of {}",
                DiagnosticLimits::MAX_FIELD_DEPTH
            ),
        ));
    }

    match value {
        Value::Array(values) => {
            for value in values {
                validate_field_value(index, value, depth + 1, value_count)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_required_text(
                    index,
                    "nested field name",
                    key,
                    DiagnosticLimits::MAX_FIELD_KEY_BYTES,
                    false,
                )?;
                validate_field_value(index, value, depth + 1, value_count)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}
