use serde::Deserialize;
use tauri::State;

use crate::application::bayes::BayesInferenceService;
use crate::error::AppError;
use crate::project::ProjectState;
use crate::sci::api::bayes::{
    AutocorrelationPlotData, BayesInferenceTask, BayesModelDraft, ColumnMeta, DensityPlotData,
    InferenceResult, ParsedExpression, PosteriorPredictivePage, PosteriorSamplePage, TracePlotData,
    parse_model_expression, validate_draft,
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

#[tauri::command]
pub fn parse_bayes_expression(input: ParseExpressionRequest) -> Result<ParsedExpression, AppError> {
    let mut known_symbols = input.symbols;
    known_symbols.extend(input.columns.into_iter().map(|column| column.name));
    known_symbols.sort();
    known_symbols.dedup();
    let options = if input.formula.contains('\\') {
        crate::math::ParseOptions::latex(&known_symbols)
    } else {
        crate::math::ParseOptions::plain(&known_symbols)
    };
    parse_model_expression(&input.formula, options)
        .map_err(|error| AppError::new("bayes_expression_parse_failed", error.to_string()))
}

#[tauri::command]
pub fn validate_bayes_model(
    input: BayesModelDraft,
) -> Result<crate::sci::api::bayes::ValidationReport, AppError> {
    Ok(validate_draft(&input))
}

#[tauri::command]
pub fn submit_bayes_inference(
    service: State<'_, BayesInferenceService>,
    project_state: State<'_, ProjectState>,
    input: BayesModelDraft,
) -> Result<BayesInferenceTask, AppError> {
    service.submit_from_project(input, &project_state)
}

#[tauri::command]
pub fn get_bayes_inference_status(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<BayesInferenceTask, AppError> {
    service.status(&task_id)
}

#[tauri::command]
pub fn cancel_bayes_inference(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<(), AppError> {
    service.cancel(&task_id)
}

#[tauri::command]
pub fn read_bayes_inference_result(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<InferenceResult, AppError> {
    service.result(&task_id)
}

#[tauri::command]
pub fn clear_bayes_inference_task(
    service: State<'_, BayesInferenceService>,
    task_id: String,
) -> Result<(), AppError> {
    service.clear_task(&task_id)
}

#[tauri::command]
pub fn read_bayes_posterior_samples(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    offset: usize,
    limit: usize,
    parameter: Option<String>,
) -> Result<PosteriorSamplePage, AppError> {
    service.sample_page(&task_id, offset, limit, parameter.as_deref())
}

#[tauri::command]
pub fn read_bayes_trace_plot_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    max_points_per_chain: Option<usize>,
) -> Result<TracePlotData, AppError> {
    service.trace_plot_data(
        &task_id,
        parameter.as_deref(),
        max_points_per_chain.unwrap_or(500),
    )
}

#[tauri::command]
pub fn read_bayes_density_plot_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    grid_points: Option<usize>,
) -> Result<DensityPlotData, AppError> {
    service.density_plot_data(&task_id, parameter.as_deref(), grid_points.unwrap_or(256))
}

#[tauri::command]
pub fn read_bayes_autocorrelation_data(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    parameter: Option<String>,
    max_lag: Option<usize>,
) -> Result<AutocorrelationPlotData, AppError> {
    service.autocorrelation_plot_data(&task_id, parameter.as_deref(), max_lag.unwrap_or(50))
}

#[tauri::command]
pub fn read_bayes_posterior_predictive(
    service: State<'_, BayesInferenceService>,
    task_id: String,
    offset: usize,
    limit: usize,
) -> Result<PosteriorPredictivePage, AppError> {
    service.posterior_predictive_page(&task_id, offset, limit)
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
            crate::sci::api::bayes::RawExpression::Binary {
                op: crate::sci::api::bayes::BinaryOp::Mul,
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
}
