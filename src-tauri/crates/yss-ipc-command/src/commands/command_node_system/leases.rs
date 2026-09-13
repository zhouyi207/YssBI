use super::results::result_query_command_error;
use crate::commands::execution_dto::ResultDescriptorDto;
use crate::error::CommandError;
use crate::schema::result::ResultReferenceDto;
use serde::Serialize;
use tauri::{State, WebviewWindow};
use yss_application::execution::ApplicationState;
use yss_execution::result::StoredResultSnapshot;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultLeaseDto {
    lease_id: String,
    descriptor: ResultDescriptorDto,
}

fn lease_id(value: &str) -> Result<uuid::Uuid, CommandError> {
    uuid::Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| CommandError::expected("invalid_result_lease"))
}

fn receipt(id: uuid::Uuid, snapshot: StoredResultSnapshot) -> Result<ResultLeaseDto, CommandError> {
    Ok(ResultLeaseDto {
        lease_id: id.to_string(),
        descriptor: ResultDescriptorDto::from_execution(
            snapshot.provenance().result_id(),
            &snapshot,
        )
        .map_err(|_| CommandError::expected("result_value_not_json"))?,
    })
}

#[tauri::command]
pub fn retain_result(
    window: WebviewWindow,
    state: State<'_, ApplicationState>,
    reference: ResultReferenceDto,
    lease: String,
    handoff: Option<String>,
) -> Result<ResultLeaseDto, CommandError> {
    let id = lease_id(&lease)?;
    if handoff
        .as_ref()
        .is_some_and(|target| target.is_empty() || target.len() > 256 || target == window.label())
    {
        return Err(CommandError::expected("invalid_result_lease"));
    }
    let snapshot = state
        .retain_result(
            reference.try_into()?,
            id,
            window.label(),
            handoff.as_deref(),
        )
        .map_err(result_query_command_error)?;
    receipt(id, snapshot)
}

#[tauri::command]
pub fn claim_result_lease(
    window: WebviewWindow,
    state: State<'_, ApplicationState>,
    lease: String,
) -> Result<ResultLeaseDto, CommandError> {
    let id = lease_id(&lease)?;
    let snapshot = state
        .claim_result_lease(id, window.label())
        .map_err(result_query_command_error)?;
    receipt(id, snapshot)
}

#[tauri::command]
pub fn release_result_lease(
    window: WebviewWindow,
    state: State<'_, ApplicationState>,
    lease: String,
) -> Result<(), CommandError> {
    state
        .release_result_lease(lease_id(&lease)?, window.label())
        .map_err(result_query_command_error)
}

#[tauri::command]
pub fn reconcile_result_leases(
    window: WebviewWindow,
    state: State<'_, ApplicationState>,
    leases: Vec<String>,
) -> Result<(), CommandError> {
    let leases = leases
        .iter()
        .map(|value| lease_id(value))
        .collect::<Result<_, _>>()?;
    state.reconcile_result_leases(window.label(), &leases);
    Ok(())
}
