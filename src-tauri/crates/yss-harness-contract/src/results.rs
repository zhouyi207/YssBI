//! Business result references encode the original owner identity without a second registry.
use crate::{CapabilityContractError, GraphEditPortRef, GraphResourceRef, InspectionPage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Copy this opaque reference from a result receipt; do not construct its contents.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct ResultRef {
    execution_session_id: String,
    result_id: u64,
}

impl ResultRef {
    pub fn new(execution_session_id: String, result_id: u64) -> Self {
        Self {
            execution_session_id,
            result_id,
        }
    }
    pub fn execution_session_id(&self) -> &str {
        &self.execution_session_id
    }
    pub fn result_id(&self) -> u64 {
        self.result_id
    }
}

impl From<ResultRef> for String {
    fn from(value: ResultRef) -> Self {
        format!(
            "r.{}.{:016x}",
            encode(value.execution_session_id.as_bytes()),
            value.result_id
        )
    }
}

impl TryFrom<String> for ResultRef {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut parts = value.split('.');
        if parts.next() != Some("r") {
            return Err("invalid resultRef");
        }
        let session = parts.next().ok_or("invalid resultRef")?;
        if session.is_empty() || session.len() > 256 {
            return Err("invalid resultRef");
        }
        let execution_session_id = decode(session).ok_or("invalid resultRef")?;
        let id = parts.next().ok_or("invalid resultRef")?;
        if id.len() != 16 || parts.next().is_some() {
            return Err("invalid resultRef");
        }
        let result_id = u64::from_str_radix(id, 16).map_err(|_| "invalid resultRef")?;
        let reference = Self::new(execution_session_id, result_id);
        if String::from(reference.clone()) != value {
            return Err("invalid resultRef");
        }
        Ok(reference)
    }
}

/// Complete table reference, including its retained result and exact table selection.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct TableRef {
    result_ref: ResultRef,
    part: Option<String>,
}

impl TableRef {
    pub fn new(result_ref: ResultRef, part: Option<String>) -> Self {
        Self { result_ref, part }
    }
    pub fn result_ref(&self) -> &ResultRef {
        &self.result_ref
    }
    pub fn part(&self) -> Option<&str> {
        self.part.as_deref()
    }
}

impl From<TableRef> for String {
    fn from(value: TableRef) -> Self {
        format!(
            "t.{}.{}",
            String::from(value.result_ref),
            value
                .part
                .map_or_else(|| "_".into(), |part| encode(part.as_bytes()))
        )
    }
}

impl TryFrom<String> for TableRef {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (reference, part) = value
            .strip_prefix("t.")
            .and_then(|value| value.rsplit_once('.'))
            .ok_or("invalid tableRef")?;
        let result_ref = ResultRef::try_from(reference.to_owned())?;
        let part = if part == "_" {
            None
        } else {
            if part.is_empty() || part.len() > 8220 {
                return Err("invalid tableRef");
            }
            Some(decode(part).ok_or("invalid tableRef")?)
        };
        let reference = Self::new(result_ref, part);
        if String::from(reference.clone()) != value {
            return Err("invalid tableRef");
        }
        Ok(reference)
    }
}

fn encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("string writer");
    }
    encoded
}
fn decode(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) || !value.is_ascii() {
        return None;
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectResultRequest {
    pub result_ref: ResultRef,
    /// Page the schema of a tabular result without reading rows.
    #[serde(default)]
    pub schema_offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub schema_limit: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadResultTableRequest {
    pub table_ref: TableRef,
    /// Exact column names; omitted means the next schema page. Unknown columns fail.
    #[serde(default)]
    #[schemars(length(max = 100))]
    pub columns: Vec<String>,
    #[serde(default)]
    pub column_offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub column_limit: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_row_limit")]
    #[schemars(range(min = 1, max = 1000))]
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListGraphResultsRequest {
    pub graph: GraphResourceRef,
    /// Omit for current pins (including stale values); select a run for its retained results.
    #[serde(default)]
    pub run_id: Option<u64>,
    #[serde(default)]
    #[schemars(length(max = 200))]
    pub node_ids: Vec<String>,
    #[serde(default)]
    #[schemars(length(max = 200))]
    pub outputs: Vec<GraphEditPortRef>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}
fn default_row_limit() -> usize {
    20
}

impl InspectResultRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.schema_limit == 0 || self.schema_limit > 100 {
            return Err(CapabilityContractError::InvalidField("schemaLimit"));
        }
        Ok(())
    }
}
impl ReadResultTableRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.limit == 0 || self.limit > 1000 || self.offset.checked_add(self.limit).is_none() {
            return Err(CapabilityContractError::InvalidField("limit"));
        }
        if self.column_limit == 0 || self.column_limit > 100 {
            return Err(CapabilityContractError::InvalidField("columnLimit"));
        }
        if self.columns.len() > 100 || self.columns.iter().any(|name| name.len() > 4096) {
            return Err(CapabilityContractError::InvalidField("columns"));
        }
        Ok(())
    }
}
impl ListGraphResultsRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        crate::validate_resource_id("graph", &self.graph.id)?;
        if self.limit == 0 || self.limit > 100 {
            return Err(CapabilityContractError::InvalidField("limit"));
        }
        if self.node_ids.len() > 200 {
            return Err(CapabilityContractError::InvalidField("nodeIds"));
        }
        if self.outputs.len() > 200 {
            return Err(CapabilityContractError::InvalidField("outputs"));
        }
        for id in &self.node_ids {
            crate::validate_resource_id("nodeIds", id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultValidity {
    CurrentValid,
    CurrentStale,
    Retained,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultTablePage {
    pub table_ref: TableRef,
    pub validity: ResultValidity,
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<serde_json::Value>,
    pub page: InspectionPage,
    pub column_page: InspectionPage,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_references_roundtrip_exact_identities_and_reject_malformed_tokens() {
        let result = ResultRef::new("00000000-0000-0000-0000-000000000001".into(), u64::MAX);
        let wire = serde_json::to_value(&result).unwrap();
        assert!(wire.is_string());
        assert_eq!(
            serde_json::from_value::<ResultRef>(wire.clone()).unwrap(),
            result
        );
        for part in [None, Some("structured:/阶段/2/a~1~0".into())] {
            let table = TableRef::new(result.clone(), part);
            let encoded = serde_json::to_value(&table).unwrap();
            assert!(encoded.is_string());
            assert_eq!(serde_json::from_value::<TableRef>(encoded).unwrap(), table);
        }
        for token in [
            "",
            "r.0.0000000000000001",
            "r.ff.0000000000000001",
            "r.61.1",
            "r.61.0000000000000001.extra",
        ] {
            assert!(ResultRef::try_from(token.to_owned()).is_err(), "{token}");
        }
        let input = crate::capability_input_schema(crate::CapabilityId::InspectResult);
        let properties = &input.as_value()["properties"];
        let reference_schema = properties["resultRef"]["$ref"].as_str().unwrap();
        assert_eq!(
            input
                .as_value()
                .pointer(reference_schema.strip_prefix('#').unwrap())
                .unwrap()["type"],
            "string"
        );
        assert!(properties.get("executionSessionId").is_none());
        assert!(properties.get("part").is_none());
    }
}
