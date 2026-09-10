use serde_json::Value;
use yss_bayes_runtime::{BayesApplicationError, BayesInferenceService};
use yss_plugin_protocol::PluginFailure;

pub fn bayes_error(error: BayesApplicationError) -> PluginFailure {
    let code = match error {
        BayesApplicationError::ValidationFailed => "bayes_validation_failed",
        BayesApplicationError::DatasetSourceUnsupported => "bayes_dataset_source_unsupported",
        BayesApplicationError::TaskNotFound => "bayes_task_not_found",
        BayesApplicationError::TaskActive => "bayes_task_active",
        BayesApplicationError::ResultNotFound => "bayes_result_not_found",
        BayesApplicationError::ArtifactExportUnsupported => "bayes_artifact_export_unsupported",
        BayesApplicationError::ArtifactNotFound => "bayes_artifact_not_found",
        BayesApplicationError::SamplesNotFound => "bayes_samples_not_found",
        BayesApplicationError::PosteriorPredictiveNotFound => {
            "bayes_posterior_predictive_not_found"
        }
        BayesApplicationError::PagingInvalid { .. } => "bayes_paging_invalid",
        BayesApplicationError::CancelFailed { .. } => "bayes_cancel_failed",
        _ => "bayes_backend_failed",
    };
    PluginFailure::new(code)
}

pub fn execute(
    service: &BayesInferenceService,
    value: Value,
    peer: &yss_plugin_sdk::Peer,
    context: &Value,
    data_dir: &std::path::Path,
) -> Result<Value, PluginFailure> {
    let method = value["commandId"]
        .as_str()
        .ok_or_else(|| PluginFailure::new("plugin_command_invalid"))?;
    let args = &value["args"];
    let task_id = args["taskId"].as_str().unwrap_or_default();
    let parameter = args["parameter"].as_str();
    let encode = |value| {
        serde_json::to_value(value).map_err(|_| PluginFailure::new("plugin_response_invalid"))
    };
    match method {
        "export_bayes_artifact_csv" => {
            let kind = serde_json::from_value(args["kind"].clone())
                .map_err(|_| PluginFailure::new("bayes_artifact_export_unsupported"))?;
            std::fs::create_dir_all(data_dir.join("exports"))
                .map_err(|_| PluginFailure::new("plugin_storage_failed"))?;
            let relative = format!("exports/{}.csv", uuid::Uuid::new_v4());
            let path = data_dir.join(&relative);
            service
                .export_artifact_csv(task_id, kind, &path.to_string_lossy())
                .map_err(bayes_error)?;
            let exported=peer.call("files.export",serde_json::json!({"context":context,"input":{"grant":args["destination"],"source":relative}}),std::time::Duration::from_secs(30));
            let _ = std::fs::remove_file(path);
            exported
        }
        "parse_bayes_expression" => {
            let input = &args["input"];
            let formula = input["formula"]
                .as_str()
                .ok_or_else(|| PluginFailure::new("bayes_expression_parse_failed"))?;
            let mut symbols = input["symbols"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            symbols.extend(
                input["columns"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|column| column["name"].as_str())
                    .map(str::to_owned),
            );
            symbols.sort();
            symbols.dedup();
            let options = if formula.contains('\\') {
                yss_math_expr::ParseOptions::latex(&symbols)
            } else {
                yss_math_expr::ParseOptions::plain(&symbols)
            };
            serde_json::to_value(
                yss_bayes_model::parse_model_expression(formula, options)
                    .map_err(|_| PluginFailure::new("bayes_expression_parse_failed"))?,
            )
            .map_err(|_| PluginFailure::new("plugin_response_invalid"))
        }
        "validate_bayes_model" => {
            let draft = serde_json::from_value(args["input"].clone())
                .map_err(|_| PluginFailure::new("bayes_validation_failed"))?;
            serde_json::to_value(yss_bayes_model::validate_draft(&draft))
                .map_err(|_| PluginFailure::new("plugin_response_invalid"))
        }
        "read_bayes_inference_result" => encode(service.result(task_id).map_err(bayes_error)?),
        "read_bayes_trace_plot_data" => serde_json::to_value(
            service
                .trace_plot_data(
                    task_id,
                    parameter,
                    args["maxPointsPerChain"].as_u64().unwrap_or(500) as usize,
                )
                .map_err(bayes_error)?,
        )
        .map_err(|_| PluginFailure::new("plugin_response_invalid")),
        "read_bayes_density_plot_data" => serde_json::to_value(
            service
                .density_plot_data(
                    task_id,
                    parameter,
                    args["gridPoints"].as_u64().unwrap_or(256) as usize,
                )
                .map_err(bayes_error)?,
        )
        .map_err(|_| PluginFailure::new("plugin_response_invalid")),
        "read_bayes_autocorrelation_data" => serde_json::to_value(
            service
                .autocorrelation_plot_data(
                    task_id,
                    parameter,
                    args["maxLag"].as_u64().unwrap_or(50) as usize,
                )
                .map_err(bayes_error)?,
        )
        .map_err(|_| PluginFailure::new("plugin_response_invalid")),
        "read_bayes_posterior_predictive" => serde_json::to_value(
            service
                .posterior_predictive_page(
                    task_id,
                    args["offset"].as_u64().unwrap_or(0) as usize,
                    args["limit"].as_u64().unwrap_or(100) as usize,
                )
                .map_err(bayes_error)?,
        )
        .map_err(|_| PluginFailure::new("plugin_response_invalid")),
        _ => Err(PluginFailure::new("plugin_method_unknown")),
    }
}
