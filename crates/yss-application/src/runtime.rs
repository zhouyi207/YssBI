//! Product runtime composition. Concrete adapters are selected here, outside use cases.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use yss_filesystem::watcher::WatcherState;
use yss_plugin_runtime::PluginManager;

use crate::database::samples::SampleCatalog;
use crate::plugins::PluginHostServices;
use crate::project::ProjectManagement;
use crate::session::ApplicationInitializationError;
use crate::session::ApplicationState;

mod harness;
pub use harness::{HarnessServices, HarnessStartupError, HarnessTransportPorts};

pub struct ApplicationPaths {
    pub app_data_dir: PathBuf,
    pub samples_dir: PathBuf,
}

pub struct ApplicationServices {
    pub application: ApplicationState,
    pub samples: SampleCatalog,
    pub harness: HarnessServices,
    pub projects: ProjectManagement,
    pub watcher: Mutex<WatcherState>,
    pub plugins: PluginManager,
}

#[derive(Debug, Error)]
pub enum ApplicationStartupError {
    #[error("application session initialization failed: {0}")]
    Application(#[from] ApplicationInitializationError),
    #[error("Harness initialization failed: {0}")]
    Harness(#[from] HarnessStartupError),
    #[error("project registry initialization failed")]
    ProjectRegistry(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ApplicationServices {
    pub async fn initialize(
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
            watcher: Mutex::new(watcher),
            plugins,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_harness_contract::{PrincipalId, ProjectSessionBinding};
    use yss_harness_core::test_support::{InMemoryHarnessStore, RejectingCapabilityGateway};

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn node_startup_error_preserves_the_assembly_failure() {
        let assembly = yss_node_catalog::BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "classical hypothesis test",
            value: "unsupported-test".into(),
        };
        let error = ApplicationStartupError::from(ApplicationInitializationError::from(
            crate::session::NodeCompositionError::from(
                yss_node_catalog::BuiltinInitializationError::from(assembly),
            ),
        ));

        assert_eq!(
            error.to_string(),
            "application session initialization failed: node components could not be assembled: built-in node definitions could not be constructed: unsupported built-in classical hypothesis test: 'unsupported-test'"
        );
    }

    #[tokio::test]
    async fn harness_startup_reports_persistence_failure_without_replacing_the_database() {
        let directory = TestDirectory(
            std::env::temp_dir().join(format!("yss-application-runtime-{}", uuid::Uuid::new_v4())),
        );
        let database_dir = directory.0.join("db");
        std::fs::create_dir_all(&database_dir).unwrap();
        let database_path = database_dir.join("statistical-harness.sqlite");
        let invalid_database = b"unreadable database fixture";
        std::fs::write(&database_path, invalid_database).unwrap();

        let result = ApplicationServices::initialize(
            ApplicationPaths {
                app_data_dir: directory.0.clone(),
                samples_dir: directory.0.join("samples"),
            },
            |_| HarnessTransportPorts {
                capability_gateway: Arc::new(RejectingCapabilityGateway),
                event_sink: Arc::new(InMemoryHarnessStore::default()),
            },
        )
        .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("an unreadable Harness database must fail startup"),
        };
        assert!(matches!(
            &error,
            ApplicationStartupError::Harness(HarnessStartupError::Persistence(failure))
                if failure.code == yss_harness_contract::PersistenceFailureCode::Unavailable
        ));
        assert_eq!(
            error.to_string(),
            "Harness initialization failed: Harness SQLite persistence could not be initialized: unavailable"
        );
        assert_eq!(std::fs::read(database_path).unwrap(), invalid_database);
    }

    #[test]
    fn harness_startup_preserves_the_host_persistence_failure_code() {
        let error = ApplicationStartupError::from(HarnessStartupError::from(
            crate::harness::HarnessInitializationError::from(yss_harness_core::HarnessError::from(
                yss_harness_contract::PersistenceFailure::new(
                    yss_harness_contract::PersistenceFailureCode::InvalidRecord,
                ),
            )),
        ));

        assert_eq!(
            error.to_string(),
            "Harness initialization failed: Harness application initialization failed: Harness host could not be initialized: harness persistence failed: invalid_record"
        );
    }

    #[tokio::test]
    async fn default_runtime_connects_persistent_services_without_a_desktop_host() {
        let directory = TestDirectory(
            std::env::temp_dir().join(format!("yss-application-runtime-{}", uuid::Uuid::new_v4())),
        );
        let model_settings = directory.0.join("settings/language-models.json");
        std::fs::create_dir_all(model_settings.parent().unwrap()).unwrap();
        std::fs::write(&model_settings, b"invalid model settings").unwrap();
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
        assert!(services.harness.models.catalog().await.is_err());
        assert_eq!(
            std::fs::read(&model_settings).unwrap(),
            b"invalid model settings"
        );
        std::fs::write(
            &model_settings,
            br#"{"providers":[],"defaultModel":null,"retiredCredentials":[]}"#,
        )
        .unwrap();
        assert!(
            services
                .harness
                .models
                .catalog()
                .await
                .unwrap()
                .providers
                .is_empty()
        );
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
