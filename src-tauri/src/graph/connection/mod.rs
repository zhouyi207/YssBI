//! 连接管理器
//!
//! ConnectionManager 是连接关系的唯一真实来源（Single Source of Truth）。
//! Pin 不存储连接信息，所有连接查询都通过 ConnectionManager。

pub mod connection_validator;

use crate::graph::NodeId;
use crate::graph::PinId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// 连接（边）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub from_pin: PinId,
    pub to_pin: PinId,
}

impl Connection {
    pub fn new(from_pin: PinId, to_pin: PinId) -> Self {
        Self { from_pin, to_pin }
    }
}

/// 连接管理器
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionManager {
    /// 所有连接（from_pin -> [to_pins]）
    connections: Mutex<HashMap<PinId, Vec<PinId>>>,

    /// 反向索引（to_pin -> from_pin）
    reverse_connections: Mutex<HashMap<PinId, PinId>>,

    /// Pin 到节点的映射
    pin_to_node: Mutex<HashMap<PinId, NodeId>>,

    /// 节点到 Pin 的映射
    node_to_pins: Mutex<HashMap<NodeId, Vec<PinId>>>,
}

// 手动实现 Clone，因为 Mutex 不支持 Clone
impl Clone for ConnectionManager {
    fn clone(&self) -> Self {
        Self {
            connections: Mutex::new(self.connections.lock().unwrap().clone()),
            reverse_connections: Mutex::new(self.reverse_connections.lock().unwrap().clone()),
            pin_to_node: Mutex::new(self.pin_to_node.lock().unwrap().clone()),
            node_to_pins: Mutex::new(self.node_to_pins.lock().unwrap().clone()),
        }
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            reverse_connections: Mutex::new(HashMap::new()),
            pin_to_node: Mutex::new(HashMap::new()),
            node_to_pins: Mutex::new(HashMap::new()),
        }
    }

    /// 注册 Pin（建立 Pin 到 Node 的映射）
    pub fn register_pin(&self, pin_id: PinId, node_id: NodeId) {
        self.pin_to_node.lock().unwrap().insert(pin_id, node_id);
        self.node_to_pins
            .lock()
            .unwrap()
            .entry(node_id)
            .or_insert_with(Vec::new)
            .push(pin_id);
    }

    /// 连接两个 Pin（底层操作，调用前应先通过 ConnectionValidator 验证）
    ///
    /// 规则：
    /// - Input Pin: 最多 1 条输入边（自动断开旧连接）
    /// - Output Pin: 可以有多条输出边
    ///
    /// 返回被自动断开的旧连接（如有）
    pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Option<(PinId, PinId)> {
        // 如果 to_pin 已有连接，先断开（保证最多 1 条输入边）
        let auto_disconnected = self.get_upstream(to_pin).map(|old_from| {
            self.disconnect(old_from, to_pin);
            (old_from, to_pin)
        });

        // 建立新连接
        self.connections
            .lock()
            .unwrap()
            .entry(from_pin)
            .or_insert_with(Vec::new)
            .push(to_pin);

        self.reverse_connections
            .lock()
            .unwrap()
            .insert(to_pin, from_pin);

        auto_disconnected
    }

    /// 断开连接
    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) {
        if let Some(targets) = self.connections.lock().unwrap().get_mut(&from_pin) {
            targets.retain(|&p| p != to_pin);
        }
        self.reverse_connections.lock().unwrap().remove(&to_pin);
    }

    /// 断开 Pin 的输入连接
    pub fn disconnect_input(&self, pin: PinId) {
        if let Some(from_pin) = self.reverse_connections.lock().unwrap().remove(&pin) {
            if let Some(targets) = self.connections.lock().unwrap().get_mut(&from_pin) {
                targets.retain(|&p| p != pin);
            }
        }
    }

    /// 断开 Pin 的所有连接（输入和输出）
    pub fn disconnect_all(&self, pin: PinId) {
        // 断开输入
        self.disconnect_input(pin);

        // 断开所有输出
        if let Some(targets) = self.connections.lock().unwrap().remove(&pin) {
            let mut reverse = self.reverse_connections.lock().unwrap();
            for target in targets {
                reverse.remove(&target);
            }
        }
    }

    /// 获取 Pin 的下游连接
    pub fn get_downstream(&self, pin: PinId) -> Vec<PinId> {
        self.connections
            .lock()
            .unwrap()
            .get(&pin)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取 Pin 的上游连接
    pub fn get_upstream(&self, pin: PinId) -> Option<PinId> {
        self.reverse_connections.lock().unwrap().get(&pin).copied()
    }

    /// 获取节点的所有 Pin
    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<PinId> {
        self.node_to_pins
            .lock()
            .unwrap()
            .get(&node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取 Pin 所属的节点
    pub fn get_pin_node(&self, pin: PinId) -> Option<NodeId> {
        self.pin_to_node.lock().unwrap().get(&pin).copied()
    }

    /// 获取节点的直接上游节点
    pub fn get_upstream_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        let pins = self.get_node_pins(node_id);
        let mut upstream_nodes = HashSet::new();

        for pin in pins {
            if let Some(upstream_pin) = self.get_upstream(pin) {
                if let Some(upstream_node) = self.get_pin_node(upstream_pin) {
                    upstream_nodes.insert(upstream_node);
                }
            }
        }

        upstream_nodes.into_iter().collect()
    }

    /// 获取节点的直接下游节点
    pub fn get_downstream_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        let pins = self.get_node_pins(node_id);
        let mut downstream_nodes = HashSet::new();

        for pin in pins {
            for downstream_pin in self.get_downstream(pin) {
                if let Some(downstream_node) = self.get_pin_node(downstream_pin) {
                    downstream_nodes.insert(downstream_node);
                }
            }
        }

        downstream_nodes.into_iter().collect()
    }

    /// 检查是否会形成循环
    pub fn would_create_cycle(&self, from_pin: PinId, to_pin: PinId) -> bool {
        let from_node = match self.get_pin_node(from_pin) {
            Some(n) => n,
            None => return false,
        };

        let to_node = match self.get_pin_node(to_pin) {
            Some(n) => n,
            None => return false,
        };

        // 使用 DFS 检查从 to_node 是否能到达 from_node
        let mut visited = HashSet::new();
        let mut stack = vec![to_node];

        while let Some(node) = stack.pop() {
            if node == from_node {
                return true;
            }

            if visited.insert(node) {
                stack.extend(self.get_downstream_nodes(node));
            }
        }

        false
    }

    /// 获取所有连接
    pub fn all_connections(&self) -> Vec<Connection> {
        let connections = self.connections.lock().unwrap();
        let mut result = Vec::new();

        for (from_pin, to_pins) in connections.iter() {
            for to_pin in to_pins {
                result.push(Connection::new(*from_pin, *to_pin));
            }
        }

        result
    }

    /// 清除所有连接
    pub fn clear(&self) {
        self.connections.lock().unwrap().clear();
        self.reverse_connections.lock().unwrap().clear();
    }

    /// 移除节点的所有连接
    pub fn remove_node(&self, node_id: NodeId) {
        let pins = self.get_node_pins(node_id);
        for pin in pins {
            self.disconnect_all(pin);
        }
        self.node_to_pins.lock().unwrap().remove(&node_id);
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
