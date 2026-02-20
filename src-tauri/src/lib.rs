//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器、项目管理、状态管理等。

pub mod commands;
pub mod database;
pub mod editor;
pub mod event;
pub mod execution;
pub mod frontend;
pub mod graph;
pub mod log;
pub mod project;
pub mod schema;
pub mod variable;

use commands::*;

// ==================== 应用入口 ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                )])
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
        .manage(project::ProjectState::new())
        .setup(|app| {
            // 初始化日志管理器
            log::init_log_manager(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ==================== Schema ====================
            get_node_definitions,
            get_editor_schema_command,
            // ==================== 项目管理 ====================
            get_project_data,
            get_project_databases_variables,
            get_project_graphs,
            get_project_path,
            new_project,
            load_project,
            save_project,
            execute_project,
            // ==================== 设置 ====================
            load_settings,
            save_settings,
            // ==================== Graph CRUD ====================
            get_graph,
            remove_graph,
            create_event,
            update_event,
            create_function,
            update_function,
            create_macro,
            update_macro,
            // ==================== Variable CRUD ====================
            create_variable,
            get_variable,
            update_variable,
            delete_variable,
            // ==================== Node ====================
            create_node,
            create_node_with_id,
            batch_create_nodes,
            delete_node,
            batch_delete_nodes,
            update_node_positions,
            restore_nodes,
            // ==================== Connection ====================
            connect_pins,
            disconnect_pin,
            delete_connection,
            get_connections,
            delete_connections_for_pin,
            delete_connections_for_node,
            // ==================== Pin ====================
            update_pin_user_value,
            clear_pin_user_value,
            // ==================== History ====================
            sync_graph_state,
            // ==================== 子图管理 ====================
            update_canvas,
            update_subgraph_io,
            rename_subgraph,
            // ==================== Database ====================
            load_database,
            delete_database,
            get_database_meta,
            get_database_rows,
            // ==================== 日志 ====================
            get_logs,
            get_log_file_path,
            get_log_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
