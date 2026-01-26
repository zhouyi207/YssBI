mod executor;

use std::collections::HashMap;
use serde_json;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn execute_graph(data: serde_json::Value) -> Result<Vec<String>, String> {
    println!("Received project data for execution: {}", data);
    
    // 从项目结构中提取所有节点和变量
    // ProjectData { version, globalVariables, events, functions, macros, metadata }
    
    let mut all_nodes = Vec::new();
    let mut all_variables = HashMap::new();
    let version = data["version"].as_str().unwrap_or("1.0.0").to_string();
    // 1. 收集全局变量 (取其具体内容)
    if let Some(globals) = data["globalVariables"].as_object() {
        for (id, var) in globals {
            all_variables.insert(id.clone(), var.clone());
        }
    }
    // 2. 从所有子图（事件、函数、宏）中收集节点和局部变量
    let categories = ["events", "functions", "macros"];
    for cat in categories {
        if let Some(subgraphs) = data[cat].as_object() {
            for (sg_id, sub) in subgraphs {
                if let Some(nodes) = sub["nodes"].as_array() {
                    for node_val in nodes {
                        let mut node = node_val.clone();
                        // 确保每个节点都知道自己属于哪个子图 (对应后端的 sub_graph_id)
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
    // 构造旧版 executor 期望的 GraphData 格式 (nodes, variables, version)
    let graph_json = serde_json::json!({
        "version": version,
        "nodes": all_nodes,
        "variables": all_variables
    });
    let graph: executor::GraphData = serde_json::from_value(graph_json)
        .map_err(|e| format!("Failed to parse project to graph: {}", e))?;

    // 创建执行上下文并运行
    let mut context = executor::ExecutionContext::new(graph);
    context.execute()
}

#[tauri::command]
fn get_node_definitions() -> Vec<executor::NodeDefinition> {
    executor::ExecutionContext::get_definitions()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![execute_graph, get_node_definitions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
