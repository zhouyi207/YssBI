//! Product runtime composition. Concrete adapters are selected here, outside use cases.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use thiserror::Error;
use yss_filesystem::watcher::WatcherState;
use yss_plugin_runtime::PluginManager;

use crate::database::samples::SampleCatalog;
use crate::ipc::CommandRuntime;
use crate::plugins::PluginHostServices;
use crate::project::ProjectManagement;
use crate::session::ApplicationInitializationError;
use crate::session::ApplicationState;

mod harness;
pub use harness::{HarnessServices, HarnessStartupError, HarnessTransportPorts};

struct ApplicationPaths {
    app_data_dir: PathBuf,
    samples_dir: PathBuf,
}

struct ApplicationServices {
    application: ApplicationState,
    samples: SampleCatalog,
    harness: HarnessServices,
    projects: ProjectManagement,
    watcher: WatcherState,
    plugins: PluginManager,
}

#[derive(Debug, Error)]
pub enum ApplicationStartupError {
    #[error("application session initialization failed")]
    Application(#[from] ApplicationInitializationError),
    #[error("Harness initialization failed")]
    Harness(#[from] HarnessStartupError),
    #[error("project registry initialization failed")]
    ProjectRegistry(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ApplicationServices {
    async fn initialize(
        paths: ApplicationPaths,
        connect_harness: impl FnOnce(ApplicationState) -> HarnessTransportPorts,
    ) -> Result<Self, ApplicationStartupError> {
        let watcher = WatcherState::new(Arc::new(
            yss_filesystem::watcher::notify::NotifyFileWatcher::with_filter(
                yss_project::is_project_index_watch_path,
            ),
        ));
        let application = ApplicationState::initialize()?;
        let samples = SampleCatalog::new(paths.samples_dir);
        let transport = connect_harness(application.clone());
        let harness =
            harness::initialize(paths.app_data_dir.clone(), &application, transport).await?;
        let store = yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(
            paths.app_data_dir.clone(),
        )
        .await
        .map_err(|error| ApplicationStartupError::ProjectRegistry(Box::new(error)))?;
        let registry_path = store.path().to_path_buf();
        let projects = ProjectManagement::new(Arc::new(store), registry_path);
        let plugins = PluginManager::initialize(
            &paths.app_data_dir,
            Arc::new(PluginHostServices::new(application.clone())),
        );
        Ok(Self {
            application,
            samples,
            harness,
            projects,
            watcher,
            plugins,
        })
    }
}

pub fn initialize(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let ipc = CommandRuntime::default();
    let services = tauri::async_runtime::block_on(ApplicationServices::initialize(
        ApplicationPaths {
            app_data_dir: app.path().app_data_dir()?,
            samples_dir: app
                .path()
                .resolve("resources/samples", tauri::path::BaseDirectory::Resource)?,
        },
        |application| ipc.harness_ports(application),
    ))?;
    app.manage(services.application);
    app.manage(services.samples);
    app.manage(services.projects);
    app.manage(services.watcher);
    app.manage(services.plugins);
    ipc.install(app.handle(), services.harness);

    // Configured windows have already run the Window State plugin's restoration hook.
    if let Some(window) = app.get_webview_window("main")
        && let Err(error) = window.show()
    {
        tracing::warn!(
            target: "yssbi::window_state",
            log_domain = "ui",
            error = %error,
            "Failed to show main window"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_automation_contract::{PrincipalId, ProjectSessionBinding};
    use yss_statistical_harness::test_support::{InMemoryHarnessStore, RejectingCapabilityGateway};

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn default_runtime_connects_persistent_services_without_a_desktop_host() {
        let directory = TestDirectory(
            std::env::temp_dir().join(format!("yss-application-runtime-{}", uuid::Uuid::new_v4())),
        );
        let services = ApplicationServices::initialize(
            ApplicationPaths {
                app_data_dir: directory.0.clone(),
                samples_dir: directory.0.join("samples"),
            },
            |_| HarnessTransportPorts {
                capability_gateway: Arc::new(RejectingCapabilityGateway),
                event_sink: Arc::new(InMemoryHarnessStore::default()),
            },
        )
        .await
        .unwrap();

        assert!(services.projects.path().is_file());
        assert!(services.projects.list_projects().await.unwrap().is_empty());
        assert!(services.plugins.list().unwrap().is_empty());
        assert!(!services.harness.provider.is_configured());
        let captured = services.application.capture_session().unwrap();
        let session = services
            .application
            .create_harness_session(
                &services.harness.host,
                PrincipalId::try_new("local-user").unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            session.project,
            ProjectSessionBinding::new(
                captured.project_instance_id().clone(),
                captured.project_session_id().clone(),
            )
        );
        assert!(
            !services
                .harness
                .host
                .events_after(&session.id, 0)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
