use crate::graph::ConnectionManager;
use crate::graph::{NodeId, NodeInstance, PinId, PinInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


/// - 所有 Node 实例
/// - 所有 Pin 实例
/// - 所有连接关系
#[derive(Serialize, Deserialize)]
pub struct GraphDataState {
    pub nodes: HashMap<NodeId, NodeInstance>,
    pub pins: HashMap<PinId, PinInstance>,
    pub connections: ConnectionManager,
}

impl Default for GraphDataState {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            pins: Default::default(),
            connections: Default::default(),
        }
    }
}
