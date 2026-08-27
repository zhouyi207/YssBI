use super::common::parse_graph_path;
use crate::application::catalog_compatibility::{
    CatalogCompatibilityError, CatalogCompatibilityRequest,
};
use crate::error::CommandError;
use crate::node_system::catalog::LocalizedCatalogDto;
use crate::node_system::document::PortAddressDto;
use crate::project::ResourceRevision;
use crate::project::{ProjectFilesystemError, ProjectInstanceId, ProjectState};
use tauri::State;

pub(super) fn get_localized_node_catalog_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    locale: &str,
) -> Result<LocalizedCatalogDto, CommandError> {
    state
        .localized_catalog_snapshot(&project_instance_id, locale)
        .map_err(|error| match error {
            ProjectFilesystemError::StaleProjectLifecycle { .. } => {
                CommandError::expected("catalog_project_stale")
            }
            _ => CommandError::from(error),
        })
}

#[tauri::command]
pub fn get_localized_node_catalog(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
) -> Result<LocalizedCatalogDto, CommandError> {
    get_localized_node_catalog_from_state(state.inner(), project_instance_id, &locale)
}

fn catalog_compatibility_command_error(error: CatalogCompatibilityError) -> CommandError {
    match error {
        CatalogCompatibilityError::GraphRevisionConflict => {
            CommandError::expected("graph_revision_conflict")
        }
        CatalogCompatibilityError::CatalogProjectStale => {
            CommandError::expected("catalog_project_stale")
        }
        CatalogCompatibilityError::CompatibleSourceInvalid => {
            CommandError::expected("compatible_source_invalid")
        }
        CatalogCompatibilityError::GraphNotLoaded => CommandError::expected("graph_not_loaded"),
        CatalogCompatibilityError::Project(error) => CommandError::from(error),
    }
}

#[tauri::command]
pub fn get_compatible_node_catalog(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    graph_revision: ResourceRevision,
    source_port: PortAddressDto,
    locale: String,
) -> Result<LocalizedCatalogDto, CommandError> {
    crate::application::catalog_compatibility::get_compatible_node_catalog(
        state.inner(),
        CatalogCompatibilityRequest {
            project_instance_id,
            graph_path: parse_graph_path(graph_path)?,
            graph_revision,
            source_port,
            locale,
        },
    )
    .map_err(catalog_compatibility_command_error)
}
