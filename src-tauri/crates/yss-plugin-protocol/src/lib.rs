mod host;
mod manifest;
mod operation;
pub use host::*;
pub use manifest::*;
pub use operation::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read, Write};

pub const PROTOCOL_MAJOR: u32 = 2;
pub const PROTOCOL_MINOR: u32 = 0;
pub const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceBudget {
    pub frame_bytes: u32,
    pub pending_requests: u32,
    pub active_tasks: u32,
    pub queued_bytes: u32,
    pub snapshot_bytes: u64,
    pub private_storage_bytes: u64,
    pub views: u32,
}
impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            frame_bytes: MAX_FRAME_BYTES as u32,
            pending_requests: 32,
            active_tasks: 4,
            queued_bytes: 8 * 1024 * 1024,
            snapshot_bytes: 128 * 1024 * 1024,
            private_storage_bytes: 1024 * 1024 * 1024,
            views: 4,
        }
    }
}
impl ResourceBudget {
    pub fn grant(&self) -> Result<Self, PluginFailure> {
        self.validate()?;
        Ok(self.clone())
    }
    pub fn validate(&self) -> Result<(), PluginFailure> {
        let max = Self::default();
        if self.frame_bytes == 0
            || self.frame_bytes > max.frame_bytes
            || self.pending_requests == 0
            || self.pending_requests > max.pending_requests
            || self.active_tasks == 0
            || self.active_tasks > max.active_tasks
            || self.queued_bytes < self.frame_bytes
            || self.queued_bytes > max.queued_bytes
            || self.snapshot_bytes == 0
            || self.snapshot_bytes > max.snapshot_bytes
            || self.private_storage_bytes == 0
            || self.private_storage_bytes > max.private_storage_bytes
            || self.views == 0
            || self.views > max.views
        {
            return Err(PluginFailure::new("plugin_budget_invalid"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: PluginFailure,
}
fn present_value<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[error("{code}")]
pub struct PluginFailure {
    pub code: String,
    pub details: Option<Value>,
    pub incident_id: Option<String>,
}
impl PluginFailure {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_owned(),
            details: None,
            incident_id: None,
        }
    }
}
impl RpcResponse {
    pub fn from_result(id: String, result: Result<Value, PluginFailure>) -> Self {
        match result {
            Ok(value) => Self {
                jsonrpc: "2.0".into(),
                id,
                result: Some(value),
                error: None,
            },
            Err(error) => Self {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(RpcError {
                    code: -32000,
                    message: "plugin_error".into(),
                    data: error,
                }),
            },
        }
    }
}
pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0u8; 4];
    match reader.read_exact(&mut prefix[..1]) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    reader.read_exact(&mut prefix[1..])?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut data = vec![0; size];
    reader.read_exact(&mut data)?;
    Ok(Some(data))
}
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    let data = serde_json::to_vec(value)?;
    if data.is_empty() || data.len() > MAX_FRAME_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    writer.write_all(&(data.len() as u32).to_be_bytes())?;
    writer.write_all(&data)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_preserve_null_success_and_reject_oversized_lengths_before_payload_read() {
        let response = RpcResponse::from_result("request".into(), Ok(Value::Null));
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &response).unwrap();
        let frame = read_frame(&mut std::io::Cursor::new(bytes))
            .unwrap()
            .unwrap();
        let decoded: RpcResponse = serde_json::from_slice(&frame).unwrap();
        assert_eq!(decoded.result, Some(Value::Null));
        assert!(decoded.error.is_none());
        let mut input = std::io::Cursor::new(((MAX_FRAME_BYTES + 1) as u32).to_be_bytes().to_vec());
        assert_eq!(
            read_frame(&mut input).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(input.position(), 4);
    }
}
