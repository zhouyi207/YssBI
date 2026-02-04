//! Graph 实现

use crate::executor::connection::{Connection, ConnectionManager};
use crate::executor::node::{NodeDefinition, NodeId, NodeInstance, NodeState};
use crate::executor::pin::{PinId, PinInstance, PinRole};
use crate::executor::register::NodeRegistry;
use crate::executor::value::{DataValue, ValueType};
use crate::executor::TypeInferenceContext;
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
    // 图 id
    pub id: String,

    // 图 name
    pub name: String,

    // 图节点
    nodes: RwLock<HashMap<NodeId, NodeInstance>>,

    // 图 pins
    pins: RwLock<HashMap<PinId, PinInstance>>,

    // 图 pins 的连接状态
    connections: Arc<ConnectionManager>,

    // 节点类型注册表
    registry: Arc<NodeRegistry>,

    // pin 类型推断上下文
    type_inference: RwLock<TypeInferenceContext>,
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
            type_inference: RwLock::new(TypeInferenceContext::new()),
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

            // 只注册有类型描述的 Pin（Data Pin）
            if let Some(type_desc) = &pin.definition.type_desc {
                self.type_inference
                    .write()
                    .unwrap()
                    .register_pin(pin_id, type_desc.clone());
            }
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

        self.rebuild_type_inference();

        Ok(())
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<NodeInstance> {
        self.nodes.read().unwrap().get(&node_id).cloned()
    }

    pub fn nodes(&self) -> Vec<NodeInstance> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    // =========================
    // ⭐ 类型推断
    // =========================

    fn rebuild_type_inference(&self) {
        let mut ti = self.type_inference.write().unwrap();
        ti.clear();
        let pins = self.pins.read().unwrap();
        
        // 只注册有类型描述的 Pin（Data Pin）
        for pin in pins.values() {
            if let Some(type_desc) = &pin.definition.type_desc {
                ti.register_pin(pin.id, type_desc.clone());
            }
        }
        
        // 重新推断所有连接
        for conn in self.connections.all_connections() {
            let _ = ti.infer_connection(conn.from_pin, conn.to_pin);
        }
    }

    /// ⭐ 查询 Pin 推断后的类型
    pub fn resolve_pin_type(&self, pin_id: PinId) -> Result<ValueType, String> {
        self.type_inference.read().unwrap().resolve_pin_type(pin_id)
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

        // 只对有类型描述的 Pin（Data Pin）进行类型推断
        // Exec Pin 没有类型描述，不需要类型推断
        let from_pin_instance = pins.get(&from_pin).unwrap();
        let to_pin_instance = pins.get(&to_pin).unwrap();
        
        if from_pin_instance.definition.type_desc.is_some() 
            && to_pin_instance.definition.type_desc.is_some() {
            self.type_inference
                .write()
                .unwrap()
                .infer_connection(from_pin, to_pin)?;
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
        Some(nodes.get(&node_id)?.definition.clone())
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
    // Role 查询（支持动态 Pin）
    // =========================

    /// 通过 Role 获取 Pin（支持静态和动态 Pin）
    /// 
    /// 查询顺序：
    /// 1. 先查询动态 Pin 映射（NodeInstance.role_to_pin）
    /// 2. 再查询静态 Pin（PinDefinition.role）
    pub fn get_pin_by_role(&self, node_id: NodeId, role: &PinRole) -> Option<PinInstance> {
        // 1️⃣ 先查询动态 Pin
        if let Some(node) = self.get_node(node_id) {
            if let Some(pin_id) = node.get_dynamic_pin_id(role) {
                return self.get_pin(pin_id);
            }
        }

        // 2️⃣ 再查询静态 Pin
        self.get_node_pins(node_id)
            .into_iter()
            .find(|p| &p.definition.role == role)
    }

    /// 通过 Role 获取多个 Pin（用于动态 Pin 组）
    pub fn get_pins_by_role(&self, node_id: NodeId, role: &PinRole) -> Vec<PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .filter(|p| &p.definition.role == role)
            .collect()
    }

    /// 通过 Role 家族获取所有匹配的 Pin
    /// 
    /// 例如：获取所有 Operands(n) 的 Pin
    pub fn get_pins_by_role_family(&self, node_id: NodeId, pattern: &PinRole) -> Vec<PinInstance> {
        self.get_node_pins(node_id)
            .into_iter()
            .filter(|p| p.definition.role.matches_family(pattern))
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
