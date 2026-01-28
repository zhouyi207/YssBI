//! 运行时图管理模块
//!
//! 负责管理整个节点图的运行时实例

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::executor::connection::ConnectionManager;
use crate::executor::error::{ExecutionError, ExecutionResult, NodeResult};
use crate::executor::node::{GenericNode, Node, NodeId};
use crate::executor::{GraphData, NodeData};

/// 运行时图
///
/// 管理所有节点实例和连接关系
pub struct RuntimeGraph {
    /// 所有节点（node_id -> Node）
    nodes: Mutex<HashMap<NodeId, Arc<Mutex<GenericNode>>>>,
    
    /// 连接管理器
    connection_manager: Arc<ConnectionManager>,
    
    /// 原始图数据到运行时节点的映射
    data_id_to_runtime_id: Mutex<HashMap<String, NodeId>>,
}

impl RuntimeGraph {
    /// 创建新的运行时图
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(HashMap::new()),
            connection_manager: Arc::new(ConnectionManager::new()),
            data_id_to_runtime_id: Mutex::new(HashMap::new()),
        }
    }

    /// 从 GraphData 实例化运行时图
    pub fn from_graph_data(graph_data: &GraphData) -> NodeResult<Self> {
        let runtime_graph = Self::new();

        // 1. 创建所有节点
        for node_data in &graph_data.nodes {
            runtime_graph.create_node_from_data(node_data)?;
        }

        // 2. 建立连接
        for node_data in &graph_data.nodes {
            runtime_graph.create_connections_from_data(node_data)?;
        }

        Ok(runtime_graph)
    }

    /// 从 NodeData 创建节点
    fn create_node_from_data(&self, node_data: &NodeData) -> NodeResult<NodeId> {
        use uuid::Uuid;
        
        let runtime_id = Uuid::new_v4();
        let node = GenericNode::new(runtime_id, &node_data.title, &node_data.node_type);

        // 创建输入 Pin
        for pin_data in &node_data.inputs {
            if pin_data.pin_type == "exec" {
                // 执行 Pin 暂时跳过，后续处理
                continue;
            }
            
            use crate::executor::pin::GenericInDataPin;
            let pin = GenericInDataPin::new(
                runtime_id,
                &pin_data.name,
                &pin_data.pin_type,
            );
            node.add_input(pin);
        }

        // 创建输出 Pin
        for pin_data in &node_data.outputs {
            if pin_data.pin_type == "exec" {
                // 执行 Pin 暂时跳过，后续处理
                continue;
            }
            
            use crate::executor::pin::GenericOutDataPin;
            let pin = GenericOutDataPin::new(
                runtime_id,
                &pin_data.name,
                &pin_data.pin_type,
            );
            node.add_output(pin);
        }

        // 注册节点到连接管理器
        self.connection_manager.register_node(&node)?;

        // 存储节点
        let node = Arc::new(Mutex::new(node));
        self.nodes.lock().unwrap().insert(runtime_id, node);

        // 映射原始 ID 到运行时 ID
        self.data_id_to_runtime_id
            .lock()
            .unwrap()
            .insert(node_data.id.clone(), runtime_id);

        Ok(runtime_id)
    }

    /// 从 NodeData 创建连接
    fn create_connections_from_data(&self, node_data: &NodeData) -> NodeResult<()> {
        // 获取当前节点的运行时 ID
        let to_runtime_id = *self
            .data_id_to_runtime_id
            .lock()
            .unwrap()
            .get(&node_data.id)
            .ok_or_else(|| crate::executor::error::NodeError::Generic(
                format!("节点不存在：{}", node_data.id)
            ))?;

        let nodes = self.nodes.lock().unwrap();
        let _to_node = nodes
            .get(&to_runtime_id)
            .ok_or_else(|| crate::executor::error::NodeError::NodeNotFound(to_runtime_id))?;

        // 处理每个输入 Pin 的连接
        for pin_data in &node_data.inputs {
            if pin_data.pin_type == "exec" {
                continue; // 执行 Pin 暂时跳过
            }

            // 简化实现：暂时跳过连接的实际建立
            // 完整实现需要维护 pin_data_id -> runtime_pin 的映射
            // 这将在后续的集成中完善
        }

        Ok(())
    }

    /// 添加节点
    pub fn add_node(&self, node: GenericNode) -> NodeResult<NodeId> {
        let id = node.id();
        self.connection_manager.register_node(&node)?;
        self.nodes
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(node)));
        Ok(id)
    }

    /// 获取节点
    pub fn get_node(&self, id: NodeId) -> Option<Arc<Mutex<GenericNode>>> {
        self.nodes.lock().unwrap().get(&id).cloned()
    }

    /// 移除节点
    pub fn remove_node(&self, id: NodeId) -> NodeResult<()> {
        self.nodes.lock().unwrap().remove(&id);
        Ok(())
    }

    /// 获取所有节点 ID
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.lock().unwrap().keys().copied().collect()
    }

    /// 获取连接管理器
    pub fn connection_manager(&self) -> &ConnectionManager {
        &self.connection_manager
    }

    /// 执行整个图
    pub fn execute(&self) -> ExecutionResult<()> {
        // 简化实现：按顺序执行所有节点
        // 实际应该进行拓扑排序
        let node_ids = self.node_ids();
        
        for node_id in node_ids {
            if let Some(node) = self.get_node(node_id) {
                node.lock().unwrap().execute()?;
            }
        }

        Ok(())
    }

    /// 执行指定节点
    pub fn execute_node(&self, node_id: NodeId) -> ExecutionResult<()> {
        let node_arc = self
            .get_node(node_id)
            .ok_or_else(|| ExecutionError::Generic(
                format!("节点不存在：{}", node_id)
            ))?;

        let mut node = node_arc.lock().unwrap();
        node.execute()
    }
}

impl std::fmt::Debug for RuntimeGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeGraph")
            .field("nodes", &format!("<{} nodes>", self.nodes.lock().unwrap().len()))
            .field("connection_manager", &self.connection_manager)
            .field("data_id_to_runtime_id", &format!("<{} mappings>", self.data_id_to_runtime_id.lock().unwrap().len()))
            .finish()
    }
}

impl Default for RuntimeGraph {
    fn default() -> Self {
        Self::new()
    }
}
