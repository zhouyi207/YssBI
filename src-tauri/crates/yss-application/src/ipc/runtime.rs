//! Construct shared IPC channels and install their command contexts.

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
}

impl CommandRuntime {
    pub(crate) fn harness_ports(&self, application: ApplicationState) -> HarnessTransportPorts {
        HarnessTransportPorts {
            capability_gateway: Arc::new(ApplicationCapabilityGateway::new(application)),
            event_sink: self.channels.clone(),
        }
    }

    pub(crate) fn install(self, app: &tauri::AppHandle, harness: HarnessServices) {
        app.manage(ActivityPanelSyncState::default());
        app.manage(crate::ipc::graph_editor_sync::GraphEditorSyncState::default());
        app.manage(crate::ipc::channel::graph_activity::GraphActivityChannels::default());
        app.manage(HarnessRuntimeState::new(
            harness.host,
            self.channels,
            harness.provider,
        ));
    }
}
