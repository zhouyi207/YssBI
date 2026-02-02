//! 项目序列化和反序列化工具
//!
//! 提供子图和节点的序列化/反序列化功能

use super::{
    CanvasState, PinDefDto, Position, SerializedNode, SerializedPin, SubGraphData,
    SubGraphType,
};
use crate::executor::node::Node;
use crate::executor::GenericNode;
use crate::schema::VariableDefinition;
use std::collections::HashMap;

// ==================== 序列化 ====================

/// 将子图序列化为 SubGraphData
///
/// # 参数
/// - `id`: 子图 ID
/// - `name`: 子图名称
/// - `sub_type`: 子图类型 (event, function, macro)
/// - `nodes`: 节点列表
/// - `canvas`: 画布状态
/// - `variables`: 变量定义
/// - `inputs`: 输入参数定义（仅用于 function/macro）
/// - `outputs`: 输出参数定义（仅用于 function/macro）
pub fn serialize_subgraph(
    id: String,
    name: String,
    sub_type: SubGraphType,
    nodes: Vec<SerializedNode>,
    canvas: CanvasState,
    variables: HashMap<String, VariableDefinition>,
    inputs: Vec<PinDefDto>,
    outputs: Vec<PinDefDto>,
) -> SubGraphData {
    SubGraphData {
        id,
        name,
        sub_type,
        nodes,
        canvas,
        variables,
        inputs,
        outputs,
    }
}

/// 从运行时节点创建序列化节点
///
/// 将 GenericNode 转换为可序列化的 SerializedNode
pub fn serialize_node_from_runtime(
    node: &GenericNode,
    position: Position,
) -> SerializedNode {
    let inputs: Vec<SerializedPin> = node
        .inputs()
        .iter()
        .map(|pin| SerializedPin {
            id: pin.id().to_string(),
            name: pin.name().to_string(),
            pin_type: pin.data_type().to_string(),
            links: vec![],
            default_value: None,
            user_value: None,
            is_array: false,
        })
        .collect();

    let outputs: Vec<SerializedPin> = node
        .outputs()
        .iter()
        .map(|pin| SerializedPin {
            id: pin.id().to_string(),
            name: pin.name().to_string(),
            pin_type: pin.data_type().to_string(),
            links: vec![],
            default_value: None,
            user_value: None,
            is_array: false,
        })
        .collect();

    SerializedNode {
        id: node.id().to_string(),
        node_type: node.node_type().to_string(),
        title: node.name().to_string(),
        position,
        is_internal: false,
        variable_id: node.variable_id(),
        variable_type: None,
        variable_name: None,
        sub_graph_id: None,
        inputs,
        outputs,
        dynamic_pins: None,
    }
}

/// 从前端节点数据创建序列化节点
///
/// 用于从前端接收的数据转换为后端格式
pub fn serialize_node_from_frontend(
    id: String,
    node_type: String,
    title: String,
    position: Position,
    is_internal: bool,
    variable_id: Option<String>,
    variable_type: Option<String>,
    variable_name: Option<String>,
    sub_graph_id: Option<String>,
    inputs: Vec<SerializedPin>,
    outputs: Vec<SerializedPin>,
) -> SerializedNode {
    SerializedNode {
        id,
        node_type,
        title,
        position,
        is_internal,
        variable_id,
        variable_type,
        variable_name,
        sub_graph_id,
        inputs,
        outputs,
        dynamic_pins: None,
    }
}

// ==================== 反序列化 ====================

/// 反序列化子图数据
///
/// 将 SubGraphData 解析为可用的组件
///
/// # 返回
/// - `nodes`: 序列化的节点列表
/// - `canvas`: 画布状态
/// - `variables`: 变量定义
/// - `inputs`: 输入参数定义
/// - `outputs`: 输出参数定义
pub fn deserialize_subgraph(
    data: &SubGraphData,
) -> (
    Vec<SerializedNode>,
    CanvasState,
    HashMap<String, VariableDefinition>,
    Vec<PinDefDto>,
    Vec<PinDefDto>,
) {
    let nodes = data.nodes.clone();
    let canvas = data.canvas.clone();
    let variables = data.variables.clone();
    let inputs = data.inputs.clone();
    let outputs = data.outputs.clone();

    (nodes, canvas, variables, inputs, outputs)
}

/// 验证节点的变量引用
///
/// 检查节点引用的变量是否存在
pub fn validate_node_variables(
    node: &SerializedNode,
    variables: &HashMap<String, VariableDefinition>,
    subgraph_name: &str,
) -> Result<(), String> {
    if let Some(var_id) = &node.variable_id {
        if !variables.contains_key(var_id) {
            return Err(format!(
                "Node {} in {} refers to missing variable {}",
                node.id, subgraph_name, var_id
            ));
        }
    }
    Ok(())
}

