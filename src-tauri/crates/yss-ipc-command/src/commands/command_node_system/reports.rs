use crate::commands::execution_dto::ResultPageDto;
use crate::error::CommandError;
use crate::schema::result::{
    ResultAnalysisRequestDto, ResultAnalysisResponseDto, ResultReferenceDto, ResultTablePartDto,
};
use tauri::State;
use yss_application::execution::ApplicationState;
use yss_application::execution::result_query::report::ReportQueryError;

pub(super) fn report_query_error(error: ReportQueryError) -> CommandError {
    match error {
        ReportQueryError::Session(error) => super::results::session_capture_command_error(error),
        ReportQueryError::Stale => CommandError::expected("stale_result_reference"),
        ReportQueryError::Unavailable => CommandError::expected("result_not_found"),
        ReportQueryError::WrongKind => CommandError::expected("unsupported_result_analysis"),
        ReportQueryError::InvalidRequest => CommandError::expected("invalid_result_query"),
        ReportQueryError::Hypothesis(
            yss_application::hypothesis::HypothesisApplicationError::InvalidInput(_),
        ) => CommandError::expected("invalid_hypothesis"),
        ReportQueryError::Serial(error) => CommandError::expected(error.command_code()),
        error => CommandError::diagnosed("result_analysis_failed", format!("{error:?}")),
    }
}

#[tauri::command]
pub async fn get_result_table_page(
    state: State<'_, ApplicationState>,
    reference: ResultReferenceDto,
    part: ResultTablePartDto,
    offset: usize,
    limit: usize,
) -> Result<ResultPageDto, CommandError> {
    let reference = reference.try_into()?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .query_result_table(reference, part.into(), offset, limit)
            .map_err(report_query_error)
            .and_then(|page| {
                ResultPageDto::from_application(reference.result_id, page)
                    .map_err(|_| CommandError::expected("result_value_not_json"))
            })
    })
    .await
    .map_err(|error| CommandError::diagnosed("result_source_read_failed", format!("{error:?}")))?
}

#[tauri::command]
pub async fn analyze_result(
    state: State<'_, ApplicationState>,
    reference: ResultReferenceDto,
    analysis: ResultAnalysisRequestDto,
) -> Result<ResultAnalysisResponseDto, CommandError> {
    let reference = reference.try_into()?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .analyze_result(reference, analysis.into())
            .map(Into::into)
            .map_err(report_query_error)
    })
    .await
    .map_err(|error| CommandError::diagnosed("result_analysis_failed", format!("{error:?}")))?
}
