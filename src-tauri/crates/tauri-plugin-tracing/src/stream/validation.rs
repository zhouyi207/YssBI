use crate::collector::{
    LogLimits, sanitize_event, sanitize_fields, sanitize_message, sanitize_source, sanitize_target,
};
use serde_json::Value;
use thiserror::Error;

use super::dto::{FrontendLogEntryDto, LogDomain, LogFields, LogLevel};

// This transport-level batch policy is independent from per-record collection limits.
pub(super) const MAX_FRONTEND_LOG_BATCH: usize = 256;

#[derive(Debug, Clone)]
pub(crate) struct ValidatedFrontendLog {
    pub level: LogLevel,
    pub domain: LogDomain,
    pub target: String,
    pub event: Option<String>,
    pub message: String,
    pub source: Option<String>,
    pub fields: LogFields,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct FrontendLogValidationError {
    message: String,
}

impl FrontendLogValidationError {
    fn batch(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn entry(index: usize, message: impl Into<String>) -> Self {
        Self::batch(format!(
            "Frontend log entry {index} is invalid: {}",
            message.into()
        ))
    }
}

pub(crate) fn validate_frontend_batch(
    entries: Vec<FrontendLogEntryDto>,
) -> Result<Vec<ValidatedFrontendLog>, FrontendLogValidationError> {
    if entries.is_empty() {
        return Err(FrontendLogValidationError::batch(
            "Frontend log batch must contain at least one entry",
        ));
    }
    if entries.len() > MAX_FRONTEND_LOG_BATCH {
        return Err(FrontendLogValidationError::batch(format!(
            "Frontend log batch exceeds the {MAX_FRONTEND_LOG_BATCH} entry limit"
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
    entry: FrontendLogEntryDto,
) -> Result<ValidatedFrontendLog, FrontendLogValidationError> {
    validate_required_text(
        index,
        "target",
        &entry.target,
        LogLimits::MAX_TARGET_BYTES,
        false,
    )?;
    validate_required_text(
        index,
        "message",
        &entry.message,
        LogLimits::MAX_MESSAGE_BYTES,
        true,
    )?;
    validate_optional_text(
        index,
        "event",
        entry.event.as_deref(),
        LogLimits::MAX_EVENT_BYTES,
        false,
    )?;
    validate_optional_text(
        index,
        "source",
        entry.source.as_deref(),
        LogLimits::MAX_SOURCE_BYTES,
        false,
    )?;
    validate_fields(index, &entry.fields)?;

    Ok(ValidatedFrontendLog {
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
) -> Result<(), FrontendLogValidationError> {
    if value.trim().is_empty() {
        return Err(FrontendLogValidationError::entry(
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
) -> Result<(), FrontendLogValidationError> {
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
) -> Result<(), FrontendLogValidationError> {
    if value.len() > max_bytes {
        return Err(FrontendLogValidationError::entry(
            index,
            format!("{field} exceeds the {max_bytes} byte limit"),
        ));
    }
    if value.chars().any(|character| {
        character.is_control() && !(allow_line_breaks && matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(FrontendLogValidationError::entry(
            index,
            format!("{field} contains control characters"),
        ));
    }
    Ok(())
}

fn validate_fields(index: usize, fields: &LogFields) -> Result<(), FrontendLogValidationError> {
    if fields.len() > LogLimits::MAX_FIELD_COUNT {
        return Err(FrontendLogValidationError::entry(
            index,
            format!(
                "fields exceeds the {} field limit",
                LogLimits::MAX_FIELD_COUNT
            ),
        ));
    }
    let encoded = serde_json::to_vec(fields).map_err(|error| {
        FrontendLogValidationError::entry(index, format!("fields is not JSON: {error}"))
    })?;
    if encoded.len() > LogLimits::MAX_FIELDS_BYTES {
        return Err(FrontendLogValidationError::entry(
            index,
            format!(
                "fields exceeds the {} byte limit",
                LogLimits::MAX_FIELDS_BYTES
            ),
        ));
    }

    let mut value_count = 0;
    for (key, value) in fields {
        validate_required_text(
            index,
            "field name",
            key,
            LogLimits::MAX_FIELD_KEY_BYTES,
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
) -> Result<(), FrontendLogValidationError> {
    *value_count += 1;
    if *value_count > LogLimits::MAX_FIELD_VALUES {
        return Err(FrontendLogValidationError::entry(
            index,
            format!(
                "fields exceeds the {} value limit",
                LogLimits::MAX_FIELD_VALUES
            ),
        ));
    }
    if depth > LogLimits::MAX_FIELD_DEPTH {
        return Err(FrontendLogValidationError::entry(
            index,
            format!(
                "fields exceeds the maximum nesting depth of {}",
                LogLimits::MAX_FIELD_DEPTH
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
                    LogLimits::MAX_FIELD_KEY_BYTES,
                    false,
                )?;
                validate_field_value(index, value, depth + 1, value_count)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}
