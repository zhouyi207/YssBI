use crate::graph::{GraphId, GraphKind, GraphPosition, GraphData};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphDTO {
    pub id: GraphId,
    pub name: String,
    pub kind: GraphKind,
    pub position: GraphPosition,
}


impl From<&GraphData> for GraphDTO {
    fn from(value: &GraphData) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            kind: value.kind.clone(),
            position: value.position.clone(),
        }
    }
}