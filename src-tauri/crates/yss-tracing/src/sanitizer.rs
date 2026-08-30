use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::{LogFields, LogLimits};

const MAX_SANITIZED_ARRAY_ENTRIES: usize = 64;
const MAX_SANITIZED_OBJECT_ENTRIES: usize = 64;

pub const REDACTED_VALUE: &str = "[REDACTED]";
const TRUNCATED_VALUE: &str = "[TRUNCATED]";
const TRUNCATED_SUFFIX: &str = "…[truncated]";
const TRUNCATED_FIELD: &str = "_diagnosticsTruncated";

static SENSITIVE_HEADER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)\b(authorization|cookie|set-cookie)\s*:\s*[^\r\n]+")
        .expect("valid logging sensitive-header regex")
});
static LABELED_SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b["']?(password|passwd|pwd|(?:access|refresh|id|auth)[_\- ]?token|token|authorization|cookie|set[_\- ]?cookie|api[_\- ]?key|connection[_\- ]?string|database[_\- ]?url)["']?\s*[:=]\s*(?:bearer\s+)?(?:"[^"]*"|'[^']*'|[^\s,;]+)"#,
    )
    .expect("valid logging labeled-secret regex")
});
static BEARER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bbearer\s+[a-z0-9._~+/=\-]+").expect("valid logging bearer-token regex")
});
static URI_USERINFO_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://)[^/\s:@]+:[^@\s/]+@")
        .expect("valid logging URI-userinfo regex")
});
static PROHIBITED_CONTENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)\b(dataframe|rows|cell[_\- ]?values?|document|clipboard[_\- ]?(?:content|text|html))\s*[:=]\s*.+",
    )
    .expect("valid logging prohibited-content regex")
});

pub fn sanitize_target(value: &str) -> String {
    sanitize_text(value, LogLimits::MAX_TARGET_BYTES, false)
}

pub fn sanitize_event(value: &str) -> String {
    sanitize_text(value, LogLimits::MAX_EVENT_BYTES, false)
}

pub fn sanitize_message(value: &str) -> String {
    sanitize_content_text(value, LogLimits::MAX_MESSAGE_BYTES)
}

pub fn sanitize_source(value: &str) -> String {
    sanitize_text(value, LogLimits::MAX_SOURCE_BYTES, false)
}

pub(crate) fn sanitize_field_string(value: &str) -> String {
    sanitize_content_text(value, LogLimits::MAX_FIELD_STRING_BYTES)
}

fn sanitize_field_key(value: &str) -> String {
    sanitize_text(value, LogLimits::MAX_FIELD_KEY_BYTES, false)
}

pub(crate) fn should_redact_field(name: &str) -> bool {
    let normalized = normalize_key(name);
    if normalized.is_empty() {
        return false;
    }

    is_secret_key(&normalized) || is_prohibited_payload_key(&normalized)
}

pub(crate) fn redacted_json_value() -> Value {
    Value::String(REDACTED_VALUE.to_owned())
}

pub fn sanitize_fields(fields: LogFields) -> LogFields {
    let mut context = SanitizeContext::default();
    let mut sanitized = LogFields::new();
    let mut encoded_bytes = 2_usize;

    for (index, (raw_key, raw_value)) in fields.into_iter().enumerate() {
        if index >= LogLimits::MAX_FIELD_COUNT {
            context.truncated = true;
            break;
        }

        let key = sanitize_field_key(&raw_key);
        if key != raw_key {
            context.truncated = true;
        }
        if sanitized.contains_key(&key) {
            context.truncated = true;
            continue;
        }

        let value = sanitize_json_value(&raw_key, raw_value, 1, &mut context);
        let entry_bytes = encoded_entry_size(&key, &value);
        let separator_bytes = usize::from(!sanitized.is_empty());
        if encoded_bytes
            .saturating_add(separator_bytes)
            .saturating_add(entry_bytes)
            > LogLimits::MAX_FIELDS_BYTES
        {
            context.truncated = true;
            continue;
        }

        encoded_bytes += separator_bytes + entry_bytes;
        sanitized.insert(key, value);
    }

    if context.truncated && !sanitized.contains_key(TRUNCATED_FIELD) {
        let value = Value::Bool(true);
        let entry_bytes = encoded_entry_size(TRUNCATED_FIELD, &value);
        let separator_bytes = usize::from(!sanitized.is_empty());
        if encoded_bytes
            .saturating_add(separator_bytes)
            .saturating_add(entry_bytes)
            <= LogLimits::MAX_FIELDS_BYTES
        {
            sanitized.insert(TRUNCATED_FIELD.to_owned(), value);
        }
    }

    sanitized
}

