//! YssBI 后端模块
//!
//! 包含所有核心功能：schema 定义、节点系统、执行器、项目管理、状态管理等。

pub mod application;
pub mod commands;
pub mod database;
pub mod error;
pub mod event;
#[allow(dead_code, unused_imports)]
mod execution;
pub mod frontend;
#[allow(dead_code, unused_imports)]
mod graph;
pub mod julia;
pub mod log;
pub mod math;
pub mod node_system;
pub mod project;
#[allow(dead_code, unused_imports)]
mod schema;
pub mod sci;
pub mod tabular;
pub mod variable;
pub mod window_state;

use commands::*;
use tauri::Manager;

// ==================== 应用入口 ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let julia_worker = julia::worker::JuliaWorkerManager::new();
    let bayes_worker = julia_worker.clone();

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
        .manage(project::ProjectWatcherState::new())
        .manage(project::ProjectPickerTaskCancelRegistry::new())
        .manage(julia_worker)
        .setup(move |app| {
            let project_state =
                project::ProjectState::try_new().map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(project_state);

            // 初始化日志管理器
            log::init_log_manager(app.handle().clone());
            let app_dir = app.path().app_data_dir()?;
            let project_registry =
                tauri::async_runtime::block_on(project::ProjectRegistry::init(app_dir.clone()))?;
            app.manage(project_registry);
            app.manage(application::bayes::BayesInferenceService::with_backend(
                std::sync::Arc::new(sci::backends::julia::bayes::JuliaBayesBackend::new(
                    app_dir.clone(),
                    bayes_worker.clone(),
                )),
            ));
            let warmup_worker = bayes_worker.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = warmup_worker.warm_up(&app_dir) {
                    tauri_plugin_log::log::warn!("Failed to warm up Julia worker: {error}");
                }
            });

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
            // ==================== Node system ====================
            get_localized_node_catalog,
            create_event,
            create_function,
            remove_graph,
            unload_project_graph,
            save_project_graph,
            duplicate_graph,
            rename_graph_resource,
            update_function_signature,
            hydrate_editor_graph,
            mutate_graph_document,
            get_project_history_status,
            undo_graph_document,
            redo_graph_document,
            execute_graph_document,
            cancel_graph_run,
            list_graph_traces,
            get_run_trace,
            get_result_source_descriptor,
            get_result_source_value,
            get_result_source_page,
            release_result_source,
            release_run_result_sources,
            // ==================== 项目管理 ====================
            get_project_databases_variables,
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
            // ==================== 窗口几何状态 ====================
            get_window_states,
            get_window_state,
            save_window_state,
            // ==================== Variable CRUD ====================
            create_variable,
            get_variable,
            update_variable,
            delete_variable,
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
            // ==================== Bayesian inference ====================
            parse_bayes_expression,
            validate_bayes_model,
            submit_bayes_inference,
            get_bayes_inference_status,
            cancel_bayes_inference,
            read_bayes_inference_result,
            clear_bayes_inference_task,
            export_bayes_artifact_csv,
            read_bayes_posterior_samples,
            read_bayes_trace_plot_data,
            read_bayes_density_plot_data,
            read_bayes_autocorrelation_data,
            read_bayes_posterior_predictive,
            // ==================== Julia runtime ====================
            get_julia_runtime_status,
            get_julia_worker_status,
            install_julia_runtime,
            // ==================== 日志 ====================
            frontend_log,
            get_logs,
            get_log_file_path,
            get_log_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
