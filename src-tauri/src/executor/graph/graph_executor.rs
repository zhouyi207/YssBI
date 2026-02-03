//! Graph 执行器
//!
//! Executor 不访问 Node 内部字段，不持有 Pin 实例。
//! Executor 只通过 Graph 查询 Pin 值、连接关系和下一个可执行 NodeId。

use super::Graph;
use crate::executor::node::{NodeExecutionContext, NodeProcessor};
use crate::executor::pin::NodeId;
use std::collections::{HashSet, VecDeque};

/// Graph 执行器
pub struct GraphExecutor {
    /// 执行日志
    logs: Vec<String>,
}

impl GraphExecutor {
    pub fn new() -> Self {
        Self { logs: Vec::new() }
    }

    /// 执行整个 Graph
    pub fn execute(&mut self, graph: &Graph) -> Result<(), String> {
        self.logs.clear();
        self.log("Starting graph execution");

        // 查找入口节点（没有输入执行 Pin 或输入执行 Pin 未连接的节点）
        let entry_nodes = self.find_entry_nodes(graph)?;
        
        if entry_nodes.is_empty() {
            return Err("No entry nodes found".to_string());
        }

        self.log(format!("Found {} entry nodes", entry_nodes.len()));

        // 从每个入口节点开始执行
        for entry_node in entry_nodes {
            self.execute_from_node(graph, entry_node)?;
        }

        self.log("Graph execution completed");
        Ok(())
    }

    /// 从指定节点开始执行
    fn execute_from_node(&mut self, graph: &Graph, start_node: NodeId) -> Result<(), String> {
        let mut queue = VecDeque::new();
        let mut executed = HashSet::new();

        queue.push_back(start_node);

        while let Some(node_id) = queue.pop_front() {
            if executed.contains(&node_id) {
                continue;
            }

            self.log(format!("Executing node {:?}", node_id));

            // 执行节点
            let next_nodes = self.execute_node(graph, node_id)?;
            executed.insert(node_id);

            // 将下游节点加入队列
            queue.extend(next_nodes);
        }

        Ok(())
    }

    /// 执行单个节点
    fn execute_node(&mut self, graph: &Graph, node_id: NodeId) -> Result<Vec<NodeId>, String> {
        // 获取节点定义
        let definition = graph
            .get_node_definition(node_id)
            .ok_or_else(|| format!("Node {:?} definition not found", node_id))?;

        // 获取节点的所有 Pin
        let pins = graph.get_node_pins(node_id);

        // 构建执行上下文
        let mut context = NodeExecutionContext::new();

        // 收集输入值（按 Role）
        for pin in &pins {
            if pin.is_input() && pin.is_data() {
                // 获取输入值（从上游或用户值）
                let value = if let Some(upstream_pin) = graph.connections().get_upstream(pin.id) {
                    // 从上游 Pin 获取值
                    graph.get_pin_value(upstream_pin)
                } else {
                    // 使用 Pin 自己的值（用户值或默认值）
                    pin.effective_value().cloned()
                };

                if let Some(value) = value {
                    context.add_input(pin.role.clone(), value);
                }
            }
        }

        // 执行处理器
        let next_exec_role = if let Some(processor) = &definition.processor {
            match processor {
                NodeProcessor::Data(func) => {
                    func(&mut context)?;
                    None
                }
                NodeProcessor::Flow(func) => {
                    let role = func(&mut context)?;
                    Some(role)
                }
                NodeProcessor::Hybrid(func) => func(&mut context)?,
            }
        } else {
            None
        };

        // 将输出值写回 Graph
        for (role, value) in context.outputs() {
            // 查找对应的输出 Pin
            if let Some(pin) = pins.iter().find(|p| p.is_output() && p.is_data() && &p.role == role) {
                graph.set_pin_value(pin.id, value.clone())?;
            }
        }

        // 确定下一个要执行的节点
        let mut next_nodes = Vec::new();

        if let Some(exec_role) = next_exec_role {
            // 通过执行流确定下一个节点
            if let Some(exec_pin) = pins.iter().find(|p| p.is_output() && p.is_exec() && p.role == exec_role) {
                // 获取连接到这个执行 Pin 的下游节点
                for downstream_pin in graph.connections().get_downstream(exec_pin.id) {
                    if let Some(downstream_node) = graph.connections().get_pin_node(downstream_pin) {
                        next_nodes.push(downstream_node);
                    }
                }
            }
        } else {
            // 数据节点：触发所有下游节点
            next_nodes = graph.connections().get_downstream_nodes(node_id);
        }

        Ok(next_nodes)
    }

    /// 查找入口节点
    fn find_entry_nodes(&self, graph: &Graph) -> Result<Vec<NodeId>, String> {
        let mut entry_nodes = Vec::new();

        for node in graph.nodes() {
            let pins = graph.get_node_pins(node.id);
            
            // 检查是否有输入执行 Pin
            let has_exec_input = pins.iter().any(|p| p.is_input() && p.is_exec());

            if !has_exec_input {
                // 没有执行输入的节点是潜在的入口节点
                entry_nodes.push(node.id);
            } else {
                // 检查执行输入是否有连接
                let exec_input_connected = pins
                    .iter()
                    .filter(|p| p.is_input() && p.is_exec())
                    .any(|p| graph.connections().get_upstream(p.id).is_some());

                if !exec_input_connected {
                    entry_nodes.push(node.id);
                }
            }
        }

        Ok(entry_nodes)
    }

    /// 记录日志
    fn log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
    }

    /// 获取执行日志
    pub fn logs(&self) -> &[String] {
        &self.logs
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}
