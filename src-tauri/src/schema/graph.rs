use crate::graph::{GraphId, GraphKind, GraphPosition, GraphInstance};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphDTO {
    pub id: GraphId,
    pub name: String,
    pub kind: GraphKind,
    pub position: GraphPosition,
}


impl From<&GraphInstance> for GraphDTO {
    fn from(value: &GraphInstance) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            kind: value.kind.clone(),
            position: value.position.clone(),
        }
    }
}