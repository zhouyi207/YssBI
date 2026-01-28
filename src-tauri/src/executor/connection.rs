//! 连接管理模块
//!
//! 负责管理节点间的 Pin 连接、类型校验和循环检测

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::executor::error::{ConnectionError, ConnectionResult};
use crate::executor::node::{GenericNode, Node, NodeId};
use crate::executor::pin::{InDataPin, OutDataPin, PinId};

/// 连接信息
#[derive(Debug, Clone)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_pin: PinId,
    pub to_node: NodeId,
    pub to_pin: PinId,
}

/// 连接管理器
///
/// 管理图中所有节点间的连接，提供类型校验和循环检测
#[derive(Debug)]
pub struct ConnectionManager {
    /// 所有连接（from_pin -> to_pin）
    connections: Mutex<HashMap<PinId, Vec<PinId>>>,

    /// Pin 到节点的映射
    pin_to_node: Mutex<HashMap<PinId, NodeId>>,

    /// 节点到 Pin 的映射
    node_to_pins: Mutex<HashMap<NodeId, Vec<PinId>>>,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            pin_to_node: Mutex::new(HashMap::new()),
            node_to_pins: Mutex::new(HashMap::new()),
        }
    }

    /// 注册节点的所有 Pin
    pub fn register_node(&self, node: &GenericNode) -> ConnectionResult<()> {
        let node_id = node.id();
        let mut pin_to_node = self.pin_to_node.lock().unwrap();
        let mut node_to_pins = self.node_to_pins.lock().unwrap();

        let mut pins = Vec::new();

        // 注册输入 Pin (使用 inputs() 方法直接获取所有输入 pins)
        for pin in node.inputs() {
            let pin_id = pin.id();
            pin_to_node.insert(pin_id, node_id);
            pins.push(pin_id);
        }

        // 注册输出 Pin (使用 outputs() 方法直接获取所有输出 pins)
        for pin in node.outputs() {
            let pin_id = pin.id();
            pin_to_node.insert(pin_id, node_id);
            pins.push(pin_id);
        }

        node_to_pins.insert(node_id, pins);

        Ok(())
    }

    /// 连接两个 Pin
    pub fn connect(
        &self,
        from_pin: &Arc<dyn OutDataPin>,
        to_pin: &Arc<dyn InDataPin>,
    ) -> ConnectionResult<()> {
        let from_id = from_pin.id();
        let to_id = to_pin.id();

        // 1. 类型检查
        if from_pin.data_type() != to_pin.data_type()
            && to_pin.data_type() != "any"
            && from_pin.data_type() != "any"
        {
            return Err(ConnectionError::TypeMismatch {
                from_type: from_pin.data_type().to_string(),
                to_type: to_pin.data_type().to_string(),
            });
        }

        // 2. 获取节点 ID
        let pin_to_node = self.pin_to_node.lock().unwrap();
        let from_node = *pin_to_node
            .get(&from_id)
            .ok_or(ConnectionError::PinNotFound(from_id))?;
        let to_node = *pin_to_node
            .get(&to_id)
            .ok_or(ConnectionError::PinNotFound(to_id))?;

        // 3. 循环检测
        if from_node == to_node {
            // 同一节点内的连接通常是允许的，但需要检查是否会导致无限循环
            // 这里简化处理，允许同节点连接
        } else if self.would_create_cycle(from_node, to_node)? {
            return Err(ConnectionError::CycleDetected { from_node, to_node });
        }

        // 4. 检查连接是否已存在
        let mut connections = self.connections.lock().unwrap();
        if let Some(targets) = connections.get(&from_id) {
            if targets.contains(&to_id) {
                return Err(ConnectionError::AlreadyConnected {
                    from_pin: from_id,
                    to_pin: to_id,
                });
            }
        }

        // 5. 建立连接
        connections
            .entry(from_id)
            .or_insert_with(Vec::new)
            .push(to_id);

        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) -> ConnectionResult<()> {
        let mut connections = self.connections.lock().unwrap();

        if let Some(targets) = connections.get_mut(&from_pin) {
            if let Some(pos) = targets.iter().position(|&id| id == to_pin) {
                targets.remove(pos);
                return Ok(());
            }
        }

        Err(ConnectionError::Generic(format!(
            "连接不存在：from={}, to={}",
            from_pin, to_pin
        )))
    }

    /// 获取 Pin 的所有下游连接
    pub fn get_downstream(&self, pin_id: PinId) -> Vec<PinId> {
        self.connections
            .lock()
            .unwrap()
            .get(&pin_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取 Pin 的上游连接
    pub fn get_upstream(&self, pin_id: PinId) -> Option<PinId> {
        let connections = self.connections.lock().unwrap();
        for (from, targets) in connections.iter() {
            if targets.contains(&pin_id) {
                return Some(*from);
            }
        }
        None
    }

    /// 获取节点的所有直接上游节点
    pub fn get_upstream_nodes(&self, node_id: NodeId) -> HashSet<NodeId> {
        let node_to_pins = self.node_to_pins.lock().unwrap();
        let pin_to_node = self.pin_to_node.lock().unwrap();
        let connections = self.connections.lock().unwrap();

        let mut upstream_nodes = HashSet::new();

        if let Some(pins) = node_to_pins.get(&node_id) {
            for &pin_id in pins {
                // 遍历所有连接，寻找以当前节点 Pin 为目标的连接
                for (&from_pin, targets) in connections.iter() {
                    if targets.contains(&pin_id) {
                        if let Some(&from_node) = pin_to_node.get(&from_pin) {
                            if from_node != node_id {
                                upstream_nodes.insert(from_node);
                            }
                        }
                    }
                }
            }
        }
        upstream_nodes
    }

    /// 获取节点的所有直接下游节点
    pub fn get_downstream_nodes(&self, node_id: NodeId) -> HashSet<NodeId> {
        let node_to_pins = self.node_to_pins.lock().unwrap();
        let pin_to_node = self.pin_to_node.lock().unwrap();
        let connections = self.connections.lock().unwrap();

        let mut downstream_nodes = HashSet::new();

        if let Some(pins) = node_to_pins.get(&node_id) {
            for &pin_id in pins {
                if let Some(targets) = connections.get(&pin_id) {
                    for &target_pin in targets {
                        if let Some(&target_node) = pin_to_node.get(&target_pin) {
                            if target_node != node_id {
                                downstream_nodes.insert(target_node);
                            }
                        }
                    }
                }
            }
        }
        downstream_nodes
    }

    /// 检查是否会形成循环
    fn would_create_cycle(&self, from_node: NodeId, to_node: NodeId) -> ConnectionResult<bool> {
        let node_to_pins = self.node_to_pins.lock().unwrap();
        let connections = self.connections.lock().unwrap();
        let pin_to_node = self.pin_to_node.lock().unwrap();

        // 使用 DFS 检测是否存在从 to_node 到 from_node 的路径
        let mut visited = HashSet::new();
        let mut stack = vec![to_node];

        while let Some(current_node) = stack.pop() {
            if current_node == from_node {
                return Ok(true); // 找到循环
            }

            if visited.contains(&current_node) {
                continue;
            }
            visited.insert(current_node);

            // 获取当前节点的所有输出 Pin
            if let Some(pins) = node_to_pins.get(&current_node) {
                for pin_id in pins {
                    // 获取这个 Pin 的所有下游连接
                    if let Some(targets) = connections.get(pin_id) {
                        for target_pin in targets {
                            // 找到目标 Pin 所属的节点
                            if let Some(&target_node) = pin_to_node.get(target_pin) {
                                if !visited.contains(&target_node) {
                                    stack.push(target_node);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(false) // 没有找到循环
    }

    /// 获取所有连接
    pub fn get_all_connections(&self) -> Vec<Connection> {
        let connections = self.connections.lock().unwrap();
        let pin_to_node = self.pin_to_node.lock().unwrap();

        let mut result = Vec::new();

        for (from_pin, targets) in connections.iter() {
            let from_node = *pin_to_node.get(from_pin).unwrap();
            for to_pin in targets {
                let to_node = *pin_to_node.get(to_pin).unwrap();
                result.push(Connection {
                    from_node,
                    from_pin: *from_pin,
                    to_node,
                    to_pin: *to_pin,
                });
            }
        }

        result
    }

    /// 清除所有连接
    pub fn clear(&self) {
        self.connections.lock().unwrap().clear();
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