/// 验证子图的完整性
///
/// 检查子图中的所有节点和连接是否有效
pub fn validate_subgraph(data: &SubGraphData) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // 验证节点的变量引用
    for node in &data.nodes {
        if let Err(e) = validate_node_variables(node, &data.variables, &data.name) {
            errors.push(e);
        }
    }

    // 验证连接
    let _node_ids: std::collections::HashSet<_> = data.nodes.iter().map(|n| &n.id).collect();
    let pin_ids: std::collections::HashSet<_> = data
        .nodes
        .iter()
        .flat_map(|n| {
            n.inputs
                .iter()
                .chain(n.outputs.iter())
                .map(|p| p.id.as_str())
        })
        .collect();

    for node in &data.nodes {
        // 检查输出 Pin 的连接
        for output in &node.outputs {
            for link in &output.links {
                if !pin_ids.contains(link.as_str()) {
                    errors.push(format!(
                        "Node {} output pin {} links to non-existent pin {}",
                        node.id, output.id, link
                    ));
                }
            }
        }

        // 检查子图引用
        if let Some(sub_graph_id) = &node.sub_graph_id {
            // 注意：这里无法验证子图是否存在，需要在更高层级验证
            if sub_graph_id.is_empty() {
                errors.push(format!(
                    "Node {} has empty subGraphId",
                    node.id
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ==================== 辅助函数 ====================

/// 创建默认画布状态
pub fn default_canvas_state() -> CanvasState {
    CanvasState {
        x: 0.0,
        y: 0.0,
        scale: 1.0,
    }
}

/// 创建默认位置
pub fn default_position() -> Position {
    Position { x: 0.0, y: 0.0 }
}

/// 从字符串解析子图类型
pub fn parse_subgraph_type(s: &str) -> Result<SubGraphType, String> {
    match s.to_lowercase().as_str() {
        "event" => Ok(SubGraphType::Event),
        "function" => Ok(SubGraphType::Function),
        "macro" => Ok(SubGraphType::Macro),
        _ => Err(format!("Invalid subgraph type: {}", s)),
    }
}

/// 将子图类型转换为字符串
pub fn subgraph_type_to_string(t: &SubGraphType) -> &'static str {
    match t {
        SubGraphType::Event => "event",
        SubGraphType::Function => "function",
        SubGraphType::Macro => "macro",
    }
}

// ==================== 批量操作 ====================

/// 批量序列化节点
pub fn serialize_nodes(
    nodes: Vec<(GenericNode, Position)>,
) -> Vec<SerializedNode> {
    nodes
        .iter()
        .map(|(node, pos)| serialize_node_from_runtime(node, pos.clone()))
        .collect()
}

/// 从序列化数据中提取所有 Pin ID
pub fn extract_all_pin_ids(nodes: &[SerializedNode]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|n| {
            n.inputs
                .iter()
                .chain(n.outputs.iter())
                .map(|p| p.id.clone())
        })
        .collect()
}

/// 从序列化数据中提取所有节点 ID
pub fn extract_all_node_ids(nodes: &[SerializedNode]) -> Vec<String> {
    nodes.iter().map(|n| n.id.clone()).collect()
}

/// 查找节点的所有连接
pub fn find_node_connections(node: &SerializedNode) -> Vec<(String, String)> {
    let mut connections = Vec::new();
    
    for output in &node.outputs {
        for link in &output.links {
            connections.push((output.id.clone(), link.clone()));
        }
    }
    
    connections
}

/// 统计子图信息
#[derive(Debug, Clone)]
pub struct SubGraphStats {
    pub node_count: usize,
    pub connection_count: usize,
    pub variable_count: usize,
    pub input_count: usize,
    pub output_count: usize,
}

pub fn get_subgraph_stats(data: &SubGraphData) -> SubGraphStats {
    let connection_count = data
        .nodes
        .iter()
        .flat_map(|n| &n.outputs)
        .map(|p| p.links.len())
        .sum();

    SubGraphStats {
        node_count: data.nodes.len(),
        connection_count,
        variable_count: data.variables.len(),
        input_count: data.inputs.len(),
        output_count: data.outputs.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subgraph_type() {
        assert_eq!(
            parse_subgraph_type("event").unwrap(),
            SubGraphType::Event
        );
        assert_eq!(
            parse_subgraph_type("function").unwrap(),
            SubGraphType::Function
        );
        assert_eq!(
            parse_subgraph_type("macro").unwrap(),
            SubGraphType::Macro
        );
        assert!(parse_subgraph_type("invalid").is_err());
    }

    #[test]
    fn test_subgraph_type_to_string() {
        assert_eq!(subgraph_type_to_string(&SubGraphType::Event), "event");
        assert_eq!(
            subgraph_type_to_string(&SubGraphType::Function),
            "function"
        );
        assert_eq!(subgraph_type_to_string(&SubGraphType::Macro), "macro");
    }

    #[test]
    fn test_default_canvas_state() {
        let canvas = default_canvas_state();
        assert_eq!(canvas.x, 0.0);
        assert_eq!(canvas.y, 0.0);
        assert_eq!(canvas.scale, 1.0);
    }

    #[test]
    fn test_extract_pin_ids() {
        let node = SerializedNode {
            id: "node1".to_string(),
            node_type: "test".to_string(),
            title: "Test".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            is_internal: false,
            variable_id: None,
            variable_type: None,
            variable_name: None,
            sub_graph_id: None,
            inputs: vec![SerializedPin {
                id: "in1".to_string(),
                name: "Input".to_string(),
                pin_type: "number".to_string(),
                links: vec![],
                default_value: None,
                user_value: None,
                is_array: false,
            }],
            outputs: vec![SerializedPin {
                id: "out1".to_string(),
                name: "Output".to_string(),
                pin_type: "number".to_string(),
                links: vec![],
                default_value: None,
                user_value: None,
                is_array: false,
            }],
            dynamic_pins: None,
        };

        let pin_ids = extract_all_pin_ids(&vec![node]);
        assert_eq!(pin_ids.len(), 2);
        assert!(pin_ids.contains(&"in1".to_string()));
        assert!(pin_ids.contains(&"out1".to_string()));
    }
}
