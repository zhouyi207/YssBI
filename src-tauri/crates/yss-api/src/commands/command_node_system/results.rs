use super::common::parse_opaque_u64;
use crate::commands::execution_dto::{
    ResultDescriptorDto, ResultPageDto, ResultValueDto, runtime_value_to_json,
};
use crate::error::CommandError;
use serde::Serialize;
use tauri::State;
use yss_application::execution::result_query::{ResultPinQuery, ResultQueryApplicationError};
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_execution::result::{ResultId, StoredResult};
use yss_execution::value::RuntimeValue;

pub(super) const MAX_INLINE_RESULT_JSON_BYTES: usize = 64 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultPagingErrorDetails {
    result_id: String,
    value_kind: &'static str,
}

fn result_query_command_error(error: ResultQueryApplicationError) -> CommandError {
    match error {
        ResultQueryApplicationError::SessionCapture(error) => session_capture_command_error(error),
        ResultQueryApplicationError::SessionChanged => {
            CommandError::expected("stale_project_lifecycle")
        }
        ResultQueryApplicationError::InvalidPageRequest => {
            CommandError::expected("invalid_result_page_request")
        }
        ResultQueryApplicationError::PageTooLarge => {
            CommandError::expected("result_page_too_large")
        }
        ResultQueryApplicationError::Relation(error) => {
            CommandError::diagnosed("result_source_read_failed", format!("{error:?}"))
        }
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

#[tauri::command]
pub fn get_result_descriptor(
    state: State<'_, ApplicationState>,
    result_id: String,
) -> Result<Option<ResultDescriptorDto>, CommandError> {
    let result_id = ResultId::from_existing(parse_opaque_u64("resultId", &result_id)?);
    state
        .query_result(result_id)
        .map_err(result_query_command_error)?
        .map(|snapshot| ResultDescriptorDto::from_execution(result_id, &snapshot))
        .transpose()
        .map_err(|_| {
            CommandError::diagnosed("result_source_read_failed", result_id.get().to_string())
        })
}

#[tauri::command]
pub fn get_result_value(
    state: State<'_, ApplicationState>,
    result_id: String,
) -> Result<Option<ResultValueDto>, CommandError> {
    let result_id = ResultId::from_existing(parse_opaque_u64("resultId", &result_id)?);
    let Some(result) = state
        .query_result(result_id)
        .map_err(result_query_command_error)?
    else {
        return Ok(None);
    };
    if matches!(
        result.value().value(),
        StoredResult::Runtime(
            RuntimeValue::List(_) | RuntimeValue::Relation(_) | RuntimeValue::Series(_)
        )
    ) {
        return Err(result_requires_paging(result_id, "sequence"));
    }
    let value = match result.value().value() {
        StoredResult::Runtime(value) => runtime_value_to_json(value)
            .map_err(|_| CommandError::expected("result_value_not_json"))?,
        StoredResult::Scalar(value) => runtime_value_to_json(&RuntimeValue::Decimal(*value))
            .map_err(|_| CommandError::expected("result_value_not_json"))?,
        StoredResult::Text(value) => serde_json::Value::String(value.to_string()),
        StoredResult::Empty => serde_json::Value::Null,
        StoredResult::Categorized { .. } => {
            return Err(CommandError::expected("result_value_not_json"));
        }
    };
    let encoded_size = serde_json::to_vec(&value)
        .map_err(|_| CommandError::expected("result_value_not_json"))?
        .len();
    if encoded_size > MAX_INLINE_RESULT_JSON_BYTES {
        return Err(result_requires_paging(result_id, "scalar"));
    }
    Ok(Some(ResultValueDto::Value(value)))
}

#[tauri::command]
pub async fn get_result_page(
    state: State<'_, ApplicationState>,
    result_id: String,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultPageDto>, CommandError> {
    let result_id = ResultId::from_existing(parse_opaque_u64("resultId", &result_id)?);
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .query_result_page(result_id, offset, limit)
            .map_err(result_query_command_error)?
            .map(|page| {
                ResultPageDto::from_application(result_id, page)
                    .map_err(|_| CommandError::expected("result_value_not_json"))
            })
            .transpose()
    })
    .await
    .map_err(|error| CommandError::diagnosed("result_source_read_failed", format!("{error:?}")))?
}

#[tauri::command]
pub fn get_pin_result(
    state: State<'_, ApplicationState>,
    graph_path: String,
    output: crate::schema::graph_mutation::PortAddressDto,
) -> Result<Option<ResultDescriptorDto>, CommandError> {
    let graph_path = yss_graph_document::GraphResourcePath::new(graph_path)
        .map_err(|_| CommandError::expected("invalid_graph_resource_path"))?;
    let output = output
        .try_into()
        .map_err(|_| CommandError::expected("invalid_output"))?;
    let result = state
        .query_pin_result(ResultPinQuery::new(graph_path, output))
        .map_err(result_query_command_error)?;
    result
        .map(|snapshot| {
            ResultDescriptorDto::from_execution(snapshot.provenance().result_id(), &snapshot)
        })
        .transpose()
        .map_err(|error| CommandError::diagnosed("result_source_read_failed", format!("{error:?}")))
}

fn result_requires_paging(result_id: ResultId, value_kind: &'static str) -> CommandError {
    CommandError::expected("result_requires_paging").with_details(ResultPagingErrorDetails {
        result_id: result_id.get().to_string(),
        value_kind,
    })
}
