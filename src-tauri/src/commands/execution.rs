//! 图执行相关命令

use crate::executor::{ExecutionContext, ExecutionContextTrait, GraphDto, NodeDto, PinDto, VariableDto};
use crate::project::ProjectData;
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_log::log::info;
use crate::state::ProjectState;

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

    info!(
        "[save_execution_log] Saved execution log to: {:?}",
        log_path
    );
    Ok(())
}

/// 执行图（从状态管理器获取数据）
#[tauri::command]
pub fn execute_graph(app: AppHandle, state: State<'_, ProjectState>) -> Result<Vec<String>, String> {
    let data = state.get_data();
    execute_project_data(app, data)
}

/// 执行指定的项目数据（兼容旧接口）
#[tauri::command]
pub fn execute_project(app: AppHandle, data: ProjectData) -> Result<Vec<String>, String> {
    execute_project_data(app, data)
}

fn execute_project_data(app: AppHandle, data: ProjectData) -> Result<Vec<String>, String> {
    info!("[execute_project_data] Received project data for execution");
    
    // 发送执行开始日志
    crate::log_exec!(
        crate::logging::LogLevel::Info,
        "开始执行图"
    );

    // 保存执行前的项目 JSON 日志
    if let Err(e) = save_execution_log(&data) {
        // 日志保存失败不应阻止执行，只记录警告
        info!(
            "[execute_project_data] Warning: Failed to save execution log: {}",
            e
        );
    }

    let _logs = vec!["[System] Received event for execution".to_string()];

    let mut nodes: Vec<NodeDto> = Vec::new();
    let mut variables: HashMap<String, VariableDto> = HashMap::new();

    // 1. 收集全局变量
    for (id, var) in &data.global_variables {
        // 将 VariableDefinition 转换为 VariableDto
        let value = var
            .static_value
            .clone()
            .or_else(|| var.default_value.clone())
            .unwrap_or(serde_json::Value::Null);
        variables.insert(
            id.clone(),
            VariableDto {
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
                let node = NodeDto {
                    id: sn.id.clone(),
                    node_type: sn.node_type.clone(),
                    title: sn.title.clone(),
                    inputs: sn
                        .inputs
                        .iter()
                        .map(|p| PinDto {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            pin_type: p.pin_type.clone(),
                            links: p.links.clone(),
                            default_value: p.default_value.clone(),
                            user_value: p.user_value.clone(),
                            is_array: p.is_array,
                            show_widget: true,
                            widget_type: None,
                        })
                        .collect(),
                    outputs: sn
                        .outputs
                        .iter()
                        .map(|p| PinDto {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            pin_type: p.pin_type.clone(),
                            links: p.links.clone(),
                            default_value: p.default_value.clone(),
                            user_value: p.user_value.clone(),
                            is_array: p.is_array,
                            show_widget: true,
                            widget_type: None,
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
                    VariableDto {
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
    
    // 发送收集节点日志
    crate::log_exec!(
        crate::logging::LogLevel::Info,
        format!("收集了 {} 个节点和 {} 个变量", nodes.len(), variables.len())
    );

    // 打印变量名，方便调试
    for (id, var) in &variables {
        info!(
            "[execute_project_data] Variable '{}' (type={})",
            id, var.var_type
        );
    }

    // 3. 构造 GraphDto
    let graph = GraphDto {
        version: "1.0.0".to_string(),
        nodes,
        variables: Some(variables),
    };

    // 4. 执行
    let mut context = ExecutionContext::new(graph);
    context.set_app_handle(app);

    // 添加初始系统日志
    context.log("[System] Received event for execution".to_string());
    
    // 发送执行开始日志
    crate::log_exec!(
        crate::logging::LogLevel::Info,
        "开始执行图节点"
    );

    let result = context.execute();
    
    // 发送执行完成日志
    match &result {
        Ok(logs) => {
            crate::log_exec!(
                crate::logging::LogLevel::Info,
                format!("图执行完成，生成了 {} 条日志", logs.len())
            );
        }
        Err(e) => {
            crate::log_exec!(
                crate::logging::LogLevel::Error,
                format!("图执行失败: {}", e)
            );
        }
    }
    
    result
}
