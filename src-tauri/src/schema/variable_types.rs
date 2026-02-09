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
            display_name: "Boolean".into(),
            pin_type: "bool".into(),
            default_value: serde_json::Value::Bool(false),
            editor_widget: EditorWidget::Checkbox,
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "int32".into(),
            display_name: "Int32".into(),
            pin_type: "int32".into(),
            default_value: serde_json::json!(0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(1.0),
                precision: Some(0),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "int64".into(),
            display_name: "Int64".into(),
            pin_type: "int64".into(),
            default_value: serde_json::json!(0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(1.0),
                precision: Some(0),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "float32".into(),
            display_name: "Float32".into(),
            pin_type: "float32".into(),
            default_value: serde_json::json!(0.0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(0.1),
                precision: Some(3),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "float64".into(),
            display_name: "Float64".into(),
            pin_type: "float64".into(),
            default_value: serde_json::json!(0.0),
            editor_widget: EditorWidget::Number {
                min: None,
                max: None,
                step: Some(0.1),
                precision: Some(3),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "string".into(),
            display_name: "String".into(),
            pin_type: "string".into(),
            default_value: serde_json::Value::String("".into()),
            editor_widget: EditorWidget::Text {
                multiline: false,
                max_length: None,
                placeholder: Some("Input text...".into()),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "date".into(),
            display_name: "Date".into(),
            pin_type: "date".into(),
            default_value: serde_json::Value::Null,
            editor_widget: EditorWidget::Text {
                multiline: false,
                max_length: None,
                placeholder: Some("YYYY-MM-DD".into()),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "datetime".into(),
            display_name: "DateTime".into(),
            pin_type: "datetime".into(),
            default_value: serde_json::Value::Null,
            editor_widget: EditorWidget::Text {
                multiline: false,
                max_length: None,
                placeholder: Some("YYYY-MM-DD HH:mm:ss".into()),
            },
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "object".into(),
            display_name: "Object".into(),
            pin_type: "object".into(),
            default_value: serde_json::Value::Null,
            editor_widget: EditorWidget::JsonEditor,
            supports_array: true,
        },
        VariableTypeDefinition {
            name: "dataframe".into(),
            display_name: "DataFrame".into(),
            pin_type: "dataframe".into(),
            default_value: serde_json::Value::Null,
            editor_widget: EditorWidget::JsonEditor,
            supports_array: false,
        },
        // 兼容旧版数组
        VariableTypeDefinition {
            name: "array".into(),
            display_name: "Array (Legacy)".into(),
            pin_type: "array".into(),
            default_value: serde_json::json!([]),
            editor_widget: EditorWidget::ArrayEditor {
                item_type: "object".into(),
            },
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
        "int8" | "int16" | "int32" | "int64" | "int" | "uint32" | "uint64" => {
            value.is_i64() || value.is_u64()
        }
        "float32" | "float64" | "float" => value.is_f64() || value.is_i64() || value.is_u64(),
        "string" => value.is_string(),
        "date" | "datetime" => value.is_string() || value.is_null(),
        "object" => value.is_object() || value.is_null(),
        "dataframe" => value.is_string() || value.is_null(), // DataFrame 传递的是 ID 字符串
        "array" => value.is_array(),
        _ => true, // 未知类型默认通过
    }
}
