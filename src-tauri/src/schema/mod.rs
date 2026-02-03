//! Schema 模块 - 提供节点定义和类型信息

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSchema {
    pub pin_types: Vec<PinTypeDefinition>,
    pub categories: Vec<CategoryDefinition>,
    pub ui_styles: Vec<UIStyleDefinition>,
    pub variable_types: Vec<VariableTypeDefinition>,
    pub node_validation_rules: Vec<NodeValidationRule>,
    pub graph_validation_rules: Vec<GraphValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinTypeDefinition {
    pub name: String,
    pub display_name: String,
    pub is_exec: bool,
    pub supports_array: bool,
    pub implicit_convert_to: Vec<String>,
    pub explicit_convert_to: Vec<String>,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryDefinition {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub visible_in_palette: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIStyleDefinition {
    pub name: String,
    pub display_name: String,
    pub has_header: bool,
    pub compact: bool,
    pub header_color: Option<String>,
    pub background_color: Option<String>,
    pub min_width: Option<i32>,
    pub min_height: Option<i32>,
    pub center_symbols: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableTypeDefinition {
    pub name: String,
    pub display_name: String,
    pub pin_type: String,
    pub default_value: Value,
    pub editor_widget: Value,
    pub supports_array: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeValidationRule {
    pub node_type: String,
    pub input_rules: Vec<PinValidationRule>,
    pub output_rules: Vec<PinValidationRule>,
    pub custom_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinValidationRule {
    pub pin_name: String,
    pub required: bool,
    pub min_connections: i32,
    pub max_connections: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphValidationRule {
    pub name: String,
    pub description: String,
    pub level: String,
    pub rule_type: Value,
}

impl Default for EditorSchema {
    fn default() -> Self {
        Self::create_default()
    }
}

impl EditorSchema {
    pub fn create_default() -> Self {
        Self {
            pin_types: Self::default_pin_types(),
            categories: Self::default_categories(),
            ui_styles: Self::default_ui_styles(),
            variable_types: Self::default_variable_types(),
            node_validation_rules: vec![],
            graph_validation_rules: vec![],
        }
    }

    fn default_pin_types() -> Vec<PinTypeDefinition> {
        vec![
            PinTypeDefinition {
                name: "exec".to_string(),
                display_name: "Exec".to_string(),
                is_exec: true,
                supports_array: false,
                implicit_convert_to: vec![],
                explicit_convert_to: vec![],
                default_value: None,
            },
            PinTypeDefinition {
                name: "Boolean".to_string(),
                display_name: "Boolean".to_string(),
                is_exec: false,
                supports_array: true,
                implicit_convert_to: vec![],
                explicit_convert_to: vec!["String".to_string()],
                default_value: Some(Value::Bool(false)),
            },
            PinTypeDefinition {
                name: "Int32".to_string(),
                display_name: "Int32".to_string(),
                is_exec: false,
                supports_array: true,
                implicit_convert_to: vec!["Int64".to_string(), "Float32".to_string(), "Float64".to_string()],
                explicit_convert_to: vec!["String".to_string()],
                default_value: Some(Value::Number(0.into())),
            },
            PinTypeDefinition {
                name: "Float64".to_string(),
                display_name: "Float64".to_string(),
                is_exec: false,
                supports_array: true,
                implicit_convert_to: vec![],
                explicit_convert_to: vec!["String".to_string()],
                default_value: Some(Value::Number(serde_json::Number::from_f64(0.0).unwrap())),
            },
            PinTypeDefinition {
                name: "String".to_string(),
                display_name: "String".to_string(),
                is_exec: false,
                supports_array: true,
                implicit_convert_to: vec![],
                explicit_convert_to: vec![],
                default_value: Some(Value::String("".to_string())),
            },
            PinTypeDefinition {
                name: "Object".to_string(),
                display_name: "Object".to_string(),
                is_exec: false,
                supports_array: true,
                implicit_convert_to: vec![],
                explicit_convert_to: vec![],
                default_value: Some(Value::Object(Default::default())),
            },
        ]
    }

    fn default_categories() -> Vec<CategoryDefinition> {
        vec![
            CategoryDefinition {
                name: "Math".to_string(),
                display_name: "Math".to_string(),
                description: Some("Mathematical operations".to_string()),
                sort_order: 0,
                color: Some("#4CAF50".to_string()),
                icon: Some("calculate".to_string()),
                visible_in_palette: true,
            },
            CategoryDefinition {
                name: "Control Flow".to_string(),
                display_name: "Control Flow".to_string(),
                description: Some("Control flow nodes".to_string()),
                sort_order: 1,
                color: Some("#2196F3".to_string()),
                icon: Some("alt_route".to_string()),
                visible_in_palette: true,
            },
        ]
    }

    fn default_ui_styles() -> Vec<UIStyleDefinition> {
        vec![
            UIStyleDefinition {
                name: "default".to_string(),
                display_name: "Default".to_string(),
                has_header: true,
                compact: false,
                header_color: None,
                background_color: None,
                min_width: Some(120),
                min_height: Some(60),
                center_symbols: Default::default(),
            },
            UIStyleDefinition {
                name: "math".to_string(),
                display_name: "Math".to_string(),
                has_header: true,
                compact: false,
                header_color: Some("#4CAF50".to_string()),
                background_color: None,
                min_width: Some(100),
                min_height: Some(50),
                center_symbols: [
                    ("add".to_string(), "+".to_string()),
                    ("subtract".to_string(), "-".to_string()),
                    ("multiply".to_string(), "×".to_string()),
                    ("divide".to_string(), "÷".to_string()),
                ].iter().cloned().collect(),
            },
        ]
    }

    fn default_variable_types() -> Vec<VariableTypeDefinition> {
        vec![
            VariableTypeDefinition {
                name: "number".to_string(),
                display_name: "Number".to_string(),
                pin_type: "Float64".to_string(),
                default_value: Value::Number(serde_json::Number::from_f64(0.0).unwrap()),
                editor_widget: serde_json::json!({
                    "type": "Number",
                    "config": {}
                }),
                supports_array: true,
            },
            VariableTypeDefinition {
                name: "string".to_string(),
                display_name: "String".to_string(),
                pin_type: "String".to_string(),
                default_value: Value::String("".to_string()),
                editor_widget: serde_json::json!({
                    "type": "Text",
                    "config": { "multiline": false }
                }),
                supports_array: true,
            },
        ]
    }
}

