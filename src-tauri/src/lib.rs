//! YssBI Tauri 组合入口。
//!
//! 这里只构造并注入各 crate authority、Application state 与平台适配器；领域行为和
//! transport contract 分别留在各自 owner。

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod architecture_tests;

use std::sync::Arc;
use tauri::Manager;
use yss_automation_contract::{
    AutomationIdKind, ClockPort, IdGenerationFailure, IdGeneratorPort, UnixMillis,
};

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

#[derive(Debug, thiserror::Error)]
enum HarnessInitializationError {
    #[error("initial Harness project session could not be captured")]
    SessionCapture(#[from] yss_application::execution::SessionCaptureError),
    #[error("Harness SQLite persistence could not be initialized")]
    Persistence(#[from] yss_automation_contract::PersistenceFailure),
    #[error("Harness host could not be initialized")]
    Host(#[from] yss_statistical_harness::HarnessError),
    #[error("Harness builtin knowledge could not be initialized")]
    Knowledge(#[from] yss_statistical_harness::KnowledgeError),
}

struct SystemHarnessClock;

impl ClockPort for SystemHarnessClock {
    fn now(&self) -> UnixMillis {
        let milliseconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        UnixMillis::from_existing(u64::try_from(milliseconds).unwrap_or(u64::MAX))
    }
}

struct HarnessIdGenerator;

impl IdGeneratorPort for HarnessIdGenerator {
    fn next_id(&self, kind: AutomationIdKind) -> Result<String, IdGenerationFailure> {
        let prefix = match kind {
            AutomationIdKind::HarnessSession => "session",
            AutomationIdKind::HarnessTurn => "turn",
            AutomationIdKind::WorkflowRun => "workflow",
            AutomationIdKind::ToolInvocation => "tool",
            AutomationIdKind::CapabilityInvocation => "capability",
            AutomationIdKind::MemoryRecord => "memory",
            AutomationIdKind::ApprovalGrant => "approval",
        };
        Ok(format!(
            "{prefix}-{}",
            yss_project_identity::OperationId::new()
        ))
    }
}

fn initialize_harness_state(
    app_dir: std::path::PathBuf,
    application: yss_application::execution::ApplicationState,
) -> Result<yss_api::HarnessRuntimeState, HarnessInitializationError> {
    let captured = application.capture_session()?;
    let current_project = yss_automation_contract::ProjectSessionBinding::new(
        captured.project_instance_id().clone(),
        captured.project_session_id().clone(),
    );
    let store = Arc::new(tauri::async_runtime::block_on(
        yss_statistical_harness_sqlite::SqliteHarnessStore::connect(app_dir),
    )?);
    let channels = Arc::new(yss_api::HarnessChannelHub::new());
    let agent_driver = Arc::new(yss_agent_rig::ConfigurableAgentDriver::new());
    let clock = Arc::new(SystemHarnessClock);
    tauri::async_runtime::block_on(
        yss_statistical_harness::install_builtin_statistical_knowledge(store.clone(), clock.now()),
    )?;
    let host = Arc::new(yss_statistical_harness::HarnessHost::new(
        yss_statistical_harness::HarnessPorts {
            agent_driver: agent_driver.clone(),
            capability_gateway: Arc::new(application),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: channels.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store,
            clock,
            ids: Arc::new(HarnessIdGenerator),
        },
    )?);
    tauri::async_runtime::block_on(host.reconcile_project_session(&current_project))?;
    tauri::async_runtime::block_on(host.recover_workflows())?;
    Ok(yss_api::HarnessRuntimeState::new(
        host,
        channels,
        agent_driver,
    ))
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

    if let Err(error) = tauri::Builder::default()
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
            let graph_projection_runtime =
                yss_api::GraphProjectionRuntime::initialize(application_state.clone())
                    .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(application_state.clone());
            app.manage(graph_projection_runtime);

            let app_dir = app.path().app_data_dir()?;
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = yss_settings::SettingsStore::open(settings_path)
                .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(settings);
            let harness_state = initialize_harness_state(app_dir.clone(), application_state)
                .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(harness_state);
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
                if let Some(win) = app.get_webview_window("main")
                    && let Err(show_error) = win.show()
                {
                    tracing::warn!(
                        target: "yssbi::window_state",
                        diagnostic_domain = "ui",
                        error = %show_error,
                        "Failed to show main window after state restoration failure"
                    );
                }
            }
            app.manage(window_state_store);

            Ok(())
        })
        .invoke_handler(yss_api::invoke_handler())
        .run(tauri::generate_context!())
    {
        tracing::error!(
            target: "yssbi::application",
            diagnostic_domain = "system",
            diagnostic_event = "applicationRuntimeFailed",
            error = %error,
            "Tauri application runtime failed"
        );
    }
}