#[derive(Default)]
struct SanitizeContext {
    values_seen: usize,
    truncated: bool,
}

fn sanitize_json_value(
    key: &str,
    value: Value,
    depth: usize,
    context: &mut SanitizeContext,
) -> Value {
    if should_redact_field(key) {
        return redacted_json_value();
    }
    if depth > LogLimits::MAX_FIELD_DEPTH || context.values_seen >= LogLimits::MAX_FIELD_VALUES {
        context.truncated = true;
        return Value::String(TRUNCATED_VALUE.to_owned());
    }
    context.values_seen += 1;

    match value {
        Value::String(value) => {
            let sanitized = sanitize_field_string(&value);
            if sanitized != value {
                context.truncated = true;
            }
            Value::String(sanitized)
        }
        Value::Array(values) => {
            if values.len() > MAX_SANITIZED_ARRAY_ENTRIES {
                context.truncated = true;
            }
            Value::Array(
                values
                    .into_iter()
                    .take(MAX_SANITIZED_ARRAY_ENTRIES)
                    .map(|value| sanitize_json_value("", value, depth + 1, context))
                    .collect(),
            )
        }
        Value::Object(values) => {
            if values.len() > MAX_SANITIZED_OBJECT_ENTRIES {
                context.truncated = true;
            }
            let mut sanitized = Map::new();
            for (raw_key, raw_value) in values.into_iter().take(MAX_SANITIZED_OBJECT_ENTRIES) {
                let key = sanitize_field_key(&raw_key);
                if key != raw_key || sanitized.contains_key(&key) {
                    context.truncated = true;
                }
                if sanitized.contains_key(&key) {
                    continue;
                }
                let value = sanitize_json_value(&raw_key, raw_value, depth + 1, context);
                sanitized.insert(key, value);
            }
            Value::Object(sanitized)
        }
        scalar @ (Value::Null | Value::Bool(_) | Value::Number(_)) => scalar,
    }
}

fn encoded_entry_size(key: &str, value: &Value) -> usize {
    let key_bytes = serde_json::to_vec(key).map_or(usize::MAX, |encoded| encoded.len());
    let value_bytes = serde_json::to_vec(value).map_or(usize::MAX, |encoded| encoded.len());
    key_bytes.saturating_add(1).saturating_add(value_bytes)
}

fn sanitize_content_text(value: &str, max_bytes: usize) -> String {
    let redacted = redact_sensitive_content(value);
    sanitize_text(redacted.as_ref(), max_bytes, true)
}

fn redact_sensitive_content(value: &str) -> Cow<'_, str> {
    let value = replace_if_matched(
        Cow::Borrowed(value),
        &SENSITIVE_HEADER_PATTERN,
        "$1: [REDACTED]",
    );
    let value = replace_if_matched(value, &LABELED_SECRET_PATTERN, "$1=[REDACTED]");
    let value = replace_if_matched(value, &BEARER_PATTERN, "Bearer [REDACTED]");
    let value = replace_if_matched(value, &URI_USERINFO_PATTERN, "$1[REDACTED]@");
    replace_if_matched(value, &PROHIBITED_CONTENT_PATTERN, "$1=[REDACTED]")
}

fn replace_if_matched<'a>(value: Cow<'a, str>, pattern: &Regex, replacement: &str) -> Cow<'a, str> {
    if pattern.is_match(value.as_ref()) {
        Cow::Owned(
            pattern
                .replace_all(value.as_ref(), replacement)
                .into_owned(),
        )
    } else {
        value
    }
}

fn sanitize_text(value: &str, max_bytes: usize, allow_line_breaks: bool) -> String {
    let mut sanitized = String::with_capacity(value.len().min(max_bytes));
    for character in value.chars() {
        let allowed_line_break = allow_line_breaks && matches!(character, '\n' | '\r' | '\t');
        let character = if character.is_control() && !allowed_line_break {
            ' '
        } else {
            character
        };
        if sanitized.len().saturating_add(character.len_utf8()) > max_bytes {
            append_truncated_suffix(&mut sanitized, max_bytes);
            return sanitized;
        }
        sanitized.push(character);
    }
    sanitized
}

