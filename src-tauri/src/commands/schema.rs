//! Schema 相关命令

use crate::executor::GenericNode;
use crate::schema::{
    get_editor_schema, CategoryDefinition, EditorSchema, PinTypeDefinition, UIStyleDefinition,
    VariableTypeDefinition,
};
use std::sync::Arc;
use tauri_plugin_log::log::info;

/// 获取所有节点定义
#[tauri::command]
pub fn get_node_definitions() -> Vec<Arc<GenericNode>> {
    let defs = crate::executor::get_all_node_definitions();
    info!("[Backend] Returning {} node definitions", defs.len());
    defs
}

/// 获取完整的编辑器 Schema（一次性获取所有元数据）
#[tauri::command]
pub fn get_editor_schema_command() -> EditorSchema {
    get_editor_schema()
}

/// 获取所有 Pin 类型定义
#[tauri::command]
pub fn get_pin_types() -> Vec<PinTypeDefinition> {
    crate::schema::get_pin_type_definitions()
}

/// 获取所有分类定义
#[tauri::command]
pub fn get_categories() -> Vec<CategoryDefinition> {
    crate::schema::get_category_definitions()
}

/// 获取所有 UI 样式定义
#[tauri::command]
pub fn get_ui_styles() -> Vec<UIStyleDefinition> {
    crate::schema::get_ui_style_definitions()
}

/// 获取所有变量类型定义
#[tauri::command]
pub fn get_variable_types() -> Vec<VariableTypeDefinition> {
    crate::schema::get_variable_type_definitions()
}

/// 检查两个类型是否可以连接（使用类型推断系统）
#[tauri::command]
pub fn check_type_connection(from_type: String, to_type: String) -> bool {
    use crate::executor::value::{PinTypeDesc, TypeInferenceContext};
    use uuid::Uuid;

    // 创建临时的类型推断上下文
    let mut type_inference = TypeInferenceContext::new();

    // 生成临时的 PinId
    let temp_output_pin_id = Uuid::new_v4();
    let temp_input_pin_id = Uuid::new_v4();

    // 注册 Pin 类型
    type_inference.register_pin(temp_output_pin_id, PinTypeDesc::from_string(&from_type));
    type_inference.register_pin(temp_input_pin_id, PinTypeDesc::from_string(&to_type));

    // 尝试推断连接
    match type_inference.infer_connection(temp_output_pin_id, temp_input_pin_id) {
        Ok(_) => true,
        Err(_) => {
            // 回退到旧的类型检查（向后兼容）
            crate::schema::can_connect(&from_type, &to_type)
        }
    }
}

/// 获取 Pin 的详细类型信息
#[tauri::command]
pub fn get_pin_type_info(type_str: String) -> serde_json::Value {
    use crate::executor::value::PinTypeDesc;

    let pin_desc = PinTypeDesc::from_string(&type_str);

    serde_json::json!({
        "originalType": type_str,
        "kind": match &pin_desc.data_type {
            crate::executor::value::DataType::Unknown => "Unknown",
            crate::executor::value::DataType::Concrete(_) => "Concrete",
            crate::executor::value::DataType::TypeVar(_) => "TypeVar",
            crate::executor::value::DataType::Union(_) => "Union",
        },
        "concreteType": pin_desc.data_type.as_concrete().map(|t| t.to_string()),
        "typeVarId": pin_desc.data_type.as_type_var().map(|id| id.0),
        "constraints": pin_desc.constraints.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>(),
        "optional": pin_desc.optional,
        "isArray": pin_desc.is_array,
        "displayString": pin_desc.type_string(),
    })
}

/// 检查 Pin 兼容性（高级版本，返回详细信息）
#[tauri::command]
pub fn check_pin_compatibility_detailed(
    source_pin_id: String,
    target_pin_id: String,
    source_type: String,
    target_type: String,
) -> serde_json::Value {
    use crate::executor::value::{PinTypeDesc, TypeInferenceContext};
    use uuid::Uuid;

    let mut type_inference = TypeInferenceContext::new();

    // 使用提供的 pin_id 或生成临时 ID
    let temp_output_pin_id = source_pin_id.parse().unwrap_or_else(|_| Uuid::new_v4());
    let temp_input_pin_id = target_pin_id.parse().unwrap_or_else(|_| Uuid::new_v4());

    // 注册 Pin 类型
    type_inference.register_pin(temp_output_pin_id, PinTypeDesc::from_string(&source_type));
    type_inference.register_pin(temp_input_pin_id, PinTypeDesc::from_string(&target_type));

    // 尝试推断连接
    match type_inference.infer_connection(temp_output_pin_id, temp_input_pin_id) {
        Ok(_) => {
            serde_json::json!({
                "compatible": true,
                "method": "TypeInference",
                "sourceType": source_type,
                "targetType": target_type,
                "message": "Types are compatible via type inference"
            })
        }
        Err(e) => {
            // 尝试旧的类型检查
            let old_compatible = crate::schema::can_connect(&source_type, &target_type);
            serde_json::json!({
                "compatible": old_compatible,
                "method": if old_compatible { "LegacyTypeCheck" } else { "Incompatible" },
                "sourceType": source_type,
                "targetType": target_type,
                "message": if old_compatible {
                    "Types are compatible via legacy type check".to_string()
                } else {
                    format!("Types are incompatible: {}", e)
                }
            })
        }
    }
}
