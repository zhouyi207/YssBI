use crate::graph::NodeState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeRuntimeState {
    pub id: String,
    pub state: NodeState,
}
