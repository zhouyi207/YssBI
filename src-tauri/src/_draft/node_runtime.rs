use crate::graph::{NodeInstance, NodeState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeRuntime {
    pub id: String,
    pub state: NodeState,
    pub instance: NodeInstance,
}
