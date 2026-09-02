use super::common::parse_graph_path;
use crate::error::CommandError;
use crate::schema::catalog::LocalizedCatalogDto;
use tauri::State;
use yss_application::catalog_query::{
    CatalogQueryApplicationError, CompatibleCatalogRequest, LocalizedCatalogRequest,
    ProjectCatalogReadError,
};
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_project_identity::ProjectInstanceId;

fn catalog_query_command_error(error: CatalogQueryApplicationError) -> CommandError {
    match error {
        CatalogQueryApplicationError::SessionCapture(error) => session_capture_command_error(error),
        CatalogQueryApplicationError::SessionChanged => {
            CommandError::expected("stale_project_lifecycle")
        }
        CatalogQueryApplicationError::CatalogProjectStale => {
            CommandError::expected("catalog_project_stale")
        }
        CatalogQueryApplicationError::Project(error) => project_catalog_command_error(error),
        CatalogQueryApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        CatalogQueryApplicationError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        CatalogQueryApplicationError::Graph(error) => match error {
            yss_application::catalog_query::GraphCatalogQueryError::GraphNotLoaded { .. } => {
                CommandError::expected("graph_not_loaded")
            }
            yss_application::catalog_query::GraphCatalogQueryError::InvalidDraft(_) => {
                CommandError::expected("compatible_source_invalid")
            }
            yss_application::catalog_query::GraphCatalogQueryError::CompatibleSourceInvalid => {
                CommandError::expected("compatible_source_invalid")
            }
        },
    }
}

fn session_capture_command_error(error: SessionCaptureError) -> CommandError {
    match error {
        SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
        SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
            .with_details(super::common::RecoveryRequiredDetails {
                recovery_required: true,
            }),
    }
}

fn project_catalog_command_error(error: ProjectCatalogReadError) -> CommandError {
    match error {
        ProjectCatalogReadError::ProjectLifecycleChanged
        | ProjectCatalogReadError::CatalogResourceStale { .. } => {
            CommandError::expected("catalog_project_stale")
        }
        ProjectCatalogReadError::AdmissionClosed => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        ProjectCatalogReadError::RecoveryRequired => CommandError::expected(
            "project_recovery_required",
        )
        .with_details(super::common::RecoveryRequiredDetails {
            recovery_required: true,
        }),
        ProjectCatalogReadError::FilesystemBusy => {
            CommandError::expected("project_filesystem_busy")
        }
        ProjectCatalogReadError::ReadFailed(source) | ProjectCatalogReadError::Internal(source) => {
            CommandError::diagnosed("catalog_project_read_failed", source)
        }
    }
}

#[tauri::command]
pub fn get_localized_node_catalog(
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
) -> Result<LocalizedCatalogDto, CommandError> {
    state
        .localized_node_catalog(LocalizedCatalogRequest::new(project_instance_id, locale))
        .map(LocalizedCatalogDto::from)
        .map_err(catalog_query_command_error)
}

#[tauri::command]
pub fn get_compatible_node_catalog(
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    document: yss_graph_document::GraphDocument,
    source_port: crate::schema::graph_mutation::PortAddressDto,
    locale: String,
) -> Result<LocalizedCatalogDto, CommandError> {
    let source_port = source_port
        .try_into()
        .map_err(|_| CommandError::expected("invalid_output"))?;
    state
        .compatible_node_catalog(CompatibleCatalogRequest::new(
            project_instance_id,
            parse_graph_path(graph_path)?,
            document,
            source_port,
            locale,
        ))
        .map(LocalizedCatalogDto::from)
        .map_err(catalog_query_command_error)
}
