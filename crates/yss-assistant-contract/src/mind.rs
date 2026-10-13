//! Internal envelope and bounded topic read projections. Project owns tree state.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindReadRequest {
    pub input: model::MindReadInput,
    pub version: Option<ResourceVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindReadResult {
    pub mind: model::MindResourceRef,
    pub version: ResourceVersion,
    pub dirty: bool,
    pub content: MindReadContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum MindReadContent {
    Outline {
        root_topic_id: String,
        topic_count: usize,
        subtree_count: usize,
        topics: Vec<MindTopicSummary>,
        page: InspectionPage,
    },
    Found {
        topics: Vec<MindTopicMatch>,
        page: InspectionPage,
    },
    Topics {
        topics: Vec<MindTopicInspection>,
    },
}
impl MindReadContent {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Outline { .. } => CapabilityId::InspectMind,
            Self::Found { .. } => CapabilityId::FindTopics,
            Self::Topics { .. } => CapabilityId::InspectTopics,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MindTopicSummary {
    pub topic_id: String,
    pub parent_id: Option<String>,
    pub summary: String,
    pub content_length: usize,
    pub depth: usize,
    pub child_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MindTopicMatch {
    pub topic: MindTopicSummary,
    /// Up to the last 32 ancestor/topic IDs, in root-to-topic order.
    pub path: Vec<String>,
    pub path_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MindTopicInspection {
    pub topic_id: String,
    pub parent_id: Option<String>,
    pub content: String,
    pub content_page: InspectionPage,
    pub child_ids: Vec<String>,
    pub children_page: InspectionPage,
    pub reference: Option<MindResourceReference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MindEditReceipt {
    /// Create: client alias -> ID. Duplicate: original topic ID -> copied ID.
    pub created_topics: BTreeMap<String, String>,
    pub affected_topic_ids: Vec<String>,
    pub deleted_topic_ids: Vec<String>,
    pub dirty: bool,
}
