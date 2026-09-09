use serde::Deserialize;
use tauri::{Manager, State, WebviewWindow};
use yss_application::{activity_panel, execution::ApplicationState};
use yss_plugin_runtime::PluginManager;
use yss_project_identity::ProjectInstanceId;

use crate::activity_panel_sync::{ActivityPanelSyncState, ActivityPanelUpdateDto};
use crate::{error::CommandError, schema::activity_panel::ActivityPanelDocumentDto};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityPanelId {
    Project,
    Nodes,
    Commands,
    Plugins,
}

#[tauri::command]
pub async fn get_activity_panel_document(
    window: WebviewWindow,
    application: State<'_, ApplicationState>,
    manager: State<'_, PluginManager>,
    panel_id: ActivityPanelId,
    project_instance_id: Option<ProjectInstanceId>,
    locale: String,
    cursor: Option<String>,
) -> Result<ActivityPanelUpdateDto, CommandError> {
    if locale.len() > 64 {
        return Err(CommandError::expected("invalid_locale"));
    }
    if cursor
        .as_ref()
        .is_some_and(|cursor| cursor.is_empty() || cursor.len() > 64)
    {
        return Err(CommandError::expected("activity_panel_cursor_invalid"));
    }
    let application = application.inner().clone();
    let manager = manager.inner().clone();
    let sync = window.state::<ActivityPanelSyncState>().inner().clone();
    let window_label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let document = match panel_id {
            ActivityPanelId::Project => application
                .project_activity_panel(project_instance_id)
                .map_err(super::command_project::query::map_project_query_error)?,
            ActivityPanelId::Nodes => application
                .nodes_activity_panel(project_instance_id, locale.clone())
                .map_err(super::command_node_system::catalog_query_command_error)?,
            ActivityPanelId::Commands => activity_panel::commands_activity_panel(),
            ActivityPanelId::Plugins => activity_panel::plugins_activity_panel(
                &manager
                    .list()
                    .map_err(super::command_plugin::plugin_error)?,
            ),
        };
        sync.publish(
            &window_label,
            &locale,
            cursor.as_deref(),
            ActivityPanelDocumentDto::try_from(document)?,
        )
    })
    .await
    .map_err(CommandError::internal)?
}
