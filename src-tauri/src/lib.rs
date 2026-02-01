//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器、项目管理、状态管理等。

pub mod executor;
pub mod project;
pub mod schema;
pub mod settings;
pub mod state;
use crate::executor::ExecutionContextTrait;
use chrono::Utc;
use executor::GenericNode;
use polars::prelude::*;
use project::{CanvasState, PinDefinition, ProjectData, SerializedNode, SubGraphData};
use schema::{
    get_editor_schema, CategoryDefinition, EditorSchema, PinTypeDefinition, UIStyleDefinition,
    VariableDefinition, VariableTypeDefinition,
};
use state::{emit_project_event, ProjectEvent, ProjectState};
use std::collections::HashMap;
use std::sync::Arc;
use settings::{load_settings, save_settings};
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;

// ==================== 数据导入命令 ====================

/// 从 CSV 导入数据
#[tauri::command]
async fn import_csv(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<crate::project::DataFrameData, String> {
    info!("[import_csv] Importing from: {}", path);

    // 使用 Polars 读取 CSV
    let df = CsvReadOptions::default()
        .with_has_header(true)
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.clone().into()))
        .map_err(|e| format!("Failed to open CSV: {}", e))?
        .finish()
        .map_err(|e| format!("Failed to parse CSV: {}", e))?;

    let id = format!("df_{:x}", Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let df_data = state.add_dataframe(id.clone(), df, Some(path))?;

    // 通知所有窗口
    emit_project_event(
        &app,
        ProjectEvent::DataFrameCreated {
            id,
            data: df_data.clone(),
        },
    );

    Ok(df_data)
}

/// 删除数据帧
#[tauri::command]
fn delete_dataframe(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    info!("[delete_dataframe] id={}", id);
    state.delete_dataframe(&id)?;
    emit_project_event(&app, ProjectEvent::DataFrameDeleted { id });
    Ok(())
}

