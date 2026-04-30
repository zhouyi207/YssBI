use crate::graph::ConnectionManager;
use crate::graph::DataType;
use crate::graph::TypeVarId;
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
            self.pin_types.remove(&pin_id);
        }
    }

    /// 替换节点的 pins（用于动态 pin 重建）
    ///
    /// 1. 移除旧 pins 及其连接
    /// 2. 添加新 pins
    /// 3. 更新节点的 pin_ids
    ///
    /// 返回被移除的 pin IDs 和被断开的连接（供事件通知使用）
    pub fn replace_node_pins(
        &mut self,
        node_id: NodeId,
        old_pin_ids: Vec<PinId>,
        new_pins: Vec<PinInstance>,
    ) -> (Vec<PinId>, Vec<(PinId, PinId)>) {
        let mut removed_connections = Vec::new();

        // 收集旧 pins 上的连接，然后断开
        for &pin_id in &old_pin_ids {
            // 收集作为 output 的连接（from_pin -> to_pins）
            let downstream = self.connections.get_downstream(pin_id);
            for to_pin in &downstream {
                removed_connections.push((pin_id, *to_pin));
            }
            // 收集作为 input 的连接（from_pin -> pin_id）
            if let Some(from_pin) = self.connections.get_upstream(pin_id) {
                removed_connections.push((from_pin, pin_id));
            }
            // 断开所有连接
            self.connections.disconnect_all(pin_id);
            // 移除 pin
            self.pins.remove(&pin_id);
            self.pin_types.remove(&pin_id);
        }

        // 添加新 pins
        let new_pin_ids: Vec<PinId> = new_pins.iter().map(|p| p.id).collect();
        for pin in new_pins {
            self.connections.register_pin(pin.id, node_id);
            self.pins.insert(pin.id, pin);
        }

        // 更新节点的 pin_ids
        if let Some(node) = self.nodes.get_mut(&node_id) {
            // 移除旧的，添加新的
            node.pin_ids.retain(|id| !old_pin_ids.contains(id));
            node.pin_ids.extend(new_pin_ids);
        }

        (old_pin_ids, removed_connections)
    }
}
