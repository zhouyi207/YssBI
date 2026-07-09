//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器、项目管理、状态管理等。

pub mod application;
pub mod ast;
pub mod commands;
pub mod database;
pub mod event;
pub mod execution;
pub mod frontend;
pub mod graph;
pub mod log;
pub mod project;
pub mod schema;
pub mod tabular;
pub mod variable;
pub mod window_state;

use commands::*;
use tauri::Manager;

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
        .manage(project::ProjectWatcherState::new())
        .manage(execution::ResultSourceStore::new())
        .manage(project::ProjectPickerTaskCancelRegistry::new())
        .manage(project::ExecutionCancelRegistry::new())
        .setup(|app| {
            // 初始化日志管理器
            log::init_log_manager(app.handle().clone());
            let app_dir = app.path().app_data_dir()?;
            let project_registry =
                tauri::async_runtime::block_on(project::ProjectRegistry::init(app_dir))?;
            app.manage(project_registry);

            // 加载并应用主窗口几何状态：先 set_size/set_position/maximize，
            // 再 show()。tauri.conf.json 中主窗口需配置为 `visible: false`，
            // 否则会先以默认尺寸闪现一帧再被这里调整。
            let window_state_path = app
                .path()
                .app_config_dir()
                .map(|p| p.join("window_state.json"))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let window_state_store = window_state::WindowStateStore::load(window_state_path);
            if let Err(e) = window_state::apply_main_window_state(app.handle(), &window_state_store)
            {
                tauri_plugin_log::log::warn!("Failed to apply main window state: {}", e);
                // 兜底：即便恢复失败也确保主窗口显示出来
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                }
            }
            app.manage(window_state_store);

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
            get_project_resource_path,
            get_project_index,
            load_project_graph,
            default_project_parent_directory,
            validate_new_project_path,
            list_registered_projects,
            scan_projects_in_directory,
            cancel_project_picker_task,
            cleanup_invalid_registered_projects,
            register_project,
            remove_registered_project,
            delete_registered_project_files,
            toggle_registered_project_favorite,
            get_project_registry_path,
            create_project,
            new_project,
            load_project,
            flush_project,
            save_project_as,
            execute_project,
            cancel_execution,
            clear_graph_execution_artifacts,
            get_result_source_descriptor,
            get_result_source_value,
            get_result_source_page,
            get_pin_result_descriptor,
            release_result_source,
            // ==================== 窗口几何状态 ====================
            get_window_states,
            get_window_state,
            save_window_state,
            // ==================== Graph CRUD ====================
            get_graph,
            unload_project_graph,
            save_project_graph,
            remove_graph,
            duplicate_graph,
            rename_graph_resource,
            create_event,
            create_function,
            update_function_signature,
            get_function_call_sites,
            purge_function_call_sites,
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
            apply_graph_patch,
            update_node_positions,
            batch_create_with_connections,
            update_call_function_target,
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
            add_repeatable_pin,
            remove_repeatable_pin,
            resolve_graph_dynamic_pins,
            // ==================== History ====================
            sync_graph_state,
            // ==================== 子图管理 ====================
            update_canvas,
            // ==================== Database ====================
            load_database,
            list_sqlite_tables,
            list_sql_tables,
            list_excel_sheets,
            delete_database,
            rename_database,
            get_database_meta,
            get_database_rows,
            get_column_stats,
            get_column_distribution,
            get_dataset_overview,
            // ==================== Database Edit ====================
            edit_cell,
            add_row,
            delete_rows,
            add_column,
            delete_column,
            cast_column,
            rename_column,
            undo_edit,
            redo_edit,
            save_database_changes,
            export_database,
            get_edit_state,
            // ==================== Worksheet ====================
            create_worksheet,
            load_worksheet,
            save_worksheet,
            delete_worksheet,
            get_plot_column_pair,
            // ==================== 假设检验 ====================
            hypothesis_test,
            parse_at_values,
            // ==================== ACF/PACF ====================
            compute_acf_pacf,
            // ==================== 序列相关检验 ====================
            compute_serial_tests,
            // ==================== Panel DID（结果页） ====================
            compute_panel_did_fake_group_ri,
            // ==================== 日志 ====================
            frontend_log,
            get_logs,
            get_log_file_path,
            get_log_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
