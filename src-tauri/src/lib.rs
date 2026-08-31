//! YssBI Tauri 组合入口。
//!
//! 这里只构造并注入各 crate authority、Application state 与平台适配器；领域行为和
//! transport contract 分别留在各自 owner。

pub mod commands;
pub mod error;
pub mod event;

mod schema;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod architecture_tests;

use commands::*;
use std::sync::Arc;
use tauri::Manager;

// ==================== 应用入口 ====================

fn initialize_project_state() -> yss_project::ProjectState {
    yss_project::ProjectState::new()
}

#[derive(Debug, thiserror::Error)]
enum ApplicationInitializationError {
    #[error("initial application session candidate could not be installed")]
    SessionInstallation,
    #[error("initial application session composition could not be prepared")]
    SessionComposition(
        #[source] yss_application::execution::session_factory::ProjectSessionCandidateError,
    ),
}

fn initialize_application_state(
    project_state: Arc<yss_project::ProjectState>,
) -> Result<yss_application::execution::ApplicationState, ApplicationInitializationError> {
    let scientific_backend: Arc<dyn yss_execution::ports::scientific::ScientificBackend> =
        Arc::new(yss_execution_sci_adapter::SciRuntimeBackend::new());
    let candidate = yss_application::execution::session_factory::build_current_project_candidate(
        yss_application::execution::ApplicationSessionEpoch::INITIAL,
        Arc::clone(&project_state),
        std::iter::empty(),
        Arc::clone(&scientific_backend),
    )
    .map_err(ApplicationInitializationError::SessionComposition)?;
    let application = yss_application::execution::ApplicationState::from_composition(
        Arc::new(yss_application::execution::ApplicationSessionSlot::new()),
        scientific_backend,
    );
    application
        .install_candidate(candidate)
        .map_err(|_| ApplicationInitializationError::SessionInstallation)?;
    Ok(application)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let julia_worker = yss_julia_worker::JuliaWorkerManager::new();
    let bayes_worker = julia_worker.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        // 注册全局状态管理器
        .manage(yss_project_watcher::ProjectWatcherState::new(
            std::sync::Arc::new(yss_project_watcher_notify::NotifyProjectFileWatcher::new()),
        ))
        .manage(yss_project_progress::ProjectTaskCancellationRegistry::new())
        .manage(julia_worker)
        .setup(move |app| {
            let log_dir = app.path().app_log_dir();
            let diagnostics = yss_diagnostics::DiagnosticsRuntime::initialize()
                .map_err(Box::<dyn std::error::Error>::from)?;
            let logging = yss_tracing::LoggingRuntime::initialize(
                log_dir.as_ref().ok().cloned(),
                Some(diagnostics.rust_log_sink()),
            )
            .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(logging);
            app.manage(diagnostics);
            if let Err(error) = log_dir {
                tracing::error!(
                    target: "yssbi::logging",
                    diagnostic_domain = "system",
                    diagnostic_event = "appLogDirectoryUnavailable",
                    error = %error,
                    "Failed to resolve application log directory; file logging is disabled"
                );
            }

            let project_state = Arc::new(initialize_project_state());
            let application_state = initialize_application_state(Arc::clone(&project_state))
                .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(application_state);

            let app_dir = app.path().app_data_dir()?;
            let registry_store = tauri::async_runtime::block_on(
                yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(app_dir.clone()),
            )?;
            let registry_path = registry_store.path().to_path_buf();
            let project_registry = yss_project_registry::ProjectRegistry::new(
                std::sync::Arc::new(registry_store),
                registry_path,
            );
            app.manage(project_registry);
            let bayes_adapter = yss_bayes_worker_julia::JuliaBayesWorkerAdapter::new(
                app_dir.clone(),
                bayes_worker.clone(),
            );
            app.manage(yss_application::bayes::BayesInferenceService::with_worker(
                app_dir.clone(),
                std::sync::Arc::new(bayes_adapter),
                std::sync::Arc::new(yss_bayes_artifact_polars::PolarsBayesArtifactReader::new()),
            ));
            let warmup_worker = bayes_worker.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = warmup_worker.warm_up(&app_dir) {
                    tracing::warn!(
                        target: "yssbi::julia::worker",
                        diagnostic_domain = "execution",
                        error = %error,
                        "Failed to warm up Julia worker"
                    );
                }
            });

            // 加载并应用主窗口几何状态：先 set_size/set_position/maximize，
            // 再 show()。tauri.conf.json 中主窗口需配置为 `visible: false`，
            // 否则会先以默认尺寸闪现一帧再被这里调整。
            let window_state_path = app
                .path()
                .app_config_dir()
                .map(|p| p.join("window_state.json"))
                .map_err(Box::<dyn std::error::Error>::from)?;
            let window_state_store = yss_window_state::WindowStateStore::load(window_state_path);
            if let Err(e) =
                yss_window_state::apply_main_window_state(app.handle(), &window_state_store)
            {
                tracing::warn!(
                    target: "yssbi::window_state",
                    diagnostic_domain = "ui",
                    error = %e,
                    "Failed to apply main window state"
                );
                // 兜底：即便恢复失败也确保主窗口显示出来
                if let Some(win) = app.get_webview_window("main") {
                    if let Err(show_error) = win.show() {
                        tracing::warn!(
                            target: "yssbi::window_state",
                            diagnostic_domain = "ui",
                            error = %show_error,
                            "Failed to show main window after state restoration failure"
                        );
                    }
                }
            }
            app.manage(window_state_store);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ==================== Node system ====================
            get_localized_node_catalog,
            get_compatible_node_catalog,
            create_event,
            create_function,
            remove_graph,
            unload_project_graph,
            save_project_graph,
            duplicate_graph,
            rename_graph_resource,
            update_function_signature,
            hydrate_editor_graph,
            export_graph_subgraph,
            mutate_graph_document,
            get_project_history_status,
            undo_graph_document,
            redo_graph_document,
            allocate_pin_preview_generation,
            execute_graph_document,
            cancel_graph_run,
            get_result_descriptor,
            get_result_value,
            get_result_page,
            get_pin_result_history,
            // ==================== 项目管理 ====================
            get_project_databases_variables,
            get_current_project_activation,
            get_project_path,
            get_project_resource_path,
            get_project_index,
            get_project_computation_settings,
            update_project_computation_settings,
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
            duplicate_worksheet,
            load_worksheet,
            save_worksheet,
            rename_worksheet_resource,
            remove_worksheet,
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
            // ==================== Diagnostics ====================
            submit_frontend_diagnostics,
            subscribe_diagnostics,
            unsubscribe_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
