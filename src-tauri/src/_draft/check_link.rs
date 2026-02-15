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