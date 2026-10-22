//! Model-visible knowledge queries and content references. No synchronization fields.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CapabilityContractError, KnowledgeChunkId, KnowledgeDocumentId, KnowledgeSourceId};

pub const MAX_KNOWLEDGE_RESULTS: u16 = 20;

pub const SEARCH_KNOWLEDGE_TOOL: &str = "knowledge_search";
pub const READ_KNOWLEDGE_TOOL: &str = "knowledge_read";

#[cfg(any(test, feature = "test-support"))]
pub fn search_knowledge_schema() -> serde_json::Value {
    serde_json::Value::from(schemars::schema_for!(SearchKnowledgeRequest))
}

#[cfg(any(test, feature = "test-support"))]
pub fn read_knowledge_schema() -> serde_json::Value {
    serde_json::Value::from(schemars::schema_for!(ReadKnowledgeRequest))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchKnowledgeRequest {
    pub query: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 20))]
    pub limit: u16,
}

fn default_limit() -> u16 {
    5
}

impl SearchKnowledgeRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        if self.query.trim().is_empty() {
            return Err(CapabilityContractError::InvalidField("query"));
        }
        if self.scopes.iter().any(|scope| scope.trim().is_empty()) {
            return Err(CapabilityContractError::InvalidField("scopes"));
        }
        if self.limit == 0 || self.limit > MAX_KNOWLEDGE_RESULTS {
            return Err(CapabilityContractError::InvalidLimit {
                maximum: MAX_KNOWLEDGE_RESULTS,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgePassageReference {
    pub document_id: KnowledgeDocumentId,
    pub chunk_id: KnowledgeChunkId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadKnowledgeRequest {
    pub reference: KnowledgePassageReference,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeMatch {
    pub reference: KnowledgePassageReference,
    pub source_id: KnowledgeSourceId,
    pub title: String,
    pub excerpt: String,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeSearchResult {
    pub matches: Vec<KnowledgeMatch>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgePassage {
    pub reference: KnowledgePassageReference,
    pub source_id: KnowledgeSourceId,
    pub title: String,
    pub text: String,
}
