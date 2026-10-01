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
) -> Result<serde_json::Value, SciError> {
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
                "labels":fit.parameter_names,"categories":fit.parameter_categories,"estimates": fit.coefficients, "inference": fit.inference,
                "omittedTerms":fit.omitted_terms,
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
    report.insert("estimationSample".into(), serde_json::json!(fit.estimation));
    report.insert("responseName".into(), serde_json::json!(fit.response_name));
    let mut report: serde_json::Value = report.into();
    if options.coefficient_table {
        report["coefficient_rows"] = serde_json::json!(crate::report_display::coefficient_rows(
            &fit.parameter_names,
            &fit.coefficients,
            &fit.inference
        )?);
        if let Some(rows) = report["coefficient_rows"].as_array_mut() {
            for (i, row) in rows.iter_mut().enumerate() {
                row["category"] =
                    serde_json::json!(fit.parameter_categories.get(i).cloned().flatten());
            }
        }
        let mut columns = crate::report_display::COEFFICIENT_COLUMNS.to_vec();
        columns.push(("category", "Category"));
        crate::report_display::section(
            &mut report,
            "coefficients",
            "Panel coefficients",
            "table",
            "/coefficient_rows",
            &columns,
        );
        crate::report_display::section(
            &mut report,
            "omitted",
            "Omitted terms",
            "table",
            "/coefficients/omittedTerms",
            &[
                ("variable", "Variable"),
                ("category", "Category"),
                ("reason", "Omission reason"),
            ],
        );
    }
    Ok(report)
}

pub use yss_sci::panel::fit::fit_panel as fit_model;
pub use yss_sci::panel::fit::predict;
pub use yss_sci::panel::{difference_gmm, fisher_cointegration, fisher_unit_root};
