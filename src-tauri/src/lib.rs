mod executor;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn execute_graph(data: serde_json::Value) -> Result<Vec<String>, String> {
    println!("Received graph data for execution: {}", data);
    
    // 解析 JSON 到 GraphData 结构
    let graph: executor::GraphData = serde_json::from_value(data)
        .map_err(|e| format!("Failed to parse graph data: {}", e))?;

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