fn append_truncated_suffix(value: &mut String, max_bytes: usize) {
    if max_bytes <= TRUNCATED_SUFFIX.len() {
        while value.len() > max_bytes {
            value.pop();
        }
        return;
    }

    let prefix_bytes = max_bytes - TRUNCATED_SUFFIX.len();
    while value.len() > prefix_bytes {
        value.pop();
    }
    value.push_str(TRUNCATED_SUFFIX);
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_secret_key(key: &str) -> bool {
    matches!(
        key,
        "password"
            | "passwd"
            | "pwd"
            | "token"
            | "authorization"
            | "cookie"
            | "cookies"
            | "setcookie"
            | "apikey"
            | "credentials"
            | "credential"
            | "privatekey"
            | "connectionstring"
            | "databaseurl"
            | "dburl"
    ) || key.contains("password")
        || key.contains("token")
        || key.contains("authorization")
        || key.contains("cookie")
        || key.contains("apikey")
        || key.contains("secret")
        || key.contains("connectionstring")
        || key.contains("databaseurl")
        || key.contains("privatekey")
}

fn is_prohibited_payload_key(key: &str) -> bool {
    matches!(
        key,
        "row"
            | "rows"
            | "rowvalues"
            | "dataframe"
            | "dataframerows"
            | "dataframedata"
            | "cellvalue"
            | "cellvalues"
            | "document"
            | "clipboard"
            | "clipboardcontent"
            | "clipboardtext"
            | "clipboardhtml"
    ) || (key.contains("dataframe") && key.contains("row"))
        || key.starts_with("cellvalue")
        || (key.starts_with("document")
            && ["content", "text", "body", "html", "data", "value"]
                .iter()
                .any(|suffix| key.ends_with(suffix)))
        || (key.starts_with("clipboard")
            && ["content", "text", "body", "html", "data", "value"]
                .iter()
                .any(|suffix| key.ends_with(suffix)))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn redacts_sensitive_and_prohibited_payload_content() {
        let fields = BTreeMap::from([
            ("API-Key".into(), json!("api-secret")),
            ("rows".into(), json!([["private-row"]])),
            (
                "nested".into(),
                json!({
                    "connection_string": "database-secret",
                    "clipboardContent": "private-clipboard",
                    "documentId": "safe-id"
                }),
            ),
        ]);

        let sanitized = sanitize_fields(fields);
        assert_eq!(sanitized["API-Key"], REDACTED_VALUE);
        assert_eq!(sanitized["rows"], REDACTED_VALUE);
        assert_eq!(sanitized["nested"]["connection_string"], REDACTED_VALUE);
        assert_eq!(sanitized["nested"]["clipboardContent"], REDACTED_VALUE);
        assert_eq!(sanitized["nested"]["documentId"], "safe-id");

        let message = sanitize_message(
            "Authorization: Bearer header-secret\npassword=hunter2 postgres://user:db-secret@host rows: [[private-cell]]",
        );
        assert!(message.contains(REDACTED_VALUE));
        assert!(!message.contains("header-secret"));
        assert!(!message.contains("hunter2"));
        assert!(!message.contains("db-secret"));
        assert!(!message.contains("private-cell"));
    }

    #[test]
    fn bounds_text_and_json_shape() {
        let long = "x".repeat(LogLimits::MAX_FIELD_STRING_BYTES + 100);
        let fields = BTreeMap::from([
            ("long".into(), json!(long)),
            (
                "array".into(),
                Value::Array(
                    (0..MAX_SANITIZED_ARRAY_ENTRIES + 10)
                        .map(|value| json!(value))
                        .collect(),
                ),
            ),
        ]);

        let sanitized = sanitize_fields(fields);
        assert!(sanitized["long"].as_str().unwrap().len() <= LogLimits::MAX_FIELD_STRING_BYTES);
        assert_eq!(
            sanitized["array"].as_array().unwrap().len(),
            MAX_SANITIZED_ARRAY_ENTRIES
        );
        assert_eq!(sanitized[TRUNCATED_FIELD], true);
        assert!(serde_json::to_vec(&sanitized).unwrap().len() <= LogLimits::MAX_FIELDS_BYTES);
    }
}
