//! 图验证模块
//!
//! 提供图结构的静态分析和验证功能，包括循环检测

use std::collections::{HashMap, HashSet, VecDeque};

/// 图验证错误
#[derive(Debug, Clone)]
pub enum GraphValidationError {
    /// 检测到循环
    CycleDetected {
        /// 循环中的节点 ID 列表
        cycle_nodes: Vec<String>,
        /// 循环类型（exec 或 data）
        cycle_type: String,
    },
    /// 孤立节点（没有连接）
    IsolatedNodes {
        node_ids: Vec<String>,
    },
    /// 无效连接
    InvalidConnection {
        from_pin: String,
        to_pin: String,
        reason: String,
    },
}

/// 图验证结果
pub type ValidationResult = Result<(), Vec<GraphValidationError>>;

/// 图验证器
pub struct GraphValidator {
    /// 节点 ID 到节点信息的映射
    nodes: HashMap<String, NodeInfo>,
    /// 连接列表
    connections: Vec<ConnectionInfo>,
}

/// 节点信息
#[derive(Debug, Clone)]
struct NodeInfo {
    id: String,
    #[allow(dead_code)]
    node_type: String,
    inputs: Vec<PinInfo>,
    outputs: Vec<PinInfo>,
}

/// Pin 信息
#[derive(Debug, Clone)]
struct PinInfo {
    id: String,
    #[allow(dead_code)]
    name: String,
    pin_type: String, // "exec" 或数据类型
}

/// 连接信息
#[derive(Debug, Clone)]
struct ConnectionInfo {
    #[allow(dead_code)]
    id: String,
    source_pin: String,
    target_pin: String,
}

