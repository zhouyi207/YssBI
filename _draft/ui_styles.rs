//! UI 样式定义模块
//!
//! 定义节点的可视化样式，如 math 节点的紧凑样式、event 节点的特殊标题栏等。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UI 样式定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIStyleDefinition {
    /// 样式标识符 (如 "default", "math", "event", "compact")
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// 是否显示标题栏
    pub has_header: bool,
    /// 是否为紧凑模式
    pub compact: bool,
    /// 标题栏背景颜色 (可选)
    pub header_color: Option<String>,
    /// 节点背景颜色 (可选)
    pub background_color: Option<String>,
    /// 最小宽度 (像素)
    pub min_width: Option<u32>,
    /// 最小高度 (像素)
    pub min_height: Option<u32>,
    /// 中心符号映射 (node_type -> symbol)
    /// 例如: {"add": "+", "subtract": "-", "multiply": "×", "divide": "÷"}
    pub center_symbols: HashMap<String, String>,
}

/// 获取所有 UI 样式定义
pub fn get_ui_style_definitions() -> Vec<UIStyleDefinition> {
    vec![
        UIStyleDefinition {
            name: "default".into(),
            display_name: "默认样式".into(),
            has_header: true,
            compact: false,
            header_color: None,
            background_color: None,
            min_width: Some(150),
            min_height: None,
            center_symbols: HashMap::new(),
        },
        UIStyleDefinition {
            name: "event".into(),
            display_name: "事件样式".into(),
            has_header: true,
            compact: false,
            header_color: Some("#e06c75".into()), // 采用 Boolean 类型的红色，保持一致性
            background_color: None,
            min_width: Some(120),
            min_height: None,
            center_symbols: HashMap::new(),
        },
        UIStyleDefinition {
            name: "math".into(),
            display_name: "数学样式".into(),
            has_header: false,
            compact: true,
            header_color: None,
            background_color: Some("#2d2d2d".into()), // 与 nodeBase 保持一致
            min_width: Some(60),
            min_height: Some(60),
            center_symbols: {
                let mut m = HashMap::new();
                m.insert("add".into(), "+".into());
                m.insert("subtract".into(), "-".into());
                m.insert("multiply".into(), "×".into());
                m.insert("divide".into(), "÷".into());
                m.insert("modulo".into(), "%".into());
                m.insert("power".into(), "^".into());
                m
            },
        },
        UIStyleDefinition {
            name: "compact".into(),
            display_name: "紧凑样式".into(),
            has_header: true,
            compact: true,
            header_color: None,
            background_color: None,
            min_width: Some(100),
            min_height: None,
            center_symbols: HashMap::new(),
        },
        UIStyleDefinition {
            name: "branch".into(),
            display_name: "分支样式".into(),
            has_header: true,
            compact: false,
            header_color: Some("#666666".into()),
            background_color: None,
            min_width: Some(100),
            min_height: None,
            center_symbols: HashMap::new(),
        },
    ]
}

/// 根据名称获取样式定义
pub fn get_ui_style_by_name(name: &str) -> Option<UIStyleDefinition> {
    get_ui_style_definitions()
        .into_iter()
        .find(|s| s.name == name)
}

/// 获取节点的中心符号
pub fn get_center_symbol(style_name: &str, node_type: &str) -> Option<String> {
    get_ui_style_by_name(style_name).and_then(|style| style.center_symbols.get(node_type).cloned())
}
