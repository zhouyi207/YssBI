use tauri::{Manager, State, WebviewWindow};
use yss_application::{activity_panel, execution::ApplicationState};
use yss_plugin_runtime::PluginManager;
use yss_project_identity::ProjectInstanceId;

use crate::activity_panel_sync::{ActivityPanelSyncState, ActivityPanelUpdateDto};
use crate::{
    error::CommandError,
    schema::activity_panel::{ActivityPanelDocumentDto, ActivityPanelId},
};

pub(crate) fn validate_activity_query(
    locale: &str,
    cursor: Option<&str>,
) -> Result<(), CommandError> {
    if locale.len() > 64 {
        return Err(CommandError::expected("invalid_locale"));
    }
    if cursor.is_some_and(|cursor| cursor.is_empty() || cursor.len() > 64) {
        return Err(CommandError::expected("activity_panel_cursor_invalid"));
    }
    Ok(())
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
    validate_activity_query(&locale, cursor.as_deref())?;
    if !panel_id.is_project_scoped() && project_instance_id.is_some() {
        return Err(CommandError::expected("activity_panel_scope_invalid"));
    }
    let application = application.inner().clone();
    let manager = manager.inner().clone();
    let sync = window.state::<ActivityPanelSyncState>().inner().clone();
    let window_label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let document = match panel_id {
            ActivityPanelId::Project => match project_instance_id {
                Some(project) => application
                    .query_project_index(project, &locale, false)
                    .map_err(super::command_project::query::map_project_query_error)?
                    .activity_panels
                    .into_iter()
                    .next()
                    .expect("project document is always present"),
                None => activity_panel::project_activity_panel(None),
            },
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
