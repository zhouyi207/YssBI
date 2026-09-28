//! Panel estimation and report records.
use crate::error::computation_failed;
use yss_sci_contract::{SciError, SciOperationCode};

pub fn fit_panel(
    response: Vec<f64>,
    predictors: Vec<Vec<f64>>,
    entity: Vec<f64>,
    time: Vec<f64>,
    options: yss_sci_contract::panel::PanelOptions,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::panel::fit::fit_panel(response, predictors, entity, time, options)?;
    serde_json::to_value(fit).map_err(|_| computation_failed(SciOperationCode::Panel))
}

pub fn summary(
    fit: &yss_sci_contract::panel::PanelFit,
    options: yss_sci_contract::panel::PanelSummaryOptions,
) -> serde_json::Value {
    let mut report = serde_json::Map::new();
    if options.model_summary {
        report.insert(
            "model".into(),
            serde_json::json!({"family": fit.family, "statistics": fit.statistics}),
        );
    }
    if options.coefficient_table {
        report.insert(
            "coefficients".into(),
            serde_json::json!({
                "estimates": fit.coefficients, "inference": fit.inference,
                "recoveredConstant": fit.recovered_constant,
                "recoveredConstantStandardError": fit.recovered_constant_standard_error,
                "omittedIndices": fit.omitted_indices,
            }),
        );
    }
    if options.effects_statistics {
        report.insert("effects".into(), serde_json::json!(fit.effects_statistics));
    }
    if options.estimator_statistics {
        report.insert(
            "estimator".into(),
            serde_json::json!(fit.estimator_statistics),
        );
    }
    report.into()
}
