use crate::{project::ProjectState, schema::{
    CategoryDefinition, EditorSchema, PinTypeDefinition, UIStyleDefinition, VariableTypeDefinition, get_editor_schema
}};
use crate::schema::NodeDefinitionDTO;
use crate::log::log_app;
use tauri::State;

// 获取所有节点定义
#[tauri::command]
pub fn get_node_definitions(state: State<ProjectState>) -> Vec<NodeDefinitionDTO> {
    log_app::info!("get_node_definitions command called");
    
    let node_register = &state.project_store.read().unwrap().node_register;
    let all_nodes = node_register.all();
    
    log_app::debug!("Node registry has {} nodes", all_nodes.len());
    
    let result: Vec<NodeDefinitionDTO> = all_nodes
        .iter()
        .map(|def| NodeDefinitionDTO::from(def.as_ref()))
        .collect();
    
    log_app::debug!("Returning {} node definitions to frontend", result.len());
    
    result
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

// 检查两个类型是否可以连接（使用类型推断系统）
// #[tauri::command]
// pub fn check_type_connection(from_type: String, to_type: String) -> bool {
//     use crate::executor::value::{PinDataType, TypeInferenceContext};
//     use uuid::Uuid;

//     // 创建临时的类型推断上下文
//     let mut type_inference = TypeInferenceContext::new();

//     // 生成临时的 PinId
//     let temp_output_pin_id = Uuid::new_v4();
//     let temp_input_pin_id = Uuid::new_v4();

//     // 注册 Pin 类型
//     type_inference.register_pin(temp_output_pin_id, PinDataType::from_string(&from_type));
//     type_inference.register_pin(temp_input_pin_id, PinDataType::from_string(&to_type));

//     // 尝试推断连接
//     match type_inference.infer_connection(temp_output_pin_id, temp_input_pin_id) {
//         Ok(_) => true,
//         Err(_) => {
//             // 回退到旧的类型检查（向后兼容）
//             crate::schema::can_connect(&from_type, &to_type)
//         }
//     }
// }

// 获取 Pin 的详细类型信息
// #[tauri::command]
// pub fn get_pin_type_info(type_str: String) -> serde_json::Value {
//     use crate::executor::value::PinDataType;

//     let pin_desc = PinDataType::from_string(&type_str);

//     serde_json::json!({
//         "originalType": type_str,
//         "kind": match &pin_desc.data_type {
//             crate::executor::value::DataType::Unknown => "Unknown",
//             crate::executor::value::DataType::Concrete(_) => "Concrete",
//             crate::executor::value::DataType::TypeVar(_) => "TypeVar",
//             crate::executor::value::DataType::Union(_) => "Union",
//         },
//         "concreteType": pin_desc.data_type.as_concrete().map(|t| t.to_string()),
//         "typeVarId": pin_desc.data_type.as_type_var().map(|id| id.0),
//         "constraints": pin_desc.constraints.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>(),
//         "optional": pin_desc.optional,
//         "isArray": pin_desc.is_array,
//         "displayString": pin_desc.type_string(),
//     })
// }