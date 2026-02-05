//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器、项目管理、状态管理等。

pub mod commands;
pub mod execution;
pub mod project;
pub mod schema;
pub mod state;
pub mod log;
pub mod graph;
pub mod variable;

use commands::*;
use state::ProjectState;

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
        .setup(|app| {
            // 初始化日志管理器
            log::init_log_manager(app.handle().clone());
            Ok(())
        })
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
            create_variable,
            update_local_variable,
            delete_local_variable,
            // Nodes 命令
            get_nodes,
            set_nodes,
            create_node,
            create_nodes,
            create_nodes_with_connections,
            delete_node,
            connect_pins,
            disconnect_pin,
            // Connection 命令
            create_connection,
            delete_connection,
            get_connections,
            delete_connections_for_pin,
            delete_connections_for_node,
            update_canvas,
            update_subgraph_io,
            rename_subgraph,
            // Pin 值管理命令
            update_pin_user_value,
            clear_pin_user_value,
            // 动态 Pin 命令
            get_node_dynamic_constraints,
            add_dynamic_pin,
            remove_dynamic_pin,
            // 执行命令
            execute_graph,
            execute_project,
            // 兼容旧接口
            save_project,
            load_project,
            parse_project,
            serialize_project,
            // 动态 Pin 命令（旧接口，保持兼容）
            add_node_dynamic_pin,
            remove_node_dynamic_pin,
            validate_pin_operation,
            // 数据导入
            import_csv,
            delete_dataframe,
            create_dataframe,
            get_dataframe_rows,
            // 日志命令
            get_logs,
            get_log_file_path,
            get_log_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
