//! Construct shared IPC channels and install their command contexts.

use crate::ipc::channel::HarnessGraphClientHub;
use crate::runtime::{HarnessServices, HarnessTransportPorts};
use crate::session::ApplicationState;
use std::sync::Arc;
use tauri::Manager;
use yss_ipc_channel::HarnessChannelHub;

use crate::ipc::activity_panel_sync::ActivityPanelSyncState;
use crate::ipc::commands::{ApplicationCapabilityGateway, HarnessRuntimeState};

#[derive(Default)]
pub(crate) struct CommandRuntime {
    channels: Arc<HarnessChannelHub>,
    graph_clients: Arc<HarnessGraphClientHub>,
}

impl CommandRuntime {
    pub(crate) fn harness_ports(&self, application: ApplicationState) -> HarnessTransportPorts {
        HarnessTransportPorts {
            capability_gateway: Arc::new(ApplicationCapabilityGateway::new(
                application,
                self.graph_clients.clone(),
            )),
            event_sink: self.channels.clone(),
        }
    }

    pub(crate) fn install(self, app: &tauri::AppHandle, harness: HarnessServices) {
        app.manage(ActivityPanelSyncState::default());
        app.manage(HarnessRuntimeState::new(
            harness.host,
            self.channels,
            harness.provider,
            self.graph_clients,
        ));
    }
}
