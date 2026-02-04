//! Graph 实现

use crate::executor::connection::{Connection, ConnectionManager};
use crate::executor::node::{NodeDefinition, NodeId, NodeInstance, NodeState};
use crate::executor::pin::{PinId, PinInstance, PinRole};
use crate::executor::register::NodeRegistry;
use crate::executor::value::{DataValue};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node 实例
/// - 所有 Pin 实例
/// - 所有连接关系
/// - 类型推断上下文
pub struct Graph {
    pub id: String,
    pub name: String,

    nodes: RwLock<HashMap<NodeId, NodeInstance>>,
    pins: RwLock<HashMap<PinId, PinInstance>>,
    connections: Arc<ConnectionManager>,
    registry: Arc<NodeRegistry>,
}

impl Graph {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        registry: Arc<NodeRegistry>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            nodes: RwLock::new(HashMap::new()),
            pins: RwLock::new(HashMap::new()),
            connections: Arc::new(ConnectionManager::new()),
            registry,
        }
    }

    // =========================
    // Node 管理
    // =========================

    pub fn create_node(&self, node_type: &str) -> Result<NodeId, String> {
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        let node = NodeInstance::from_definition(definition.clone());
        let node_id = node.id;

        // 创建 Pin 并注册到类型推断系统
        for pin_def in &definition.pins {
            let pin = PinInstance::from_definition(pin_def, node_id, 20);
            let pin_id = pin.id;

            self.pins.write().unwrap().insert(pin_id, pin.clone());
            self.connections.register_pin(pin_id, node_id);
        }

        self.nodes.write().unwrap().insert(node_id, node);
        Ok(node_id)
    }

    pub fn remove_node(&self, node_id: NodeId) -> Result<(), String> {
        let pin_ids = self.connections.get_node_pins(node_id);
        self.connections.remove_node(node_id);

        let mut pins = self.pins.write().unwrap();
        for pin_id in pin_ids {
            pins.remove(&pin_id);
        }

        self.nodes
            .write()
            .unwrap()
            .remove(&node_id)
            .ok_or_else(|| format!("Node {:?} not found", node_id))?;

        Ok(())
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<NodeInstance> {
        self.nodes.read().unwrap().get(&node_id).cloned()
    }

    pub fn nodes(&self) -> Vec<NodeInstance> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    // =========================
    // Pin 管理
    // =========================

    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<PinInstance> {
        let pin_ids = self.connections.get_node_pins(node_id);
        let pins = self.pins.read().unwrap();

        pin_ids
            .into_iter()
            .filter_map(|id| pins.get(&id).cloned())
            .collect()
    }

    pub fn get_pin(&self, pin_id: PinId) -> Option<PinInstance> {
        self.pins.read().unwrap().get(&pin_id).cloned()
    }

    pub fn set_pin_current_value(&self, pin_id: PinId, value: DataValue) -> Result<(), String> {
        let mut pins = self.pins.write().unwrap();
        let pin = pins
            .get_mut(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;

        pin.set_current_value(value);
        Ok(())
    }

    pub fn set_pin_user_value(
        &self,
        pin_id: PinId,
        value: Option<DataValue>,
    ) -> Result<(), String> {
        let mut pins = self.pins.write().unwrap();
        let pin = pins
            .get_mut(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;

        pin.set_user_value(value);
        Ok(())
    }

    // =========================
    // ⭐ 核心：值解析逻辑（Graph 负责）
    // =========================

    /// 获取 Pin 的“有效值”
    ///
    /// 顺序：
    /// 1. 上游连接值
    /// 2. Pin 当前运行时值
    /// 3. 用户填写值
    /// 4. 定义期默认值
    pub fn resolve_pin_value(&self, pin_id: PinId) -> Option<DataValue> {
        let pins = self.pins.read().unwrap();
        let pin = pins.get(&pin_id)?;

        // 1️⃣ 上游连接
        if let Some(upstream) = self.connections.get_upstream(pin_id) {
            if let Some(v) = self.resolve_pin_value(upstream) {
                return Some(v);
            }
        }

        // 2️⃣ 当前运行时值
        if let Some(v) = pin.current_value() {
            return Some(v.clone());
        }

        // 3️⃣ 用户值
        if let Some(v) = pin.user_value() {
            return Some(v.clone());
        }

        // 4️⃣ 默认值
        pin.definition.default_value.clone()
    }

    // =========================
    // 连接管理
    // =========================

    pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        let pins = self.pins.read().unwrap();

        if !pins.contains_key(&from_pin) {
            return Err(format!("Source pin {:?} not found", from_pin));
        }
        if !pins.contains_key(&to_pin) {
            return Err(format!("Target pin {:?} not found", to_pin));
        }

        self.connections.connect(from_pin, to_pin)
    }

    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) {
        self.connections.disconnect(from_pin, to_pin);
    }

    pub fn connections(&self) -> &Arc<ConnectionManager> {
        &self.connections
    }

    pub fn all_connections(&self) -> Vec<Connection> {
        self.connections.all_connections()
    }

    // =========================
    // Node 状态
    // =========================

    pub fn get_node_definition(&self, node_id: NodeId) -> Option<Arc<NodeDefinition>> {
        let nodes = self.nodes.read().unwrap();
        nodes.get(&node_id)?.definition.clone()
    }

    pub fn get_node_state(&self, node_id: NodeId) -> Option<NodeState> {
        self.nodes.read().unwrap().get(&node_id).map(|n| n.state)
    }

    pub fn set_node_state(&self, node_id: NodeId, state: NodeState) -> Result<(), String> {
        let mut nodes = self.nodes.write().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .ok_or_else(|| format!("Node {:?} not found", node_id))?;

        node.state = state;
        Ok(())
    }

    // =========================
    // Role 查询
    // =========================

    pub fn get_pin_by_role(&self, node_id: NodeId, role: &PinRole) -> Option<PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .find(|p| &p.definition.role == role)
    }

    pub fn get_pins_by_role(&self, node_id: NodeId, role: &PinRole) -> Vec<PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .filter(|p| &p.definition.role == role)
            .collect()
    }

    // =========================
    // 清理
    // =========================

    pub fn clear(&self) {
        self.nodes.write().unwrap().clear();
        self.pins.write().unwrap().clear();
        self.connections.clear();
    }
}
