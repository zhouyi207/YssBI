//! 变量类型定义模块
//!
//! 定义所有可用的变量类型及其属性。

use serde::{Deserialize, Serialize};

/// 变量类型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableTypeDefinition {
    /// 类型标识符 (如 "int", "float", "string", "bool")
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// 对应的 Pin 类型
    pub pin_type: String,
    /// 默认值
    pub default_value: serde_json::Value,
    /// 编辑器控件类型
    pub editor_widget: EditorWidget,
    /// 类型颜色 (用于 UI)
    pub color: String,
    /// 是否支持数组
    pub supports_array: bool,
}

/// 编辑器控件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum EditorWidget {
    /// 数字输入框
    Number {
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
        precision: Option<u32>,
    },
    /// 文本输入框
    Text {
        multiline: bool,
        max_length: Option<usize>,
        placeholder: Option<String>,
    },
    /// 复选框
    Checkbox,
    /// 下拉选择
    Select { options: Vec<SelectOption> },
    /// 颜色选择器
    Color,
    /// JSON 编辑器 (用于 object/struct 类型)
    JsonEditor,
    /// 数组编辑器
    ArrayEditor { item_type: String },
}

/// 下拉选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: serde_json::Value,
    pub label: String,
}

/// 获取所有变量类型定义
pub fn get_variable_type_definitions() -> Vec<VariableTypeDefinition> {
    vec![
        VariableTypeDefinition {
            name: "bool".into(),
            display_name: "布尔".into(),
            pin_type: "bool".into(),
            default_value: serde_json::Value::Bool(false),
            editor_widget: EditorWidget::Checkbox,
            color: "#9D0006".into(),
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "int".into(),
            display_name: "整数".into(),
            pin_type: "int".into(),
            default_value: serde_json::json!(0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(1.0),
                precision: Some(0),
            },
            color: "#1C9898".into(),
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "float".into(),
            display_name: "浮点数".into(),
            pin_type: "float".into(),
            default_value: serde_json::json!(0.0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(0.1),
                precision: Some(3),
            },
            color: "#9ECD4D".into(),
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "string".into(),
            display_name: "字符串".into(),
            pin_type: "string".into(),
            default_value: serde_json::Value::String("".into()),
            editor_widget: EditorWidget::Text {
                multiline: false,
                max_length: None,
                placeholder: Some("输入文本...".into()),
            },
            color: "#FF00FF".into(),
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "object".into(),
            display_name: "对象".into(),
            pin_type: "object".into(),
            default_value: serde_json::Value::Null,
            editor_widget: EditorWidget::JsonEditor,
            color: "#0D7EA6".into(),
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "array".into(),
            display_name: "数组".into(),
            pin_type: "array".into(),
            default_value: serde_json::json!([]),
            editor_widget: EditorWidget::ArrayEditor {
                item_type: "object".into(),
            },
            color: "#FF7F00".into(),
            supports_array: false,
        },
    ]
}

/// 根据名称获取变量类型定义
pub fn get_variable_type_by_name(name: &str) -> Option<VariableTypeDefinition> {
    get_variable_type_definitions()
        .into_iter()
        .find(|t| t.name == name)
}

/// 验证变量值是否符合类型
pub fn validate_variable_value(type_name: &str, value: &serde_json::Value) -> bool {
    match type_name {
        "bool" => value.is_boolean(),
        "int" => value.is_i64() || value.is_u64(),
        "float" => value.is_f64() || value.is_i64() || value.is_u64(),
        "string" => value.is_string(),
        "object" => value.is_object() || value.is_null(),
        "array" => value.is_array(),
        _ => true, // 未知类型默认通过
    }
}
