use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::CommandError;
use yss_application::bayes::{BayesApplicationError, BayesInferenceService};
use yss_application::execution::ApplicationState;
use yss_bayes_model::{
    BayesModelDraft, ColumnMeta, ParsedExpression, parse_model_expression, validate_draft,
};
use yss_bayes_result::{
    AutocorrelationPlotData, BayesInferenceTask, DensityPlotData, InferenceResult,
    PosteriorPredictivePage, PosteriorSamplePage, ResultArtifactKind, TracePlotData,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseExpressionRequest {
    pub formula: String,
    #[serde(default)]
    pub columns: Vec<ColumnMeta>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BayesPagingDetails {
    offset: usize,
    limit: usize,
}

fn bayes_command_error(error: BayesApplicationError) -> CommandError {
    match error {
        BayesApplicationError::ValidationFailed => {
            CommandError::expected("bayes_validation_failed")
        }
        BayesApplicationError::DatasetSourceUnsupported => {
            CommandError::expected("bayes_dataset_source_unsupported")
        }
        BayesApplicationError::TaskNotFound => CommandError::expected("bayes_task_not_found"),
        BayesApplicationError::TaskActive => CommandError::expected("bayes_task_active"),
        BayesApplicationError::ResultNotFound => CommandError::expected("bayes_result_not_found"),
        BayesApplicationError::ArtifactExportUnsupported => {
            CommandError::expected("bayes_artifact_export_unsupported")
        }
        BayesApplicationError::ArtifactNotFound => {
            CommandError::expected("bayes_artifact_not_found")
        }
        BayesApplicationError::SamplesNotFound => CommandError::expected("bayes_samples_not_found"),
        BayesApplicationError::PosteriorPredictiveNotFound => {
            CommandError::expected("bayes_posterior_predictive_not_found")
        }
        BayesApplicationError::PagingInvalid { offset, limit } => {
            CommandError::expected("bayes_paging_invalid")
                .with_details(BayesPagingDetails { offset, limit })
        }
        error @ BayesApplicationError::CancelFailed { .. } => {
            CommandError::diagnosed("bayes_cancel_failed", error)
        }
        error @ BayesApplicationError::ServiceLockPoisoned => {
            CommandError::diagnosed("bayes_service_lock_poisoned", error)
        }
        error @ BayesApplicationError::DatasetLoadFailed { .. } => {
            CommandError::diagnosed("bayes_dataset_load_failed", error)
        }
        error @ BayesApplicationError::ArtifactReadFailed { .. } => {
            CommandError::diagnosed("bayes_result_artifact_read_failed", error)
        }
        error @ BayesApplicationError::ArtifactWriteFailed { .. } => {
            CommandError::diagnosed("bayes_artifact_export_failed", error)
        }
        error @ BayesApplicationError::SamplesInvalid { .. } => {
            CommandError::diagnosed("bayes_samples_invalid", error)
        }
        error @ BayesApplicationError::PosteriorPredictiveInvalid { .. } => {
            CommandError::diagnosed("bayes_posterior_predictive_invalid", error)
        }
        error @ BayesApplicationError::BackendStateInvalid { .. } => {
            CommandError::diagnosed("bayes_result_not_found", error)
        }
    }
}

#[tauri::command]
pub fn parse_bayes_expression(
    input: ParseExpressionRequest,
) -> Result<ParsedExpression, CommandError> {
    let mut known_symbols = input.symbols;
    known_symbols.extend(input.columns.into_iter().map(|column| column.name));
    known_symbols.sort();
    known_symbols.dedup();
    let options = if input.formula.contains('\\') {
        yss_math::ParseOptions::latex(&known_symbols)
    } else {
        yss_math::ParseOptions::plain(&known_symbols)
    };
    parse_model_expression(&input.formula, options)
        .map_err(|_| CommandError::expected("bayes_expression_parse_failed"))
}

#[tauri::command]
pub fn validate_bayes_model(
    input: BayesModelDraft,
) -> Result<yss_bayes_model::ValidationReport, CommandError> {
    Ok(validate_draft(&input))
}

#[tauri::command]
pub fn submit_bayes_inference(
    service: State<'_, BayesInferenceService>,
    application: State<'_, ApplicationState>,
    input: BayesModelDraft,
) -> Result<BayesInferenceTask, CommandError> {
    service
        .submit_from_application(&application, input)
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn get_bayes_inference_status(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<BayesInferenceTask, CommandError> {
    service.status(&task_id).map_err(bayes_command_error)
}

#[tauri::command]
pub fn cancel_bayes_inference(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<(), CommandError> {
    service.cancel(&task_id).map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_inference_result(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<InferenceResult, CommandError> {
    service.result(&task_id).map_err(bayes_command_error)
}

#[tauri::command]
pub fn clear_bayes_inference_task(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<(), CommandError> {
    service.clear_task(&task_id).map_err(bayes_command_error)
}

#[tauri::command]
pub fn export_bayes_artifact_csv(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    kind: ResultArtifactKind,
    destination: String,
) -> Result<(), CommandError> {
    service
        .export_artifact_csv(&task_id, kind, &destination)
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_posterior_samples(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    offset: usize,
    limit: usize,
    parameter: Option<String>,
) -> Result<PosteriorSamplePage, CommandError> {
    service
        .sample_page(&task_id, offset, limit, parameter.as_deref())
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_trace_plot_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    max_points_per_chain: Option<usize>,
) -> Result<TracePlotData, CommandError> {
    service
        .trace_plot_data(
            &task_id,
            parameter.as_deref(),
            max_points_per_chain.unwrap_or(500),
        )
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_density_plot_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    grid_points: Option<usize>,
) -> Result<DensityPlotData, CommandError> {
    service
        .density_plot_data(&task_id, parameter.as_deref(), grid_points.unwrap_or(256))
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_autocorrelation_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    max_lag: Option<usize>,
) -> Result<AutocorrelationPlotData, CommandError> {
    service
        .autocorrelation_plot_data(&task_id, parameter.as_deref(), max_lag.unwrap_or(50))
        .map_err(bayes_command_error)
}

#[tauri::command]
pub fn read_bayes_posterior_predictive(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    offset: usize,
    limit: usize,
) -> Result<PosteriorPredictivePage, CommandError> {
    service
        .posterior_predictive_page(&task_id, offset, limit)
        .map_err(bayes_command_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_accepts_optional_context() {
        let request: ParseExpressionRequest =
            serde_json::from_value(serde_json::json!({ "formula": "y = ax" })).unwrap();
        assert!(request.columns.is_empty());
        assert!(request.symbols.is_empty());
    }

    #[test]
    fn parse_request_combines_column_and_symbol_context() {
        let request: ParseExpressionRequest = serde_json::from_value(serde_json::json!({
            "formula": "y \\sim \\operatorname{Normal}(ax, \\sigma)",
            "columns": [{ "name": "x", "dtype": "number", "nullable": false }],
            "symbols": ["y", "a", "sigma"]
        }))
        .unwrap();
        let parsed = parse_bayes_expression(request).unwrap();
        assert_eq!(parsed.symbols, ["a", "sigma", "x", "y"]);
        assert!(matches!(
            parsed.formula.raw_predictor,
            yss_bayes_model::RawExpression::Binary {
                op: yss_bayes_model::BinaryOp::Mul,
                ..
            }
        ));
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            serde_json::json!({
                "formula": {
                    "formulaText": "y \\sim \\operatorname{Normal}(ax, \\sigma)",
                    "rawResponse": { "type": "symbol", "name": "y" },
                    "rawPredictor": {
                        "type": "binary",
                        "op": "mul",
                        "left": { "type": "symbol", "name": "a" },
                        "right": { "type": "symbol", "name": "x" }
                    }
                },
                "symbols": ["a", "sigma", "x", "y"]
            })
        );
    }

    #[test]
    fn expected_application_error_maps_to_exact_command_wire() {
        let application_error = BayesApplicationError::PagingInvalid {
            offset: 4,
            limit: 0,
        };

        let wire = serde_json::to_value(bayes_command_error(application_error)).unwrap();

        assert_eq!(
            wire,
            serde_json::json!({
                "code": "bayes_paging_invalid",
                "details": { "offset": 4, "limit": 0 },
                "incidentId": null,
            })
        );
    }

    #[test]
    fn internal_application_error_maps_to_incident_without_source_prose() {
        let error = bayes_command_error(BayesApplicationError::ArtifactReadFailed {
            context: "posterior samples",
            source: "private artifact path and backend prose".to_string(),
        });

        let wire = serde_json::to_value(error).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 3);
        assert_eq!(wire["code"], "bayes_result_artifact_read_failed");
        assert!(wire["details"].is_null());
        assert!(uuid::Uuid::parse_str(wire["incidentId"].as_str().unwrap()).is_ok());
        assert!(wire.get("message").is_none());
        assert!(wire.get("detail").is_none());
        assert!(wire.get("hint").is_none());
        assert!(!wire.to_string().contains("private artifact path"));
        assert!(!wire.to_string().contains("backend prose"));
    }
}
