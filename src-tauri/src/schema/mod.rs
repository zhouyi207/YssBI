//! Schema 模块
//!
//! 包含所有类型定义、分类、样式和验证规则。
//! 这些定义作为系统的权威数据源，前端从这里获取所有元数据。

pub mod categories;
pub mod pin_types;
pub mod ui_styles;
pub mod validation;
pub mod variable_types;

// 重新导出常用类型
pub use categories::{get_category_definitions, CategoryDefinition};
pub use pin_types::{
    can_connect, check_type_compatibility, get_pin_type_definitions, PinTypeDefinition,
    TypeConversion,
};
pub use ui_styles::{get_center_symbol, get_ui_style_definitions, UIStyleDefinition};
pub use validation::{
    get_graph_validation_rules, get_node_validation_rules, GraphValidationRule,
    NodeValidationRule, ValidationLevel, ValidationMessage, ValidationResult,
};
pub use variable_types::{get_variable_type_definitions, VariableTypeDefinition};

use serde::Serialize;

/// 完整的 Schema 数据，用于一次性传输给前端
#[derive(Debug, Clone, Serialize)]
pub struct EditorSchema {
    pub pin_types: Vec<PinTypeDefinition>,
    pub categories: Vec<CategoryDefinition>,
    pub ui_styles: Vec<UIStyleDefinition>,
    pub variable_types: Vec<VariableTypeDefinition>,
    pub node_validation_rules: Vec<NodeValidationRule>,
    pub graph_validation_rules: Vec<GraphValidationRule>,
}

/// 获取完整的 Schema
pub fn get_editor_schema() -> EditorSchema {
    EditorSchema {
        pin_types: get_pin_type_definitions(),
        categories: get_category_definitions(),
        ui_styles: get_ui_style_definitions(),
        variable_types: get_variable_type_definitions(),
        node_validation_rules: get_node_validation_rules(),
        graph_validation_rules: get_graph_validation_rules(),
    }
}
