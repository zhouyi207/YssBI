//! Graph 实现
//!
//! ❌ 不负责「定义」
//! ❌ 不负责「编译策略」
//! ❌ 不负责「执行调度」
//! ✅ 持有状态
//! ✅ 提供受控 mutation API

use super::{GraphDataState, GraphKind, GraphPosition};
use crate::graph::connection::Connection;
use crate::graph::node::{NodeId, NodeInstance};
use crate::graph::pin::{PinId, PinInstance, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataValue;
use crate::graph::{DataType, GraphId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node, Pin 实例 和连接关系
/// - 类型推断上下文
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphInstance {
    // 图 id
    pub id: GraphId,

    // 图 name
    pub name: String,

    // 位置
    pub position: GraphPosition,

    // 类型
    pub kind: GraphKind,

    // 数据状态 (node, pin, connection)
    pub data_state: Arc<RwLock<GraphDataState>>,

    // 节点类型注册表（序列化时跳过，需要在加载后重新设置）
    #[serde(skip)]
    registry: Arc<NodeRegistry>,
}

/// 创建和清理
impl GraphInstance {
    pub fn new(name: impl Into<String>, kind: GraphKind, registry: Arc<NodeRegistry>) -> Self {
        Self {
            id: GraphId::new(),
            name: name.into(),
            position: GraphPosition::default(),
            kind,
            data_state: Default::default(),
            registry,
        }
    }

    pub fn clear(&self) {
        *self.data_state.write().unwrap() = GraphDataState::default();
    }

    /// 设置节点注册表（用于反序列化后恢复）
    pub fn set_registry(&mut self, registry: Arc<NodeRegistry>) {
        self.registry = registry;
    }

    /// 获取节点注册表的引用
    pub fn registry(&self) -> &Arc<NodeRegistry> {
        &self.registry
    }
}

/// Node 管理
impl GraphInstance {
    pub fn create_node(&self, node_type: &str) -> Result<NodeId, String> {
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        let (node, pins) = NodeInstance::from_definition(definition.clone());
        let node_id = node.id;

        let mut data_state = self.data_state.write().unwrap();
        data_state.add_node(node);
        data_state.add_pins(pins);
        Ok(node_id)
    }

    pub fn remove_node(&self, node_id: NodeId) -> Result<(), String> {
        let pins = self.get_pin_instances_by_node_id(node_id);

        let mut data_state = self.data_state.write().unwrap();
        data_state.connections.remove_node(node_id);

        for pin in pins {
            data_state.pins.remove(&pin.id);
        }
        data_state.nodes.remove(&node_id);

        Ok(())
    }

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> NodeId {
        let data_state = self.data_state.read().unwrap();
        let pin_instance = data_state.pins.get(&pin_id).unwrap();
        pin_instance.node_id
    }

    pub fn get_node_instance_by_node_id(&self, node_id: NodeId) -> Option<NodeInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.get(&node_id).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<NodeInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.values().cloned().collect()
    }
}

/// Pin 管理
impl GraphInstance {
    pub fn get_pin_instances_by_node_id(&self, node_id: NodeId) -> Vec<PinInstance> {
        let data_state = self.data_state.read().unwrap();
        let pin_ids = data_state.nodes.get(&node_id).unwrap().pin_ids.clone();
        let pins = data_state.pins.clone();

        pin_ids
            .into_iter()
            .filter_map(|id| pins.get(&id).cloned())
            .collect()
    }

    pub fn get_pin_instance_by_pin_id(&self, pin_id: PinId) -> Option<PinInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.pins.get(&pin_id).cloned()
    }

    pub fn get_pin_data_type_by_pin_id(&self, pin_id: PinId) -> Option<DataType> {
        self.data_state
            .read()
            .unwrap()
            .pin_types
            .get(&pin_id)
            .cloned()
    }

    pub fn get_pin_user_value_by_pin_id(&self, pin_id: PinId) -> Option<DataValue> {
        let data_state = self.data_state.read().unwrap();
        if let Some(pin) = data_state.pins.get(&pin_id) {
            return pin.user_value.clone();
        }
        None
    }

    pub fn set_pin_user_value_by_pin_id(
        &self,
        pin_id: PinId,
        value: DataValue,
    ) -> Result<(), String> {
        let mut data_state = self.data_state.write().unwrap();

        if let Some(pin) = data_state.pins.get_mut(&pin_id) {
            pin.user_value = Some(value);
        } else {
            return Err(format!("Pin {:?} not found", pin_id));
        }
        Ok(())
    }

    pub fn get_pin_instance_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Option<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .find(|p| &p.definition.role == role)
    }

    /// 通过 Role 获取多个 Pin（用于动态 Pin 组）
    pub fn get_pin_instances_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .filter(|p| &p.definition.role == role)
            .collect()
    }

    pub fn get_pin_instances_by_pin_role_family(
        &self,
        node_id: NodeId,
        pattern: &PinRole,
    ) -> Vec<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .filter(|p| p.definition.role.matches_family(pattern))
            .collect()
    }
}

/// 连接管理
impl GraphInstance {
    pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        let data_state = self.data_state.write().unwrap();
        let pins = data_state.pins.clone();
        if !pins.contains_key(&from_pin) {
            return Err(format!("Source pin {:?} not found", from_pin));
        }
        if !pins.contains_key(&to_pin) {
            return Err(format!("Target pin {:?} not found", to_pin));
        }

        // 只对有类型描述的 Pin（Data Pin）进行类型推断
        // Exec Pin 没有类型描述，不需要类型推断
        let from_pin_instance = pins.get(&from_pin).unwrap();
        let to_pin_instance = pins.get(&to_pin).unwrap();

        if from_pin_instance.definition.data_type.is_some()
            && to_pin_instance.definition.data_type.is_some()
        {}

        data_state.connections.connect(from_pin, to_pin)
    }

    pub fn get_downstream_by_pin_id(&self, pin_id: PinId) -> Vec<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state.connections.get_downstream(pin_id)
    }

    pub fn get_upstream_by_pin_id(&self, pin_id: PinId) -> Option<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state.connections.get_upstream(pin_id)
    }

    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) {
        let data_state = self.data_state.write().unwrap();
        data_state.connections.disconnect(from_pin, to_pin);
    }

    pub fn all_connections(&self) -> Vec<Connection> {
        let data_state = self.data_state.write().unwrap();
        data_state.connections.all_connections()
    }
}
