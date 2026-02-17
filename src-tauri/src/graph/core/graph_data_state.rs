use crate::graph::{ConnectionManager};
use crate::graph::TypeVarId;
use crate::graph::DataType;
use crate::graph::{NodeId, NodeInstance, PinId, PinInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// - 所有 Node 实例
/// - 所有 Pin 实例
/// - 所有连接关系
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDataState {
    pub nodes: HashMap<NodeId, NodeInstance>,
    pub pins: HashMap<PinId, PinInstance>,
    pub connections: ConnectionManager,

    // 新增：Pin 类型缓存
    pub pin_types: HashMap<PinId, DataType>,

    // 新增：TypeVar 类型缓存
    pub type_var_bindings: HashMap<TypeVarId, DataType>,
}

impl Default for GraphDataState {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            pins: Default::default(),
            connections: Default::default(),
            pin_types: Default::default(),
            type_var_bindings: Default::default(),
        }
    }
}

impl GraphDataState {
    pub fn add_node(&mut self, node_instance: NodeInstance) {
        self.nodes.insert(node_instance.id, node_instance);
    }

    pub fn add_pins(&mut self, pin_instances: Vec<PinInstance>) {
        for pin_instance in pin_instances {
            self.pins.insert(pin_instance.id, pin_instance);
        }
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        self.nodes.remove(&node_id);
    }

    pub fn remove_pins(&mut self, pin_ids: Vec<PinId>) {
        for pin_id in pin_ids {
            self.pins.remove(&pin_id);
        }
    }
}
