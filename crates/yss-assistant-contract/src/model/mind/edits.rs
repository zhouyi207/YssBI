use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTopicsInput {
    pub mind: MindResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub topics: Vec<TopicCreation>,
    #[serde(default)]
    pub before_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicCreation {
    /// Unique in this call; refer to another new topic as $clientId, in any order.
    pub client_id: String,
    pub parent_id: String,
    pub content: String,
    #[serde(default)]
    pub reference: Option<MindResourceReference>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTopicsInput {
    pub mind: MindResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub topics: Vec<TopicUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicUpdate {
    pub topic_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Omit to retain the reference; null clears it. An unavailable external target may be retained.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::graph::present_optional"
    )]
    pub reference: Option<Option<MindResourceReference>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveTopicsInput {
    pub mind: MindResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub topics: Vec<TopicMove>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicMove {
    pub topic_id: String,
    pub parent_id: String,
    #[serde(default)]
    pub before_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteTopicsInput {
    pub mind: MindResourceRef,
    /// Remove these topics and all descendants. The resource root cannot be removed here.
    #[schemars(length(min = 1, max = 200))]
    pub topic_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateTopicsInput {
    pub mind: MindResourceRef,
    /// Disjoint subtree roots. The receipt maps every original ID to its new copy.
    #[schemars(length(min = 1, max = 200))]
    pub topic_ids: Vec<String>,
    pub parent_id: String,
    #[serde(default)]
    pub before_id: Option<String>,
}

pub(crate) fn validate_topic_edits(edit: &ResourceEdit) -> Result<usize, CapabilityContractError> {
    let count = match edit {
        ResourceEdit::CreateTopics { topics, before_id } => {
            topic_ids(topics.iter().map(|v| v.client_id.as_str()))?;
            for topic in topics {
                if topic.client_id.starts_with('$') {
                    return Err(CapabilityContractError::InvalidField("clientId"));
                }
                topic_id(
                    "parentId",
                    topic
                        .parent_id
                        .strip_prefix('$')
                        .unwrap_or(&topic.parent_id),
                )?;
            }
            if let Some(id) = before_id {
                topic_id("beforeId", id)?;
            }
            topics.len()
        }
        ResourceEdit::UpdateTopics { topics } => {
            topic_ids(topics.iter().map(|v| v.topic_id.as_str()))?;
            if topics
                .iter()
                .any(|v| v.content.is_none() && v.reference.is_none())
            {
                return Err(CapabilityContractError::InvalidField("topics"));
            }
            topics.len()
        }
        ResourceEdit::MoveTopics { topics } => {
            topic_ids(topics.iter().map(|v| v.topic_id.as_str()))?;
            for topic in topics {
                topic_id("parentId", &topic.parent_id)?;
                if let Some(id) = &topic.before_id {
                    topic_id("beforeId", id)?;
                }
            }
            topics.len()
        }
        ResourceEdit::DeleteTopics { topic_ids: ids } => {
            topic_ids(ids.iter().map(String::as_str))?;
            ids.len()
        }
        ResourceEdit::DuplicateTopics {
            topic_ids: ids,
            parent_id,
            before_id,
        } => {
            topic_ids(ids.iter().map(String::as_str))?;
            topic_id("parentId", parent_id)?;
            if let Some(id) = before_id {
                topic_id("beforeId", id)?;
            }
            ids.len()
        }
        _ => return Err(CapabilityContractError::InvalidField("edit.kind")),
    };
    Ok(count)
}