/// 创建数据帧（手动创建）
#[tauri::command]
fn create_dataframe(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: crate::project::DataFrameData,
) -> Result<crate::project::DataFrameData, String> {
    info!("[create_dataframe] id={}, name={}", id, data.name);
    let result = state.create_dataframe(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::DataFrameCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 获取数据帧行数据
#[tauri::command]
fn get_dataframe_rows(
    state: State<'_, ProjectState>,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<Vec<serde_json::Value>>, String> {
    let df_store = state.df_store.read().unwrap();
    let df = df_store
        .get(&id)
        .ok_or_else(|| format!("DataFrame '{}' not found in memory", id))?;

    let height = df.height();
    if offset >= height {
        return Ok(vec![]);
    }

    let actual_limit = std::cmp::min(limit, height - offset);
    let slice = df.slice(offset as i64, actual_limit);

    let mut rows = Vec::new();
    for i in 0..slice.height() {
        let mut row = Vec::new();
        for col_idx in 0..slice.width() {
            let val = slice.get_columns()[col_idx].get(i).unwrap();
            let json_val = match val {
                polars::prelude::AnyValue::Null => serde_json::Value::Null,
                polars::prelude::AnyValue::Boolean(b) => serde_json::Value::Bool(b),
                polars::prelude::AnyValue::String(s) => serde_json::Value::String(s.to_string()),
                polars::prelude::AnyValue::StringOwned(s) => {
                    serde_json::Value::String(s.to_string())
                }
                polars::prelude::AnyValue::Int8(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int16(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Int64(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt8(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt16(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::UInt64(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Float32(v) => serde_json::json!(v),
                polars::prelude::AnyValue::Float64(v) => serde_json::json!(v),
                _ => serde_json::Value::String(format!("{:?}", val)),
            };
            row.push(json_val);
        }
        rows.push(row);
    }

    Ok(rows)
}

// ==================== Schema 命令 ====================

/// 获取所有节点定义
#[tauri::command]
fn get_node_definitions() -> Vec<Arc<GenericNode>> {
    let defs = executor::get_all_node_definitions();
    info!("[Backend] Returning {} node definitions", defs.len());
    defs
}

/// 获取完整的编辑器 Schema（一次性获取所有元数据）
#[tauri::command]
fn get_editor_schema_command() -> EditorSchema {
    get_editor_schema()
}

/// 获取所有 Pin 类型定义
#[tauri::command]
fn get_pin_types() -> Vec<PinTypeDefinition> {
    schema::get_pin_type_definitions()
}

/// 获取所有分类定义
#[tauri::command]
fn get_categories() -> Vec<CategoryDefinition> {
    schema::get_category_definitions()
}

/// 获取所有 UI 样式定义
#[tauri::command]
fn get_ui_styles() -> Vec<UIStyleDefinition> {
    schema::get_ui_style_definitions()
}

/// 获取所有变量类型定义
#[tauri::command]
fn get_variable_types() -> Vec<VariableTypeDefinition> {
    schema::get_variable_type_definitions()
}

/// 检查两个类型是否可以连接（使用类型推断系统）
#[tauri::command]
fn check_type_connection(from_type: String, to_type: String) -> bool {
    use crate::executor::value::{PinTypeDesc, TypeInferenceContext};
    use uuid::Uuid;
    
    // 创建临时的类型推断上下文
    let mut type_inference = TypeInferenceContext::new();
    
    // 生成临时的 PinId
    let temp_output_pin_id = Uuid::new_v4();
    let temp_input_pin_id = Uuid::new_v4();
    
    // 注册 Pin 类型
    type_inference.register_pin(
        temp_output_pin_id,
        PinTypeDesc::from_string(&from_type)
    );
    type_inference.register_pin(
        temp_input_pin_id,
        PinTypeDesc::from_string(&to_type)
    );
    
    // 尝试推断连接
    match type_inference.infer_connection(temp_output_pin_id, temp_input_pin_id) {
        Ok(_) => true,
        Err(_) => {
            // 回退到旧的类型检查（向后兼容）
            schema::can_connect(&from_type, &to_type)
        }
    }
}

/// 获取 Pin 的详细类型信息
#[tauri::command]
fn get_pin_type_info(type_str: String) -> serde_json::Value {
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
fn check_pin_compatibility_detailed(
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
    type_inference.register_pin(
        temp_output_pin_id,
        PinTypeDesc::from_string(&source_type)
    );
    type_inference.register_pin(
        temp_input_pin_id,
        PinTypeDesc::from_string(&target_type)
    );
    
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
            let old_compatible = schema::can_connect(&source_type, &target_type);
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

// ==================== 项目状态命令 ====================

/// 获取当前项目状态
#[tauri::command]
fn get_project_state(state: State<'_, ProjectState>) -> ProjectData {
    let data = state.get_data();
    info!(
        "[get_project_state] events={}, functions={}, macros={}, globalVars={}",
        data.events.len(),
        data.functions.len(),
        data.macros.len(),
        data.global_variables.len()
    );
    // 打印每个 event 的详细信息
    for (id, event) in &data.events {
        info!(
            "[get_project_state] Event '{}': name='{}', nodes={}",
            id,
            event.name,
            event.nodes.len()
        );
    }
    data
}

/// 获取当前项目路径
#[tauri::command]
fn get_project_path(state: State<'_, ProjectState>) -> Option<String> {
    let path = state.get_current_path();
    info!("[get_project_path] path={:?}", path);
    path
}

/// 新建项目（清空当前状态）
#[tauri::command]
fn new_project(app: AppHandle, state: State<'_, ProjectState>) -> Result<(), String> {
    info!("[new_project] Clearing project state");
    state.clear();
    emit_project_event(&app, ProjectEvent::ProjectCleared);
    Ok(())
}

/// 加载项目（从状态管理器）
#[tauri::command]
fn load_project_to_state(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<ProjectData, String> {
    info!("[load_project_to_state] Loading from path: {}", path);
    let project = project::load_project_from_file(&path)?;
    info!(
        "[load_project_to_state] Loaded: global_vars={}, events={}, functions={}, macros={}",
        project.global_variables.len(),
        project.events.len(),
        project.functions.len(),
        project.macros.len()
    );

    // 记录加载的变量详情
    for (id, var) in &project.global_variables {
        info!(
            "[load_project_to_state] Global Variable '{}': name={}, type={:?}",
            id, var.name, var.data_type
        );
    }

    state.set_data(project.clone());
    state.set_current_path(Some(path.clone()));
    emit_project_event(
        &app,
        ProjectEvent::ProjectLoaded {
            data: project.clone(),
            path: Some(path),
        },
    );
    Ok(project)
}

/// 保存项目（从状态管理器）
#[tauri::command]
fn save_project_from_state(
    app: AppHandle,
    state: State<'_, ProjectState>,
    path: String,
) -> Result<(), String> {
    info!("[save_project_from_state] Saving to path: {}", path);
    let mut project = state.get_data();
    project.update_metadata();
    project::save_project_to_file(&project, &path)?;
    state.set_current_path(Some(path.clone()));
    emit_project_event(&app, ProjectEvent::ProjectSaved { path });
    Ok(())
}

/// 设置项目数据（用于前端批量同步）
#[tauri::command]
fn set_project_data(
    app: AppHandle,
    state: State<'_, ProjectState>,
    data: ProjectData,
    path: Option<String>,
    emit_event: Option<bool>, // 是否触发事件，默认 false
) -> Result<(), String> {
    info!(
        "[set_project_data] Receiving data: events={}, functions={}, macros={}, global_vars={}",
        data.events.len(),
        data.functions.len(),
        data.macros.len(),
        data.global_variables.len()
    );
    // 打印每个 event 的详细信息
    for (id, event) in &data.events {
        info!(
            "[set_project_data] Event '{}': name='{}', nodes={}",
            id,
            event.name,
            event.nodes.len()
        );
    }
    state.set_data(data.clone());
    if let Some(p) = path.clone() {
        state.set_current_path(Some(p));
    }
    info!("[set_project_data] Data stored successfully");

    // 只在明确要求时才触发事件（避免重复触发）
    if emit_event.unwrap_or(false) {
        info!("[set_project_data] Emitting ProjectLoaded event");
        emit_project_event(&app, ProjectEvent::ProjectLoaded { data, path });
    }
    Ok(())
}

// ==================== Events CRUD 命令 ====================

/// 获取所有事件子图
#[tauri::command]
fn get_events(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    let events = state.get_events();
    info!("[get_events] Returning {} events", events.len());
    events
}

/// 获取单个事件子图
#[tauri::command]
fn get_event(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    let event = state.get_event(&id);
    info!("[get_event] id={}, found={}", id, event.is_some());
    event
}

/// 创建事件子图
#[tauri::command]
fn create_event(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    info!(
        "[create_event] id={}, name={}, nodes={}",
        id,
        data.name,
        data.nodes.len()
    );
    let result = state.create_event(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::EventCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新事件子图
#[tauri::command]
fn update_event(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    info!(
        "[update_event] id={}, name={}, nodes={}",
        id,
        data.name,
        data.nodes.len()
    );
    let result = state.update_event(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::EventUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除事件子图
#[tauri::command]
fn delete_event(app: AppHandle, state: State<'_, ProjectState>, id: String) -> Result<(), String> {
    state.delete_event(&id)?;
    emit_project_event(&app, ProjectEvent::EventDeleted { id });
    Ok(())
}

// ==================== Functions CRUD 命令 ====================

/// 获取所有函数子图
#[tauri::command]
fn get_functions(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    state.get_functions()
}

/// 获取单个函数子图
#[tauri::command]
fn get_function(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    state.get_function(&id)
}

/// 创建函数子图
#[tauri::command]
fn create_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.create_function(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::FunctionCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新函数子图
#[tauri::command]
fn update_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.update_function(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::FunctionUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除函数子图
#[tauri::command]
fn delete_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    state.delete_function(&id)?;
    emit_project_event(&app, ProjectEvent::FunctionDeleted { id });
    Ok(())
}

// ==================== Macros CRUD 命令 ====================

/// 获取所有宏子图
#[tauri::command]
fn get_macros(state: State<'_, ProjectState>) -> HashMap<String, SubGraphData> {
    state.get_macros()
}

/// 获取单个宏子图
#[tauri::command]
fn get_macro(state: State<'_, ProjectState>, id: String) -> Option<SubGraphData> {
    state.get_macro(&id)
}

/// 创建宏子图
#[tauri::command]
fn create_macro(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.create_macro(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::MacroCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新宏子图
#[tauri::command]
fn update_macro(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: SubGraphData,
) -> Result<SubGraphData, String> {
    let result = state.update_macro(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::MacroUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除宏子图
#[tauri::command]
fn delete_macro(app: AppHandle, state: State<'_, ProjectState>, id: String) -> Result<(), String> {
    state.delete_macro(&id)?;
    emit_project_event(&app, ProjectEvent::MacroDeleted { id });
    Ok(())
}

// ==================== Global Variables CRUD 命令 ====================

/// 获取所有全局变量
#[tauri::command]
fn get_global_variables(state: State<'_, ProjectState>) -> HashMap<String, VariableDefinition> {
    state.get_global_variables()
}

/// 获取单个全局变量
#[tauri::command]
fn get_global_variable(state: State<'_, ProjectState>, id: String) -> Option<VariableDefinition> {
    state.get_global_variable(&id)
}

/// 创建全局变量
#[tauri::command]
fn create_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.create_global_variable(id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::GlobalVariableCreated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新全局变量
#[tauri::command]
fn update_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.update_global_variable(&id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::GlobalVariableUpdated {
            id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除全局变量
#[tauri::command]
fn delete_global_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    id: String,
) -> Result<(), String> {
    state.delete_global_variable(&id)?;
    emit_project_event(&app, ProjectEvent::GlobalVariableDeleted { id });
    Ok(())
}

// ==================== Local Variables CRUD 命令 ====================

/// 获取子图的局部变量
#[tauri::command]
fn get_local_variables(
    state: State<'_, ProjectState>,
    subgraph_id: String,
) -> Result<HashMap<String, VariableDefinition>, String> {
    state.get_local_variables(&subgraph_id)
}

/// 创建局部变量
#[tauri::command]
fn create_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.create_local_variable(&subgraph_id, variable_id.clone(), data)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableCreated {
            subgraph_id,
            variable_id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 更新局部变量
#[tauri::command]
fn update_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
    data: VariableDefinition,
) -> Result<VariableDefinition, String> {
    let result = state.update_local_variable(&subgraph_id, &variable_id, data)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableUpdated {
            subgraph_id,
            variable_id,
            data: result.clone(),
        },
    );
    Ok(result)
}

/// 删除局部变量
#[tauri::command]
fn delete_local_variable(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    variable_id: String,
) -> Result<(), String> {
    state.delete_local_variable(&subgraph_id, &variable_id)?;
    emit_project_event(
        &app,
        ProjectEvent::LocalVariableDeleted {
            subgraph_id,
            variable_id,
        },
    );
    Ok(())
}

// ==================== Nodes 命令 ====================

/// 获取子图的节点列表
#[tauri::command]
fn get_nodes(
    state: State<'_, ProjectState>,
    subgraph_id: String,
) -> Result<Vec<SerializedNode>, String> {
    state.get_nodes(&subgraph_id)
}

/// 设置子图的节点列表
#[tauri::command]
fn set_nodes(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    nodes: Vec<SerializedNode>,
) -> Result<(), String> {
    state.set_nodes(&subgraph_id, nodes.clone())?;
    emit_project_event(&app, ProjectEvent::NodesUpdated { subgraph_id, nodes });
    Ok(())
}

/// 创建单个节点
#[tauri::command]
fn create_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node: SerializedNode,
) -> Result<SerializedNode, String> {
    info!(
        "[create_node] subgraph_id={}, node_id={}, node_type={}",
        subgraph_id, node.id, node.node_type
    );
    let result = state.create_node(&subgraph_id, node)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(result)
}

/// 删除单个节点
#[tauri::command]
fn delete_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
) -> Result<(), String> {
    info!(
        "[delete_node] subgraph_id={}, node_id={}",
        subgraph_id, node_id
    );
    state.delete_node(&subgraph_id, &node_id)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(())
}

/// 批量创建节点
#[tauri::command]
fn create_nodes(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    nodes: Vec<SerializedNode>,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[create_nodes] subgraph_id={}, count={}",
        subgraph_id,
        nodes.len()
    );
    let new_nodes = state.create_nodes(&subgraph_id, nodes)?;

    // 发送节点更新事件
    let all_nodes = state.get_nodes(&subgraph_id)?;
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id,
            nodes: all_nodes,
        },
    );

    Ok(new_nodes)
}

/// 连接两个 Pin
#[tauri::command]
fn connect_pins(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    source_pin_id: String,
    target_pin_id: String,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[connect_pins] subgraph_id={}, source={}, target={}",
        subgraph_id, source_pin_id, target_pin_id
    );
    let nodes = state.connect_pins(&subgraph_id, &source_pin_id, &target_pin_id)?;

    // 发送节点更新事件
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id: subgraph_id.clone(),
            nodes: nodes.clone(),
        },
    );

    info!("[connect_pins] Connection successful");
    Ok(nodes)
}

/// 断开 Pin 的所有连接
#[tauri::command]
fn disconnect_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    pin_id: String,
) -> Result<Vec<SerializedNode>, String> {
    info!(
        "[disconnect_pin] subgraph_id={}, pin_id={}",
        subgraph_id, pin_id
    );
    let nodes = state.disconnect_pin(&subgraph_id, &pin_id)?;

    // 发送节点更新事件
    emit_project_event(
        &app,
        ProjectEvent::NodesUpdated {
            subgraph_id: subgraph_id.clone(),
            nodes: nodes.clone(),
        },
    );

    info!("[disconnect_pin] Disconnection successful");
    Ok(nodes)
}

/// 更新子图的画布状态
#[tauri::command]
fn update_canvas(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    canvas: CanvasState,
) -> Result<(), String> {
    state.update_canvas(&subgraph_id, canvas)
}

/// 更新子图的输入输出定义
#[tauri::command]
fn update_subgraph_io(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    inputs: Option<Vec<PinDefinition>>,
    outputs: Option<Vec<PinDefinition>>,
) -> Result<SubGraphData, String> {
    state.update_subgraph_io(&subgraph_id, inputs, outputs)?;
    // 返回更新后的子图数据
    let updated = state
        .get_event(&subgraph_id)
        .or_else(|| state.get_function(&subgraph_id))
        .or_else(|| state.get_macro(&subgraph_id))
        .ok_or_else(|| format!("Subgraph '{}' not found after update", subgraph_id))?;

    // 发送相应的更新事件
    if state.get_event(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::EventUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else if state.get_function(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::FunctionUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else {
        emit_project_event(
            &app,
            ProjectEvent::MacroUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    }

    Ok(updated)
}

/// 重命名子图
#[tauri::command]
fn rename_subgraph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    new_name: String,
) -> Result<SubGraphData, String> {
    state.rename_subgraph(&subgraph_id, new_name)?;
    // 返回更新后的子图数据
    let updated = state
        .get_event(&subgraph_id)
        .or_else(|| state.get_function(&subgraph_id))
        .or_else(|| state.get_macro(&subgraph_id))
        .ok_or_else(|| format!("Subgraph '{}' not found after rename", subgraph_id))?;

    // 发送相应的更新事件
    if state.get_event(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::EventUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else if state.get_function(&subgraph_id).is_some() {
        emit_project_event(
            &app,
            ProjectEvent::FunctionUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    } else {
        emit_project_event(
            &app,
            ProjectEvent::MacroUpdated {
                id: subgraph_id,
                data: updated.clone(),
            },
        );
    }

    Ok(updated)
}

// ==================== 执行日志 ====================

/// 保存执行日志到文件
///
/// 在每次执行图时自动保存项目 JSON 到 logs 目录，文件名包含时间戳
fn save_execution_log(data: &ProjectData) -> Result<(), String> {
    use std::fs;
    use std::path::PathBuf;
    
    // 创建 logs 目录
    let logs_dir = PathBuf::from("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(&logs_dir)
            .map_err(|e| format!("Failed to create logs directory: {}", e))?;
    }
    
    // 生成带时间戳的文件名
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("execution_{}.json", timestamp);
    let log_path = logs_dir.join(filename);
    
    // 序列化并保存项目数据
    let json = data.to_json()?;
    fs::write(&log_path, json)
        .map_err(|e| format!("Failed to write execution log: {}", e))?;
    
    info!("[save_execution_log] Saved execution log to: {:?}", log_path);
    Ok(())
}

// ==================== 执行命令 ====================

/// 执行图（从状态管理器获取数据）
#[tauri::command]
fn execute_graph(app: AppHandle, state: State<'_, ProjectState>) -> Result<Vec<String>, String> {
    let data = state.get_data();
    execute_project_data(app, data)
}

/// 执行指定的项目数据（兼容旧接口）
#[tauri::command]
fn execute_project(app: AppHandle, data: ProjectData) -> Result<Vec<String>, String> {
    execute_project_data(app, data)
}

fn execute_project_data(app: AppHandle, data: ProjectData) -> Result<Vec<String>, String> {
    info!("[execute_project_data] Received project data for execution");
    
    // 保存执行前的项目 JSON 日志
    if let Err(e) = save_execution_log(&data) {
        // 日志保存失败不应阻止执行，只记录警告
        info!("[execute_project_data] Warning: Failed to save execution log: {}", e);
    }
    
    let _logs = vec!["[System] Received event for execution".to_string()];

    let mut nodes: Vec<executor::NodeData> = Vec::new();
    let mut variables: HashMap<String, executor::VariableData> = HashMap::new();

    // 1. 收集全局变量
    for (id, var) in &data.global_variables {
        // 将 VariableDefinition 转换为 VariableData
        let value = var
            .static_value
            .clone()
            .or_else(|| var.default_value.clone())
            .unwrap_or(serde_json::Value::Null);
        variables.insert(
            id.clone(),
            executor::VariableData {
                name: var.name.clone(),
                var_type: format!("{:?}", var.data_type).to_lowercase(),
                value,
            },
        );
    }

    // 2. 从所有子图收集节点和局部变量
    let collections = vec![(&data.events), (&data.functions), (&data.macros)];

    for subgraphs in collections {
        for (sg_id, sub) in subgraphs {
            // 收集子图节点
            for sn in &sub.nodes {
                let node = executor::NodeData {
                    id: sn.id.clone(),
                    node_type: sn.node_type.clone(),
                    title: sn.title.clone(),
                    inputs: sn
                        .inputs
                        .iter()
                        .map(|p| executor::PinData {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            pin_type: p.pin_type.clone(),
                            links: p.links.clone(),
                            default_value: p.default_value.clone(),
                            is_array: p.is_array,
                        })
                        .collect(),
                    outputs: sn
                        .outputs
                        .iter()
                        .map(|p| executor::PinData {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            pin_type: p.pin_type.clone(),
                            links: p.links.clone(),
                            default_value: p.default_value.clone(),
                            is_array: p.is_array,
                        })
                        .collect(),
                    variable_id: sn.variable_id.clone(),
                    sub_graph_id: Some(sg_id.clone()),
                };
                nodes.push(node);
            }

            // 收集局部变量
            for (id, var) in &sub.variables {
                let value = var
                    .static_value
                    .clone()
                    .or_else(|| var.default_value.clone())
                    .unwrap_or(serde_json::Value::Null);
                variables.insert(
                    id.clone(),
                    executor::VariableData {
                        name: var.name.clone(),
                        var_type: format!("{:?}", var.data_type).to_lowercase(),
                        value,
                    },
                );
            }
        }
    }

    info!(
        "[execute_project_data] Collected {} nodes and {} variables",
        nodes.len(),
        variables.len()
    );

    // 打印变量名，方便调试
    for (id, var) in &variables {
        info!(
            "[execute_project_data] Variable '{}' (type={})",
            id, var.var_type
        );
    }

    // 3. 构造 GraphData
    let graph = executor::GraphData {
        version: "1.0.0".to_string(),
        nodes,
        variables: Some(variables),
    };

    // 4. 执行
    let mut context = executor::ExecutionContext::new(graph);
    context.set_app_handle(app);

    // 添加初始系统日志
    context.log("[System] Received event for execution".to_string());

    context.execute()
}

// ==================== Unified Create Variable ====================

#[tauri::command]
fn create_variable(
    state: State<'_, ProjectState>,
    subgraph_id: Option<String>,
    name: Option<String>,
    data_type: Option<String>,
) -> Result<VariableDefinition, String> {
    state.create_variable(subgraph_id, name, data_type)
}

// ==================== 兼容旧接口的项目文件命令 ====================

/// 保存项目到指定路径（兼容旧接口）
#[tauri::command]
fn save_project(path: String, project_json: String) -> Result<(), String> {
    let mut project: ProjectData = serde_json::from_str(&project_json)
        .map_err(|e| format!("Failed to parse project data: {}", e))?;

    // 更新元数据时间戳
    project.update_metadata();

    project::save_project_to_file(&project, &path)
}

/// 从指定路径加载项目（兼容旧接口）
#[tauri::command]
fn load_project(path: String) -> Result<ProjectData, String> {
    project::load_project_from_file(&path)
}

/// 解析项目 JSON（不涉及文件操作）
#[tauri::command]
fn parse_project(json: String) -> Result<ProjectData, String> {
    ProjectData::from_json(&json)
}

/// 序列化项目为 JSON（不涉及文件操作）
#[tauri::command]
fn serialize_project(project: ProjectData) -> Result<String, String> {
    project.to_json()
}

// ==================== 应用入口 ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .level(tauri_plugin_log::log::LevelFilter::Debug)
                .format(|out, message, record| {
                    use chrono::Local;
                    // 简化日志格式: [时间][来源][级别] 消息
                    let target = record.target();
                    // 简化 webview 目标名称
                    let short_target = if target.starts_with("webview:") {
                        "FE"
                    } else if target.contains("yssbi") {
                        "BE"
                    } else {
                        target
                    };
                    let now = Local::now();
                    out.finish(format_args!(
                        "[{}][{}][{}] {}",
                        now.format("%H:%M:%S%.3f"),
                        short_target,
                        record.level(),
                        message
                    ))
                })
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // 注册全局状态管理器
        .manage(ProjectState::new())
        .invoke_handler(tauri::generate_handler![
            // Schema 命令
            get_node_definitions,
            get_editor_schema_command,
            get_pin_types,
            get_categories,
            get_ui_styles,
            get_variable_types,
            check_type_connection,
            get_pin_type_info,
            check_pin_compatibility_detailed,
            // 项目状态命令
            get_project_state,
            get_project_path,
            new_project,
            load_project_to_state,
            save_project_from_state,
            set_project_data,
            // 设置相关
            load_settings,
            save_settings,
            // Events CRUD
            get_events,
            get_event,
            create_event,
            update_event,
            delete_event,
            // Functions CRUD
            get_functions,
            get_function,
            create_function,
            update_function,
            delete_function,
            // Macros CRUD
            get_macros,
            get_macro,
            create_macro,
            update_macro,
            delete_macro,
            // Global Variables CRUD
            get_global_variables,
            get_global_variable,
            create_global_variable,
            update_global_variable,
            delete_global_variable,
            // Local Variables CRUD
            get_local_variables,
            create_local_variable,
            create_variable, // Added here
            update_local_variable,
            delete_local_variable,
            // Nodes 命令
            get_nodes,
            set_nodes,
            create_node,
            create_nodes,
            delete_node,
            connect_pins,
            disconnect_pin,
            update_canvas,
            update_subgraph_io,
            rename_subgraph,
            // 执行命令
            execute_graph,
            execute_project,
            // 兼容旧接口
            save_project,
            load_project,
            parse_project,
            serialize_project,
            // 数据导入
            import_csv,
            delete_dataframe,
            create_dataframe,
            get_dataframe_rows,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
