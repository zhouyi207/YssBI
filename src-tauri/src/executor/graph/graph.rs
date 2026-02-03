//! Graph 实现

use crate::executor::connection::ConnectionManager;
use crate::executor::node::{NodeDefinition, NodeInstance, NodeId};
use crate::executor::pin::{PinId, PinInstance};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::executor::register::NodeRegistry;

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node 实例
/// - 所有 Pin 实例
/// - 所有连接关系
pub struct Graph {
    /// Graph ID
    pub id: String,
    
    /// Graph 名称
    pub name: String,
    
    /// 所有节点实例
    nodes: RwLock<HashMap<NodeId, NodeInstance>>,
    
    /// 所有 Pin 实例
    pins: RwLock<HashMap<PinId, PinInstance>>,
    
    /// 连接管理器
    connections: Arc<ConnectionManager>,
    
    /// 节点注册中心（用于创建节点）
    registry: Arc<NodeRegistry>,
}

impl Graph {
    pub fn new(id: impl Into<String>, name: impl Into<String>, registry: Arc<NodeRegistry>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            nodes: RwLock::new(HashMap::new()),
            pins: RwLock::new(HashMap::new()),
            connections: Arc::new(ConnectionManager::new()),
            registry,
        }
    }

    /// 创建节点实例
    pub fn create_node(&self, node_type: &str) -> Result<NodeId, String> {
        // 从注册中心获取定义
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        // 创建节点实例
        let node = NodeInstance::from_definition(definition.clone());
        let node_id = node.id;

        // 为节点创建 Pin 实例
        for pin_def in &definition.pins {
            let pin = PinInstance::from_definition(pin_def, node_id, 20);
            let pin_id = pin.id;
            
            // 注册 Pin
            self.pins.write().unwrap().insert(pin_id, pin);
            self.connections.register_pin(pin_id, node_id);
        }

        // 存储节点
        self.nodes.write().unwrap().insert(node_id, node);

        Ok(node_id)
    }

    /// 删除节点
    pub fn remove_node(&self, node_id: NodeId) -> Result<(), String> {
        // 获取节点的所有 Pin
        let pin_ids = self.connections.get_node_pins(node_id);

        // 删除所有连接
        self.connections.remove_node(node_id);

        // 删除所有 Pin
        let mut pins = self.pins.write().unwrap();
        for pin_id in pin_ids {
            pins.remove(&pin_id);
        }

        // 删除节点
        self.nodes
            .write()
            .unwrap()
            .remove(&node_id)
            .ok_or_else(|| format!("Node {:?} not found", node_id))?;

        Ok(())
    }

    /// 获取节点
    pub fn get_node(&self, node_id: NodeId) -> Option<NodeInstance> {
        self.nodes.read().unwrap().get(&node_id).cloned()
    }

    /// 获取所有节点
    pub fn nodes(&self) -> Vec<NodeInstance> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    /// 获取节点的所有 Pin
    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<PinInstance> {
        let pin_ids = self.connections.get_node_pins(node_id);
        let pins = self.pins.read().unwrap();
        pin_ids
            .into_iter()
            .filter_map(|id| pins.get(&id).cloned())
            .collect()
    }

    /// 获取 Pin
    pub fn get_pin(&self, pin_id: PinId) -> Option<PinInstance> {
        self.pins.read().unwrap().get(&pin_id).cloned()
    }

    /// 更新 Pin 值
    pub fn set_pin_value(&self, pin_id: PinId, value: crate::executor::value::DataValue) -> Result<(), String> {
        let mut pins = self.pins.write().unwrap();
        let pin = pins
            .get_mut(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;
        
        pin.set_value(value);
        Ok(())
    }

    /// 获取 Pin 值
    pub fn get_pin_value(&self, pin_id: PinId) -> Option<crate::executor::value::DataValue> {
        self.pins
            .read()
            .unwrap()
            .get(&pin_id)
            .and_then(|p| p.effective_value().cloned())
    }

    /// 设置 Pin 用户值
    pub fn set_pin_user_value(&self, pin_id: PinId, value: Option<crate::executor::value::DataValue>) -> Result<(), String> {
        let mut pins = self.pins.write().unwrap();
        let pin = pins
            .get_mut(&pin_id)
            .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;
        
        pin.set_user_value(value);
        Ok(())
    }

    /// 连接两个 Pin
    pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Result<(), String> {
        // 验证 Pin 存在
        {
            let pins = self.pins.read().unwrap();
            if !pins.contains_key(&from_pin) {
                return Err(format!("Source pin {:?} not found", from_pin));
            }
            if !pins.contains_key(&to_pin) {
                return Err(format!("Target pin {:?} not found", to_pin));
            }
        }

        // 建立连接
        self.connections.connect(from_pin, to_pin)
    }

    /// 断开连接
    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) {
        self.connections.disconnect(from_pin, to_pin);
    }

    /// 获取连接管理器
    pub fn connections(&self) -> &Arc<ConnectionManager> {
        &self.connections
    }

    /// 获取所有连接
    pub fn all_connections(&self) -> Vec<crate::executor::connection::Connection> {
        self.connections.all_connections()
    }

    /// 清空 Graph
    pub fn clear(&self) {
        self.nodes.write().unwrap().clear();
        self.pins.write().unwrap().clear();
        self.connections.clear();
    }

    /// 获取节点定义
    pub fn get_node_definition(&self, node_id: NodeId) -> Option<Arc<NodeDefinition>> {
        let nodes = self.nodes.read().unwrap();
        let node = nodes.get(&node_id)?;
        node.definition.clone()
    }
}