impl GraphValidator {
    /// 创建新的图验证器
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
        }
    }

    /// 从 GraphDto 创建验证器
    pub fn from_graph_dto(graph: &crate::executor::GraphDto) -> Self {
        let mut validator = Self::new();

        // 添加节点
        for node in &graph.nodes {
            let node_info = NodeInfo {
                id: node.id.clone(),
                node_type: node.node_type.clone(),
                inputs: node
                    .inputs
                    .iter()
                    .map(|p| PinInfo {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                    })
                    .collect(),
                outputs: node
                    .outputs
                    .iter()
                    .map(|p| PinInfo {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        pin_type: p.pin_type.clone(),
                    })
                    .collect(),
            };
            validator.nodes.insert(node.id.clone(), node_info);
        }

        // 添加连接
        for conn in &graph.connections {
            validator.connections.push(ConnectionInfo {
                id: conn.id.clone(),
                source_pin: conn.source_pin.clone(),
                target_pin: conn.target_pin.clone(),
            });
        }

        validator
    }

    /// 验证图结构
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();

        // 1. 检测 Exec Flow 循环
        if let Err(cycle_nodes) = self.detect_exec_cycle() {
            errors.push(GraphValidationError::CycleDetected {
                cycle_nodes,
                cycle_type: "exec".to_string(),
            });
        }

        // 2. 检测 Data Flow 循环
        if let Err(cycle_nodes) = self.detect_data_cycle() {
            errors.push(GraphValidationError::CycleDetected {
                cycle_nodes,
                cycle_type: "data".to_string(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 检测 Exec Flow 循环
    fn detect_exec_cycle(&self) -> Result<(), Vec<String>> {
        // 构建 Exec Flow 图（节点 -> 节点）
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for conn in &self.connections {
            // 检查是否是 exec 连接
            if let Some((from_node, from_pin)) = self.find_pin_node(&conn.source_pin) {
                if let Some((to_node, _to_pin)) = self.find_pin_node(&conn.target_pin) {
                    if from_pin.pin_type == "exec" {
                        graph
                            .entry(from_node.id.clone())
                            .or_insert_with(Vec::new)
                            .push(to_node.id.clone());
                    }
                }
            }
        }

        // 使用 DFS 检测循环
        self.detect_cycle_dfs(&graph)
    }

    /// 检测 Data Flow 循环
    fn detect_data_cycle(&self) -> Result<(), Vec<String>> {
        // 构建 Data Flow 图（节点 -> 节点）
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for conn in &self.connections {
            // 检查是否是数据连接
            if let Some((from_node, from_pin)) = self.find_pin_node(&conn.source_pin) {
                if let Some((to_node, _to_pin)) = self.find_pin_node(&conn.target_pin) {
                    if from_pin.pin_type != "exec" {
                        graph
                            .entry(from_node.id.clone())
                            .or_insert_with(Vec::new)
                            .push(to_node.id.clone());
                    }
                }
            }
        }

        // 使用 DFS 检测循环
        self.detect_cycle_dfs(&graph)
    }

    /// 使用 DFS 检测循环
    fn detect_cycle_dfs(&self, graph: &HashMap<String, Vec<String>>) -> Result<(), Vec<String>> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut cycle_path = Vec::new();

        for node_id in graph.keys() {
            if !visited.contains(node_id) {
                if self.dfs_visit(
                    node_id,
                    graph,
                    &mut visited,
                    &mut rec_stack,
                    &mut cycle_path,
                ) {
                    return Err(cycle_path);
                }
            }
        }

        Ok(())
    }

    /// DFS 访问节点
    fn dfs_visit(
        &self,
        node_id: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        cycle_path: &mut Vec<String>,
    ) -> bool {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());
        cycle_path.push(node_id.to_string());

        if let Some(neighbors) = graph.get(node_id) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if self.dfs_visit(neighbor, graph, visited, rec_stack, cycle_path) {
                        return true;
                    }
                } else if rec_stack.contains(neighbor) {
                    // 找到循环，添加循环的起始节点
                    cycle_path.push(neighbor.clone());
                    return true;
                }
            }
        }

        rec_stack.remove(node_id);
        cycle_path.pop();
        false
    }

    /// 查找 Pin 所属的节点
    fn find_pin_node(&self, pin_id: &str) -> Option<(&NodeInfo, &PinInfo)> {
        for node in self.nodes.values() {
            // 查找输入 Pin
            for pin in &node.inputs {
                if pin.id == pin_id {
                    return Some((node, pin));
                }
            }
            // 查找输出 Pin
            for pin in &node.outputs {
                if pin.id == pin_id {
                    return Some((node, pin));
                }
            }
        }
        None
    }

    /// 使用拓扑排序检测循环（备用方法）
    #[allow(dead_code)]
    fn topological_sort(&self, graph: &HashMap<String, Vec<String>>) -> Result<Vec<String>, Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // 初始化所有节点的入度
        for node_id in graph.keys() {
            in_degree.entry(node_id.clone()).or_insert(0);
        }

        // 计算入度
        for neighbors in graph.values() {
            for neighbor in neighbors {
                *in_degree.entry(neighbor.clone()).or_insert(0) += 1;
            }
        }

        // 找到所有入度为 0 的节点
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        // 拓扑排序
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.clone());

            if let Some(neighbors) = graph.get(&node_id) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        // 如果结果包含所有节点，则无循环
        if result.len() == graph.len() {
            Ok(result)
        } else {
            // 返回未访问的节点（循环中的节点）
            let unvisited: Vec<String> = graph
                .keys()
                .filter(|k| !result.contains(k))
                .cloned()
                .collect();
            Err(unvisited)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle() {
        let mut validator = GraphValidator::new();

        // 添加节点 A -> B -> C（无循环）
        validator.nodes.insert(
            "A".to_string(),
            NodeInfo {
                id: "A".to_string(),
                node_type: "test".to_string(),
                inputs: vec![],
                outputs: vec![PinInfo {
                    id: "A_out".to_string(),
                    name: "Out".to_string(),
                    pin_type: "exec".to_string(),
                }],
            },
        );

        validator.nodes.insert(
            "B".to_string(),
            NodeInfo {
                id: "B".to_string(),
                node_type: "test".to_string(),
                inputs: vec![PinInfo {
                    id: "B_in".to_string(),
                    name: "In".to_string(),
                    pin_type: "exec".to_string(),
                }],
                outputs: vec![PinInfo {
                    id: "B_out".to_string(),
                    name: "Out".to_string(),
                    pin_type: "exec".to_string(),
                }],
            },
        );

        validator.nodes.insert(
            "C".to_string(),
            NodeInfo {
                id: "C".to_string(),
                node_type: "test".to_string(),
                inputs: vec![PinInfo {
                    id: "C_in".to_string(),
                    name: "In".to_string(),
                    pin_type: "exec".to_string(),
                }],
                outputs: vec![],
            },
        );

        validator.connections.push(ConnectionInfo {
            id: "conn1".to_string(),
            source_pin: "A_out".to_string(),
            target_pin: "B_in".to_string(),
        });

        validator.connections.push(ConnectionInfo {
            id: "conn2".to_string(),
            source_pin: "B_out".to_string(),
            target_pin: "C_in".to_string(),
        });

        assert!(validator.validate().is_ok());
    }

    #[test]
    fn test_exec_cycle() {
        let mut validator = GraphValidator::new();

        // 添加节点 A -> B -> A（循环）
        validator.nodes.insert(
            "A".to_string(),
            NodeInfo {
                id: "A".to_string(),
                node_type: "test".to_string(),
                inputs: vec![PinInfo {
                    id: "A_in".to_string(),
                    name: "In".to_string(),
                    pin_type: "exec".to_string(),
                }],
                outputs: vec![PinInfo {
                    id: "A_out".to_string(),
                    name: "Out".to_string(),
                    pin_type: "exec".to_string(),
                }],
            },
        );

        validator.nodes.insert(
            "B".to_string(),
            NodeInfo {
                id: "B".to_string(),
                node_type: "test".to_string(),
                inputs: vec![PinInfo {
                    id: "B_in".to_string(),
                    name: "In".to_string(),
                    pin_type: "exec".to_string(),
                }],
                outputs: vec![PinInfo {
                    id: "B_out".to_string(),
                    name: "Out".to_string(),
                    pin_type: "exec".to_string(),
                }],
            },
        );

        validator.connections.push(ConnectionInfo {
            id: "conn1".to_string(),
            source_pin: "A_out".to_string(),
            target_pin: "B_in".to_string(),
        });

        validator.connections.push(ConnectionInfo {
            id: "conn2".to_string(),
            source_pin: "B_out".to_string(),
            target_pin: "A_in".to_string(),
        });

        let result = validator.validate();
        assert!(result.is_err());
        
        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            if let GraphValidationError::CycleDetected { cycle_nodes, cycle_type } = &errors[0] {
                assert_eq!(cycle_type, "exec");
                assert!(cycle_nodes.contains(&"A".to_string()));
                assert!(cycle_nodes.contains(&"B".to_string()));
            } else {
                panic!("Expected CycleDetected error");
            }
        }
    }
}
