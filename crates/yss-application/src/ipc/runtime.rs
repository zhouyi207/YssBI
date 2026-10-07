//! Construct shared IPC channels and install their command contexts.

use crate::runtime::{
    ApplicationPaths, ApplicationServices, HarnessServices, HarnessTransportPorts,
};
use crate::session::ApplicationState;
use std::sync::Arc;
use tauri::Manager;
use yss_ipc_channel::HarnessChannelHub;

use crate::harness::ApplicationCapabilityGateway;
use crate::ipc::activity_panel_sync::ActivityPanelSyncState;
use crate::ipc::commands::HarnessRuntimeState;

#[derive(Default)]
pub(crate) struct CommandRuntime {
    channels: Arc<HarnessChannelHub>,
}

impl CommandRuntime {
    pub(crate) fn harness_ports(
        &self,
        application: ApplicationState,
        app: &tauri::AppHandle,
    ) -> HarnessTransportPorts {
        let app = app.clone();
        HarnessTransportPorts {
            capability_gateway: Arc::new(ApplicationCapabilityGateway::new(
                application,
                Arc::new(move |mutation| {
                    let result =
                        crate::ipc::schema::application_event::resource_mutation_to_transport(
                            mutation,
                        );
                    if let Err(error) = yss_ipc_event::emit_project_event_result(
                        &app,
                        &yss_ipc_contract::event::Event::Project(Box::new(
                            yss_ipc_contract::event::EventProject::ResourceMutationCommitted {
                                result,
                            },
                        )),
                    ) {
                        tracing::warn!(domain = "Application", event = "harness_resource_publication_failed", error = ?error, "Resource committed but its UI notification failed");
                    }
                }),
            )),
            event_sink: self.channels.clone(),
        }
    }

    pub(crate) fn install(self, app: &tauri::AppHandle, harness: HarnessServices) {
        app.manage(ActivityPanelSyncState::default());
        app.manage(crate::ipc::channel::presentation::PresentationChannels::default());
        app.manage(crate::ipc::graph_editor_sync::GraphEditorSyncState::default());
        app.manage(crate::ipc::channel::graph_activity::GraphActivityChannels::default());
        app.manage(HarnessRuntimeState::new(
            harness.host,
            self.channels,
            harness.models,
            harness.knowledge,
        ));
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
        |application| ipc.harness_ports(application, app.handle()),
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
