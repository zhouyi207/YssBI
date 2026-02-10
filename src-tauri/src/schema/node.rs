use crate::graph::{NodeId, NodeInstance};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeInstanceDTO {
    pub id: NodeId,
    pub name: String,
    pub category: Vec<String>,
}

impl From<&NodeInstance> for NodeInstanceDTO {
    fn from(value: &NodeInstance) -> Self {
        Self {
            id: value.id,
            name: value.definition.title.clone(),
            category: value.definition.category.clone(),
        }
    }
}
