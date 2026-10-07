use crate::{PluginFailure, valid_id};

pub const OPERATION_RETENTION_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Callers create this once for a logical operation and reuse it for retries.
pub fn operation_id(created_at_ms: u64, nonce: &str) -> Result<String, PluginFailure> {
    let id = format!("op-{created_at_ms}-{nonce}");
    if nonce.is_empty() || !valid_id(&id) {
        return Err(PluginFailure::new("plugin_operation_invalid"));
    }
    Ok(id)
}

/// A forgotten receipt cannot turn an expired operation into a new execution.
pub fn validate_operation_id(id: &str, now_ms: u64) -> Result<(), PluginFailure> {
    let mut parts = id.splitn(3, '-');
    if !valid_id(id) || parts.next() != Some("op") {
        return Err(PluginFailure::new("plugin_operation_invalid"));
    }
    let created = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| PluginFailure::new("plugin_operation_invalid"))?;
    if parts.next().is_none_or(str::is_empty) || created > now_ms.saturating_add(60_000) {
        return Err(PluginFailure::new("plugin_operation_invalid"));
    }
    if now_ms.saturating_sub(created) >= OPERATION_RETENTION_MS {
        return Err(PluginFailure::new("plugin_operation_expired"));
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn operation_window_rejects_expired_future_and_opaque_replays() {
    let created = 10_000_000_000;
    let id = operation_id(created, "same-logical-request").unwrap();
    assert!(validate_operation_id(&id, created + OPERATION_RETENTION_MS - 1).is_ok());
    assert_eq!(
        validate_operation_id(&id, created + OPERATION_RETENTION_MS)
            .unwrap_err()
            .code,
        "plugin_operation_expired"
    );
    assert_eq!(
        validate_operation_id(&id, created - 60_001)
            .unwrap_err()
            .code,
        "plugin_operation_invalid"
    );
    assert_eq!(
        validate_operation_id("old-opaque-request", created)
            .unwrap_err()
            .code,
        "plugin_operation_invalid"
    );
}
