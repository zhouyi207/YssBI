//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器等。

pub mod executor;
pub mod nodes;
pub mod schema;

use nodes::{GraphData, NodeDefinition};
use schema::{
    CategoryDefinition, EditorSchema, PinTypeDefinition, UIStyleDefinition,
    VariableTypeDefinition, get_editor_schema,
};
use std::collections::HashMap;

// ==================== Tauri 命令 ====================

/// 执行图
#[tauri::command]
fn execute_graph(data: serde_json::Value) -> Result<Vec<String>, String> {
    println!("Received project data for execution: {}", data);

    let mut all_nodes = Vec::new();
    let mut all_variables = HashMap::new();
    let version = data["version"].as_str().unwrap_or("1.0.0").to_string();

    // 收集全局变量
    if let Some(globals) = data["globalVariables"].as_object() {
        for (id, var) in globals {
            all_variables.insert(id.clone(), var.clone());
        }
    }

    // 从所有子图收集节点和局部变量
    let categories = ["events", "functions", "macros"];
    for cat in categories {
        if let Some(subgraphs) = data[cat].as_object() {
            for (sg_id, sub) in subgraphs {
                if let Some(nodes_arr) = sub["nodes"].as_array() {
                    for node_val in nodes_arr {
                        let mut node = node_val.clone();
                        if node["subGraphId"].is_null() {
                            node["subGraphId"] = serde_json::Value::String(sg_id.clone());
                        }
                        all_nodes.push(node);
                    }
                }
                if let Some(vars) = sub["variables"].as_object() {
                    for (vid, v) in vars {
                        all_variables.insert(vid.clone(), v.clone());
                    }
                }
            }
        }
    }

    // 构造 GraphData
    let graph_json = serde_json::json!({
        "version": version,
        "nodes": all_nodes,
        "variables": all_variables
    });
    let graph: GraphData = serde_json::from_value(graph_json)
        .map_err(|e| format!("Failed to parse project to graph: {}", e))?;

    // 执行
    let mut context = executor::ExecutionContext::new(graph);
    context.execute()
}

/// 获取所有节点定义
#[tauri::command]
fn get_node_definitions() -> Vec<NodeDefinition> {
    nodes::get_all_node_definitions()
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

/// 检查两个类型是否可以连接
#[tauri::command]
fn check_type_connection(from_type: String, to_type: String) -> bool {
    schema::can_connect(&from_type, &to_type)
}

// ==================== 应用入口 ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            execute_graph,
            get_node_definitions,
            get_editor_schema_command,
            get_pin_types,
            get_categories,
            get_ui_styles,
            get_variable_types,
            check_type_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
