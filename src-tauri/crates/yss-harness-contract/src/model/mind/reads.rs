use super::*;

fn default_depth() -> usize {
    2
}
fn default_limit() -> usize {
    50
}
fn default_content_limit() -> usize {
    2048
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectMindInput {
    pub mind: MindResourceRef,
    #[serde(default)]
    pub root_topic_id: Option<String>,
    /// Relative to the selected root. Zero returns only that root.
    #[serde(default = "default_depth")]
    #[schemars(range(max = 20))]
    pub depth: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindTopicsInput {
    pub mind: MindResourceRef,
    /// Case-insensitive literal substring of topic content.
    pub query: String,
    #[serde(default)]
    pub root_topic_id: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectTopicsInput {
    pub mind: MindResourceRef,
    #[schemars(length(min = 1, max = 20))]
    pub topic_ids: Vec<String>,
    /// Unicode scalar offset, applied separately to each selected topic's content.
    #[serde(default)]
    pub content_offset: usize,
    #[serde(default = "default_content_limit")]
    #[schemars(range(min = 1, max = 2048))]
    pub content_limit: usize,
    #[serde(default)]
    pub children_offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 50))]
    pub children_limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "query",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum MindReadInput {
    Outline(InspectMindInput),
    Find(FindTopicsInput),
    Topics(InspectTopicsInput),
}

impl MindReadInput {
    pub fn mind(&self) -> &MindResourceRef {
        match self {
            Self::Outline(v) => &v.mind,
            Self::Find(v) => &v.mind,
            Self::Topics(v) => &v.mind,
        }
    }
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Outline(_) => CapabilityId::InspectMind,
            Self::Find(_) => CapabilityId::FindTopics,
            Self::Topics(_) => CapabilityId::InspectTopics,
        }
    }
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_resource_id("mind.id", &self.mind().id)?;
        let (root, limit, maximum) = match self {
            Self::Outline(v) => {
                if v.depth > 20 {
                    return Err(CapabilityContractError::InvalidField("depth"));
                }
                (v.root_topic_id.as_deref(), v.limit, 200)
            }
            Self::Find(v) => {
                if v.query.is_empty() || v.query.len() > 4096 {
                    return Err(CapabilityContractError::InvalidField("query"));
                }
                (v.root_topic_id.as_deref(), v.limit, 100)
            }
            Self::Topics(v) => {
                topic_ids(v.topic_ids.iter().map(String::as_str))?;
                if v.topic_ids.is_empty() || v.topic_ids.len() > 20 {
                    return Err(CapabilityContractError::InvalidLimit { maximum: 20 });
                }
                if v.content_limit == 0 || v.content_limit > 2048 {
                    return Err(CapabilityContractError::InvalidField("contentLimit"));
                }
                (None, v.children_limit, 50)
            }
        };
        if let Some(root) = root {
            topic_id("rootTopicId", root)?;
        }
        if limit == 0 || limit > maximum {
            return Err(CapabilityContractError::InvalidLimit {
                maximum: maximum as u16,
            });
        }
        Ok(())
    }
}
