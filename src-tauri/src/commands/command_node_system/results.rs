use super::common::parse_opaque_u64;
use crate::commands::node_system_execution_dto::{
    PinResultEntryDto, ResultDescriptorDto, ResultPageDto, ResultValueDto,
};
use crate::error::CommandError;
use crate::node_system::document::PortAddressDto;
use crate::node_system::runtime::{ResultId, ResultState, StoredValueKind};
use crate::project::ProjectState;
use serde::Serialize;
use tauri::State;

pub(super) fn get_result_descriptor_from_state(
    state: &ProjectState,
    result_id: &str,
) -> Result<Option<ResultDescriptorDto>, CommandError> {
    let result_id = parse_opaque_u64("resultId", result_id)?;
    state
        .result(ResultId::new(result_id))
        .map_err(CommandError::from)
        .map(|result| result.as_deref().map(Into::into))
}

#[tauri::command]
pub fn get_result_descriptor(
    state: State<'_, ProjectState>,
    result_id: String,
) -> Result<Option<ResultDescriptorDto>, CommandError> {
    get_result_descriptor_from_state(&state, &result_id)
}

pub(crate) fn result_value_to_json(
    value: &crate::node_system::protocol::Value,
) -> Result<serde_json::Value, CommandError> {
    use crate::node_system::protocol::Value;

    Ok(match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => (*value).into(),
        Value::Integer(value) => (*value).into(),
        Value::Unsigned(value) => (*value).into(),
        Value::Decimal(value) => value
            .as_str()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
            .ok_or_else(|| CommandError::expected("result_value_not_json"))?,
        Value::String(value) => value.as_ref().into(),
        Value::Bytes(values) => values
            .iter()
            .copied()
            .map(serde_json::Value::from)
            .collect(),
        Value::List(values) => values
            .iter()
            .map(result_value_to_json)
            .collect::<Result<_, _>>()?,
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.to_string(), result_value_to_json(value)?)))
            .collect::<Result<_, CommandError>>()?,
    })
}

fn result_values_to_json(
    values: &[crate::node_system::protocol::Value],
) -> Result<Box<[serde_json::Value]>, CommandError> {
    values
        .iter()
        .map(result_value_to_json)
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

pub(super) const MAX_INLINE_RESULT_JSON_BYTES: usize = 64 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultStateErrorDetails {
    result_id: String,
    state: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultPagingErrorDetails {
    result_id: String,
    value_kind: &'static str,
}

fn result_state_error(result_id: ResultId, state: &ResultState) -> CommandError {
    let state = match state {
        ResultState::Pending(_) => "pending",
        ResultState::Failed(_) => "failed",
        ResultState::Cancelled => "cancelled",
        ResultState::Ready(_) => unreachable!("ready results do not produce state errors"),
    };
    CommandError::expected("result_not_ready").with_details(ResultStateErrorDetails {
        result_id: result_id.get().to_string(),
        state,
    })
}

fn result_requires_paging(result_id: ResultId, kind: StoredValueKind) -> CommandError {
    let value_kind = match kind {
        StoredValueKind::Scalar => "scalar",
        StoredValueKind::Sequence => "sequence",
        StoredValueKind::DataSeries => "dataSeries",
    };
    CommandError::expected("result_requires_paging").with_details(ResultPagingErrorDetails {
        result_id: result_id.get().to_string(),
        value_kind,
    })
}

pub(super) fn get_result_value_from_state(
    state: &ProjectState,
    result_id: &str,
) -> Result<Option<ResultValueDto>, CommandError> {
    let result_id = parse_opaque_u64("resultId", result_id)?;
    let Some(result) = state
        .result(ResultId::new(result_id))
        .map_err(CommandError::from)?
    else {
        return Ok(None);
    };
    let ResultState::Ready(value) = &result.state else {
        return Err(result_state_error(ResultId::new(result_id), &result.state));
    };
    if value.kind() != StoredValueKind::Scalar {
        return Err(result_requires_paging(
            ResultId::new(result_id),
            value.kind(),
        ));
    }
    let values = value
        .page(0, 1)
        .map_err(|error| CommandError::diagnosed("result_read_failed", error))?;
    let value = values
        .first()
        .map(result_value_to_json)
        .transpose()?
        .unwrap_or(serde_json::Value::Null);
    let encoded_size = serde_json::to_vec(&value)
        .map_err(|error| CommandError::diagnosed("result_value_not_json", error))?
        .len();
    if encoded_size > MAX_INLINE_RESULT_JSON_BYTES {
        return Err(result_requires_paging(
            ResultId::new(result_id),
            StoredValueKind::Scalar,
        ));
    }
    Ok(Some(ResultValueDto::Value(value)))
}

#[tauri::command]
pub fn get_result_value(
    state: State<'_, ProjectState>,
    result_id: String,
) -> Result<Option<ResultValueDto>, CommandError> {
    get_result_value_from_state(&state, &result_id)
}

pub(super) fn get_result_page_from_state(
    state: &ProjectState,
    result_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultPageDto>, CommandError> {
    let result_id = ResultId::new(parse_opaque_u64("resultId", result_id)?);
    let Some(result) = state.result(result_id).map_err(CommandError::from)? else {
        return Ok(None);
    };
    let ResultState::Ready(value) = &result.state else {
        return Err(result_state_error(result_id, &result.state));
    };
    let values = value
        .page(offset, limit)
        .map_err(|error| CommandError::diagnosed("result_read_failed", error))?;
    Ok(Some(ResultPageDto::new(
        result_id,
        offset.min(value.len()),
        limit,
        value.kind(),
        value.data_series_metadata().cloned(),
        value.len(),
        result_values_to_json(&values)?,
    )))
}

#[tauri::command]
pub fn get_result_page(
    state: State<'_, ProjectState>,
    result_id: String,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultPageDto>, CommandError> {
    get_result_page_from_state(&state, &result_id, offset, limit)
}

pub(super) fn get_pin_result_history_from_state(
    state: &ProjectState,
    graph_path: &str,
    output: PortAddressDto,
) -> Result<Box<[PinResultEntryDto]>, CommandError> {
    let port = output
        .try_into()
        .map_err(|_| CommandError::expected("invalid_output"))?;
    let output = crate::node_system::plan::GraphOutputRef {
        graph_path: crate::graph_document::GraphResourcePath::new(graph_path)
            .map_err(|_| CommandError::expected("invalid_graph_resource_path"))?,
        port,
    };
    state
        .pin_result_history(&output)
        .map_err(CommandError::from)
        .map(|history| {
            history
                .into_vec()
                .into_iter()
                .map(|(entry, result)| PinResultEntryDto::from_entry(entry, &result))
                .collect::<Vec<_>>()
                .into_boxed_slice()
        })
}

#[tauri::command]
pub fn get_pin_result_history(
    state: State<'_, ProjectState>,
    graph_path: String,
    output: PortAddressDto,
) -> Result<Box<[PinResultEntryDto]>, CommandError> {
    get_pin_result_history_from_state(&state, &graph_path, output)
}
