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
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};
use yss_automation_contract::{
    AutomationIdKind, ClockPort, IdGenerationFailure, IdGeneratorPort, UnixMillis,
};

// ==================== 应用入口 ====================

const WINDOW_STATE_FLAGS: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

fn window_state_key(label: &str) -> &str {
    label.split_once('-').map_or(label, |(kind, _)| kind)
}

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
        Ok(format!("{prefix}-{}", uuid::Uuid::new_v4()))
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
    let graph_clients = Arc::new(yss_api::HarnessGraphClientHub::new());
    let agent_driver = Arc::new(yss_agent_rig::ConfigurableAgentDriver::new());
    let clock = Arc::new(SystemHarnessClock);
    tauri::async_runtime::block_on(
        yss_statistical_harness::install_builtin_statistical_knowledge(store.clone(), clock.now()),
    )?;
    let host = Arc::new(yss_statistical_harness::HarnessHost::new(
        yss_statistical_harness::HarnessPorts {
            agent_driver: agent_driver.clone(),
            capability_gateway: Arc::new(yss_api::ApplicationCapabilityGateway::new(
                application,
                graph_clients.clone(),
            )),
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
    tauri::async_runtime::block_on(host.recover_interrupted_turns())?;
    tauri::async_runtime::block_on(host.reconcile_project_session(&current_project))?;
    tauri::async_runtime::block_on(host.recover_workflows())?;
    Ok(yss_api::HarnessRuntimeState::new(
        host,
        channels,
        agent_driver,
        graph_clients,
    ))
}

fn initialize_application_state(
    project_state: Arc<yss_project::ProjectState>,
) -> Result<yss_application::execution::ApplicationState, ApplicationInitializationError> {
    let scientific_backend: Arc<dyn yss_sci_contract::scientific::ScientificBackend> =
        Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(WINDOW_STATE_FLAGS)
                .map_label(window_state_key)
                .with_filter(|kind| {
                    matches!(
                        kind,
                        "main" | "dataview" | "logs" | "plot" | "inspect" | "info"
                    )
                })
                // Main is restored explicitly in setup before it is shown.
                .skip_initial_state("main")
                .build(),
        )
        // 注册全局状态管理器
        .manage(yss_project_watcher::ProjectWatcherState::new(
            std::sync::Arc::new(yss_project_watcher_notify::NotifyProjectFileWatcher::new()),
        ))
        .manage(yss_project_progress::ProjectTaskCancellationRegistry::new())
        .manage(yss_api::ActivityPanelSyncState::default())
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
            app.manage(application_state.clone());
            app.manage(yss_application::database::samples::SampleCatalog::new(
                app.path()
                    .resolve("resources/samples", tauri::path::BaseDirectory::Resource)?,
            ));

            let app_dir = app.path().app_data_dir()?;
            let harness_state =
                initialize_harness_state(app_dir.clone(), application_state.clone())
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
            let plugins = yss_plugin_runtime::PluginManager::initialize(
                &app_dir,
                Arc::new(yss_application::plugins::PluginHostServices::new(
                    application_state,
                )),
            );
            app.manage(plugins);

            if let Some(win) = app.get_webview_window("main") {
                if let Err(error) = win.restore_state(WINDOW_STATE_FLAGS) {
                    tracing::warn!(
                        target: "yssbi::window_state",
                        diagnostic_domain = "ui",
                        error = %error,
                        "Failed to restore main window geometry"
                    );
                }
                if let Err(error) = win.show() {
                    tracing::warn!(
                        target: "yssbi::window_state",
                        diagnostic_domain = "ui",
                        error = %error,
                        "Failed to show main window"
                    );
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Some(application) =
                    window.try_state::<yss_application::execution::ApplicationState>()
            {
                application.close_result_owner(window.label());
            }
        })
        .invoke_handler(yss_api::invoke_handler())
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|app, event| {
            // The manager has removed the destroyed window before this callback. Saving
            // here cannot query a dead window or participate in frontend close decisions.
            if matches!(
                event,
                tauri::RunEvent::WindowEvent {
                    event: tauri::WindowEvent::Destroyed,
                    ..
                }
            ) && let Err(error) = app.save_window_state(WINDOW_STATE_FLAGS)
            {
                tracing::warn!(
                    target: "yssbi::window_state",
                    diagnostic_domain = "ui",
                    error = %error,
                    "Failed to persist window geometry"
                );
            }
        }),
        Err(error) => tracing::error!(
            target: "yssbi::application",
            diagnostic_domain = "system",
            diagnostic_event = "applicationRuntimeFailed",
            error = %error,
            "Tauri application runtime failed"
        ),
    }
}

#[cfg(test)]
mod window_state_tests {
    use super::{WINDOW_STATE_FLAGS, window_state_key};
    use tauri_plugin_window_state::StateFlags;

    #[test]
    fn window_instances_share_geometry_by_kind_without_restoring_visibility_or_decorations() {
        for kind in ["main", "dataview", "logs", "plot", "inspect", "info"] {
            assert_eq!(window_state_key(kind), kind);
            assert_eq!(window_state_key(&format!("{kind}-first-instance")), kind);
            assert_eq!(window_state_key(&format!("{kind}-second-instance")), kind);
        }
        assert!(
            !WINDOW_STATE_FLAGS
                .intersects(StateFlags::VISIBLE | StateFlags::DECORATIONS | StateFlags::FULLSCREEN)
        );
    }
}
