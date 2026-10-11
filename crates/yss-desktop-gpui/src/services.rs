//! Native composition and scheduling of the existing application services.
mod layout_store;
pub(crate) use layout_store::Viewport;
mod paths;
use std::sync::Arc;

use anyhow::Result;
use tokio::{runtime::Handle, sync::broadcast};
use yss_application::{
    events::CommittedResourceMutation,
    harness::ApplicationCapabilityGateway,
    runtime::{ApplicationServices, HarnessTransportPorts},
};
use yss_harness_contract::{
    HarnessEventEnvelope, HarnessEventSinkPort, PersistenceFailure, PersistenceFuture,
};

#[derive(Clone)]
pub enum NativeEvent {
    ModelsChanged,
    Resource(CommittedResourceMutation),
    Harness(HarnessEventEnvelope),
    Graph(
        yss_project_identity::ProjectInstanceId,
        yss_application::graph::editing::GraphActivity,
    ),
    Index(yss_project::ProjectIndexInvalidation),
    Ui(
        yss_project_identity::ProjectInstanceId,
        uuid::Uuid,
        yss_ui_contract::UiEvent,
    ),
}

struct NativeHarnessEvents(broadcast::Sender<NativeEvent>);

impl HarnessEventSinkPort for NativeHarnessEvents {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            // Harness persists its event before delivery. Views recover from its history after lag.
            let _ = self.0.send(NativeEvent::Harness(event.clone()));
            Ok(())
        })
    }
}

pub struct NativeServices {
    pub application: ApplicationServices,
    pub executor: Handle,
    events: broadcast::Sender<NativeEvent>,
    pub logging: yss_logging::LogCollection,
    pub layouts: Arc<layout_store::LayoutStore>,
    pub preferences: Arc<yss_settings::SettingsStore>,
    pub preference_load_failed: bool,
}

impl NativeServices {
    pub async fn initialize(executor: Handle) -> Result<Arc<Self>> {
        let paths = paths::NativePaths::resolve()?;
        let preferences = Arc::new(yss_settings::SettingsStore::new(
            &paths.application.app_data_dir,
        ));
        let reader = preferences.clone();
        let loaded = executor.spawn_blocking(move || reader.load()).await?;
        let preference_load_failed = loaded.is_err();
        crate::text::set_locale(preferences.snapshot().language.locale());
        if let Err(error) = loaded {
            tracing::warn!(
                code = "native_preferences_read_failed",
                %error,
                "Native preferences could not be read"
            );
        }
        let layouts = Arc::new(layout_store::LayoutStore::new(
            &paths.application.app_data_dir,
        ));
        let settings = preferences.snapshot();
        let retention = yss_logging::LogRetentionPolicy::new(
            settings.logs.retention_days,
            settings.logs.max_storage_mib,
        )?;
        let logging =
            yss_logging::LogCollection::initialize_with_retention(Some(paths.logs), retention)?;
        let (events, _) = broadcast::channel(512);
        let publisher = events.clone();
        let application = ApplicationServices::initialize(paths.application, move |application| {
            let committed = publisher.clone();
            HarnessTransportPorts {
                capability_gateway: Arc::new(ApplicationCapabilityGateway::new(
                    application,
                    Arc::new(move |mutation| {
                        let _ = committed.send(NativeEvent::Resource(mutation.clone()));
                    }),
                )),
                event_sink: Arc::new(NativeHarnessEvents(publisher)),
            }
        })
        .await?;
        Ok(Arc::new(Self {
            application,
            executor,
            events,
            logging,
            layouts,
            preferences,
            preference_load_failed,
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<NativeEvent> {
        self.events.subscribe()
    }

    pub fn publish_resource(&self, mutation: CommittedResourceMutation) {
        let _ = self.events.send(NativeEvent::Resource(mutation));
    }

    pub fn models_changed(&self) {
        let _ = self.events.send(NativeEvent::ModelsChanged);
    }

    pub fn graph_subscription(
        &self,
        project: &yss_project_identity::ProjectInstanceId,
    ) -> Result<yss_application::graph::editing::GraphActivitySubscription> {
        let events = self.events.clone();
        let identity = project.clone();
        Ok(self.application.application.subscribe_graph_activity(
            project,
            Arc::new(move |event| {
                let _ = events.send(NativeEvent::Graph(identity.clone(), event));
            }),
        )?)
    }

    pub fn workbench_binding(
        &self,
        project: &yss_project_identity::ProjectInstanceId,
        delivery: uuid::Uuid,
    ) -> Result<yss_application::presentation::WorkbenchBinding> {
        let events = self.events.clone();
        let identity = project.clone();
        Ok(self.application.application.attach_workbench(
            project,
            "main".into(),
            Arc::new(move |event| {
                let _ = events.send(NativeEvent::Ui(identity.clone(), delivery, event));
            }),
        )?)
    }

    pub fn watch_project(
        self: &Arc<Self>,
        project: &yss_project_identity::ProjectInstanceId,
    ) -> Result<()> {
        if let Some(path) = self
            .application
            .application
            .query_project_path(project.clone())?
            && let Err(_error) = self.application.application.watch_project_changes(
                &self.application.watcher,
                &path,
                project,
                Arc::new(ProjectChanges {
                    owner: Arc::downgrade(self),
                    project: project.clone(),
                }),
            )
        {
            tracing::warn!(
                code = "native_project_watch_failed",
                "Native project watcher could not start"
            );
        }
        Ok(())
    }

    /// Native commands run off the UI thread while retaining the application owner.
    pub fn run<T: Send + 'static>(
        self: &Arc<Self>,
        operation: impl FnOnce(&ApplicationServices) -> Result<T> + Send + 'static,
    ) -> tokio::task::JoinHandle<Result<T>> {
        let owner = self.clone();
        self.executor
            .spawn_blocking(move || operation(&owner.application))
    }
}

struct ProjectChanges {
    owner: std::sync::Weak<NativeServices>,
    project: yss_project_identity::ProjectInstanceId,
}

impl yss_filesystem::watcher::ChangeSink for ProjectChanges {
    fn publish(&self, change: yss_filesystem::watcher::ObservedChange) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        match owner
            .application
            .application
            .reconcile_project_change(&self.project, change.change)
        {
            Ok(Some(invalidation)) => {
                let _ = owner.events.send(NativeEvent::Index(invalidation));
            }
            Ok(None) => {}
            Err(_error) => {
                tracing::debug!(
                    code = "native_project_watch_observation_was_rejected",
                    "Native project watch observation was rejected"
                )
            }
        }
    }
}
