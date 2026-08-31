use super::common::parse_opaque_u64;
use crate::commands::execution_dto::{
    PinResultEntryDto, ResultDescriptorDto, ResultPageDto, ResultValueDto, ResultValueKindDto,
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
        ResultQueryApplicationError::Execution(
            yss_execution::result::ExecutionResultQueryError::ResultSourceReadFailed { result_id },
        ) => CommandError::diagnosed("result_source_read_failed", result_id.get().to_string()),
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
        .map(|result| {
            result
                .as_deref()
                .map(|result| ResultDescriptorDto::from_execution(result_id, result))
        })
        .map_err(result_query_command_error)
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
    if matches!(result.value(), StoredResult::Runtime(RuntimeValue::List(_))) {
        return Err(result_requires_paging(result_id, "sequence"));
    }
    let values = execution_result_values(&result, 0, 1)?;
    let value = values
        .into_vec()
        .into_iter()
        .next()
        .unwrap_or(serde_json::Value::Null);
    let encoded_size = serde_json::to_vec(&value)
        .map_err(|_| CommandError::expected("result_value_not_json"))?
        .len();
    if encoded_size > MAX_INLINE_RESULT_JSON_BYTES {
        return Err(result_requires_paging(result_id, "scalar"));
    }
    Ok(Some(ResultValueDto::Value(value)))
}

#[tauri::command]
pub fn get_result_page(
    state: State<'_, ApplicationState>,
    result_id: String,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultPageDto>, CommandError> {
    let result_id = ResultId::from_existing(parse_opaque_u64("resultId", &result_id)?);
    let Some(result) = state
        .query_result(result_id)
        .map_err(result_query_command_error)?
    else {
        return Ok(None);
    };
    let total_count = execution_result_len(&result);
    let values = execution_result_values(&result, offset, limit)?;
    Ok(Some(ResultPageDto::from_execution(
        result_id,
        offset.min(total_count),
        limit,
        execution_result_kind(&result),
        total_count,
        values,
    )))
}

#[tauri::command]
pub fn get_pin_result_history(
    state: State<'_, ApplicationState>,
    graph_path: String,
    output: crate::schema::graph_mutation::PortAddressDto,
) -> Result<Box<[PinResultEntryDto]>, CommandError> {
    let graph_path = yss_graph_document::GraphResourcePath::new(graph_path)
        .map_err(|_| CommandError::expected("invalid_graph_resource_path"))?;
    let output = output
        .try_into()
        .map_err(|_| CommandError::expected("invalid_output"))?;
    state
        .query_pin_result_history(ResultPinQuery::new(graph_path, output))
        .map(|history| {
            history
                .into_vec()
                .into_iter()
                .map(|snapshot| {
                    let (entry, result) = snapshot.into_parts();
                    PinResultEntryDto::from_execution(entry, result.as_ref())
                })
                .collect::<Vec<_>>()
                .into_boxed_slice()
        })
        .map_err(result_query_command_error)
}

fn result_requires_paging(result_id: ResultId, value_kind: &'static str) -> CommandError {
    CommandError::expected("result_requires_paging").with_details(ResultPagingErrorDetails {
        result_id: result_id.get().to_string(),
        value_kind,
    })
}

fn execution_result_kind(result: &StoredResult) -> ResultValueKindDto {
    match result.value() {
        StoredResult::Categorized { value, .. } => execution_result_kind(value),
        StoredResult::Runtime(RuntimeValue::List(_)) => ResultValueKindDto::Sequence,
        _ => ResultValueKindDto::Scalar,
    }
}

fn execution_result_len(result: &StoredResult) -> usize {
    match result.value() {
        StoredResult::Categorized { value, .. } => execution_result_len(value),
        StoredResult::Runtime(RuntimeValue::List(values)) => values.len(),
        StoredResult::Empty => 0,
        _ => 1,
    }
}

fn execution_result_values(
    result: &StoredResult,
    offset: usize,
    limit: usize,
) -> Result<Box<[serde_json::Value]>, CommandError> {
    let values: Vec<RuntimeValue> = match result.value() {
        StoredResult::Categorized { value, .. } => {
            return execution_result_values(value, offset, limit);
        }
        StoredResult::Runtime(RuntimeValue::List(values)) => {
            values.iter().skip(offset).take(limit).cloned().collect()
        }
        StoredResult::Runtime(value) if offset == 0 && limit > 0 => vec![value.clone()],
        StoredResult::Runtime(value) => (offset == 0 && limit > 0)
            .then_some(value.clone())
            .into_iter()
            .collect(),
        StoredResult::Scalar(value) if offset == 0 && limit > 0 => {
            vec![RuntimeValue::Decimal(*value)]
        }
        StoredResult::Text(value) if offset == 0 && limit > 0 => {
            vec![RuntimeValue::String(value.clone())]
        }
        StoredResult::Empty | StoredResult::Scalar(_) | StoredResult::Text(_) => Vec::new(),
    };
    values
        .into_iter()
        .map(|value| runtime_value_to_json(&value))
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn runtime_value_to_json(value: &RuntimeValue) -> Result<serde_json::Value, CommandError> {
    Ok(match value {
        RuntimeValue::Null => serde_json::Value::Null,
        RuntimeValue::Bool(value) => (*value).into(),
        RuntimeValue::Integer(value) => (*value).into(),
        RuntimeValue::Unsigned(value) => (*value).into(),
        RuntimeValue::Decimal(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| CommandError::expected("result_value_not_json"))?,
        RuntimeValue::String(value) | RuntimeValue::Resource(value) => value.as_ref().into(),
        RuntimeValue::List(values) => values
            .iter()
            .map(runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        RuntimeValue::Record(values) => values
            .iter()
            .map(|(key, value)| Ok((key.to_string(), runtime_value_to_json(value)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, CommandError>>()?
            .into_iter()
            .collect::<serde_json::Map<_, _>>()
            .into(),
    })
}
