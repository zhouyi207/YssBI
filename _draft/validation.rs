//! 验证规则定义模块
//!
//! 定义图验证规则，如必须连接的针脚、节点约束等。

use serde::{Deserialize, Serialize};

/// 针脚验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinValidationRule {
    /// 针脚名称
    pub pin_name: String,
    /// 是否必须连接
    pub required: bool,
    /// 最小连接数 (默认为 0)
    pub min_connections: u32,
    /// 最大连接数 (None 表示无限制)
    pub max_connections: Option<u32>,
}

/// 节点验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeValidationRule {
    /// 节点类型
    pub node_type: String,
    /// 输入针脚规则
    pub input_rules: Vec<PinValidationRule>,
    /// 输出针脚规则
    pub output_rules: Vec<PinValidationRule>,
    /// 自定义验证消息 (可选)
    pub custom_message: Option<String>,
}

/// 图验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphValidationRule {
    /// 规则名称
    pub name: String,
    /// 规则描述
    pub description: String,
    /// 规则级别
    pub level: ValidationLevel,
    /// 规则类型
    pub rule_type: GraphRuleType,
}

/// 验证级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationLevel {
    /// 错误 - 阻止执行
    Error,
    /// 警告 - 允许执行但提示
    Warning,
    /// 信息 - 仅供参考
    Info,
}

/// 图规则类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GraphRuleType {
    /// 必须有入口节点
    RequireEntryNode { entry_node_types: Vec<String> },
    /// 禁止循环引用
    NoCycles,
    /// 所有 exec 路径必须终止
    AllPathsTerminate,
    /// 不允许悬空的数据针脚 (未使用的输出)
    NoUnusedOutputs { excluded_types: Vec<String> },
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// 是否通过验证
    pub valid: bool,
    /// 验证消息列表
    pub messages: Vec<ValidationMessage>,
}

/// 验证消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMessage {
    /// 消息级别
    pub level: ValidationLevel,
    /// 消息内容
    pub message: String,
    /// 相关节点 ID (可选)
    pub node_id: Option<String>,
    /// 相关针脚 ID (可选)
    pub pin_id: Option<String>,
}

/// 获取所有节点验证规则
pub fn get_node_validation_rules() -> Vec<NodeValidationRule> {
    vec![
        // Print 节点：In 和 Value 必须连接
        NodeValidationRule {
            node_type: "print".into(),
            input_rules: vec![PinValidationRule {
                pin_name: "In".into(),
                required: true,
                min_connections: 1,
                max_connections: Some(1),
            }],
            output_rules: vec![],
            custom_message: None,
        },
        // Branch 节点：条件必须连接
        NodeValidationRule {
            node_type: "if_else".into(),
            input_rules: vec![PinValidationRule {
                pin_name: "In".into(),
                required: true,
                min_connections: 1,
                max_connections: Some(1),
            }],
            output_rules: vec![],
            custom_message: None,
        },
        // Set Variable：必须有变量 ID
        NodeValidationRule {
            node_type: "set_variable".into(),
            input_rules: vec![PinValidationRule {
                pin_name: "In".into(),
                required: true,
                min_connections: 1,
                max_connections: Some(1),
            }],
            output_rules: vec![],
            custom_message: Some("必须关联一个变量".into()),
        },
    ]
}

/// 获取所有图验证规则
pub fn get_graph_validation_rules() -> Vec<GraphValidationRule> {
    vec![
        GraphValidationRule {
            name: "require_entry".into(),
            description: "图必须有入口节点".into(),
            level: ValidationLevel::Error,
            rule_type: GraphRuleType::RequireEntryNode {
                entry_node_types: vec![
                    "event_on_run".into(),
                    "function_entry".into(),
                    "macro_inputs".into(),
                ],
            },
        },
        GraphValidationRule {
            name: "no_cycles".into(),
            description: "执行路径不允许循环引用".into(),
            level: ValidationLevel::Warning,
            rule_type: GraphRuleType::NoCycles,
        },
    ]
}

/// 获取指定节点类型的验证规则
pub fn get_validation_rule_for_node(node_type: &str) -> Option<NodeValidationRule> {
    get_node_validation_rules()
        .into_iter()
        .find(|r| r.node_type == node_type)
}
