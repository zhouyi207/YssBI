use crate::error::computation_failed;
use yss_sci_contract::{SciError, SciOperationCode};

pub use yss_sci::causal::iv::fit::fit_instrumental_variables;

pub fn summary(
    fit: &yss_sci_contract::causal::iv::InstrumentalVariableFit,
    options: yss_sci_contract::causal::iv::IvSummaryOptions,
) -> Result<serde_json::Value, SciError> {
    use yss_sci::causal::iv::fit as analysis;
    if fit.parameter_names.len() != fit.coefficients.len()
        || fit.instrument_names.len() != fit.design.instruments.len()
        || !fit.inference.has_shape(fit.coefficients.len())
    {
        return Err(yss_sci_contract::SciError::InvalidInput {
            operation: SciOperationCode::InstrumentalVariables,
            violation: yss_sci_contract::execution::ScientificInputViolation::ShapeMismatch,
        });
    }
    let mut report = serde_json::Map::new();
    report.insert("responseName".into(), serde_json::json!(fit.response_name));
    report.insert(
        "statisticDistribution".into(),
        serde_json::json!(if fit.small { "t" } else { "z" }),
    );
    if options.model_summary {
        report.insert(
            "model".into(),
            serde_json::json!({"family": fit.family, "statistics": fit.statistics}),
        );
    }
    if options.coefficient_table {
        report.insert(
            "coefficients".into(),
            serde_json::json!({"labels":fit.parameter_names,"estimates": fit.coefficients, "inference": fit.inference}),
        );
    }
    let first_stage = if options.first_stage {
        let (mut equations, statistics) = analysis::first_stage(fit)?;
        let included = fit.design.exogenous.len() + usize::from(fit.options.constant);
        let labels = fit.parameter_names[..included]
            .iter()
            .chain(&fit.instrument_names)
            .cloned()
            .collect::<Vec<_>>();
        for (j, equation) in equations.iter_mut().enumerate() {
            equation.endog_name = fit.parameter_names[included + j].clone();
            equation.var_names = labels.clone();
        }
        Some((equations, statistics))
    } else {
        None
    };
    if options.overidentification {
        let result = if fit.family == "iv_liml" {
            serde_json::json!(analysis::liml_overidentification(fit)?)
        } else {
            serde_json::json!(analysis::overidentification(fit)?)
        };
        if result.is_null() {
            report.insert(
                "overidentificationUnavailable".into(),
                serde_json::json!(
                    if fit.design.instruments.len() == fit.design.endogenous.len() {
                        "exact_identification"
                    } else if fit.family == "iv_liml" && fit.options.covariance.is_robust() {
                        "requires_nonrobust_covariance"
                    } else {
                        "insufficient_residual_variation_or_degrees_of_freedom"
                    }
                ),
            );
        }
        report.insert("overidentification".into(), result);
    }
    if options.endogeneity {
        let (hausman, endogenous) = analysis::endogeneity(fit)?;
        if hausman.is_none() && endogenous.is_none() {
            report.insert(
                "endogeneityUnavailable".into(),
                serde_json::json!(if fit.options.covariance.is_robust() {
                    "requires_nonrobust_covariance"
                } else {
                    "insufficient_residual_variation_or_degrees_of_freedom"
                }),
            );
        }
        report.insert(
            "endogeneity".into(),
            serde_json::json!({"hausman": hausman, "endogenous": endogenous}),
        );
    }
    let mut report: serde_json::Value = report.into();
    use crate::report_display::{COEFFICIENT_COLUMNS, coefficient_rows, equation, section};
    if options.coefficient_table {
        report["coefficient_rows"] = serde_json::json!(coefficient_rows(
            &fit.parameter_names,
            &fit.coefficients,
            &fit.inference
        )?);
        report["structural_equation"] = serde_json::json!(equation(
            &fit.response_name,
            &fit.parameter_names,
            &fit.coefficients
        ));
        section(
            &mut report,
            "coefficients",
            "Structural coefficients",
            "table",
            "/coefficient_rows",
            COEFFICIENT_COLUMNS,
        );
        section(
            &mut report,
            "equation",
            "Structural equation",
            "equation",
            "/structural_equation",
            &[],
        );
    }
    if let Some((equations, statistics)) = first_stage {
        let mut rows = Vec::new();
        let mut texts = Vec::new();
        for e in &equations {
            texts.push(equation(&e.endog_name, &e.var_names, &e.betas));
            for mut row in coefficient_rows(&e.var_names, &e.betas, &e.inference)? {
                row["equation"] = serde_json::json!(e.endog_name);
                rows.push(row);
            }
        }
        report["first_stage_rows"] = serde_json::json!(rows);
        report["first_stage_equations"] = serde_json::json!(texts.join("\n"));
        section(
            &mut report,
            "first_stage",
            "First-stage equations",
            "table",
            "/first_stage_rows",
            &[
                ("equation", "Response"),
                ("variable", "Variable"),
                ("estimate", "Coefficient"),
                ("standard_error", "Std. error"),
                ("statistic", "t"),
                ("p_value", "p-value"),
            ],
        );
        section(
            &mut report,
            "first_stage_equations",
            "First-stage model equations",
            "equation",
            "/first_stage_equations",
            &[],
        );
        let size = statistics.min_eigenvalue_cv.as_ref().map(|cv| &cv.size);
        let bias = statistics
            .min_eigenvalue_cv
            .as_ref()
            .and_then(|cv| cv.bias.as_ref());
        report["weak_instrument_rows"] = serde_json::json!([{
            "min_eigenvalue": statistics.min_eigenvalue,
            "partial_r2": statistics.partial_r2,
            "f_stat": statistics.f_stat,
            "f_p_value": statistics.f_p_value,
            "critical_values_unavailable_reason": statistics.min_eigenvalue_cv_note,
            "size_10": size.map(|row| row.pct_10),
            "size_15": size.map(|row| row.pct_15),
            "size_20": size.map(|row| row.pct_20),
            "size_25": size.map(|row| row.pct_25),
            "bias_5": bias.map(|row| row.pct_5),
            "bias_10": bias.map(|row| row.pct_10),
            "bias_20": bias.map(|row| row.pct_20),
            "bias_30": bias.map(|row| row.pct_30),
            "bias_unavailable_reason": bias.is_none().then_some("not_tabulated_for_this_identification_or_covariance"),
        }]);
        report["firstStage"] =
            serde_json::json!({"equations": equations, "statistics": statistics});
        section(
            &mut report,
            "weak_instruments",
            "Weak-instrument diagnostics",
            "table",
            "/weak_instrument_rows",
            &[
                ("min_eigenvalue", "Minimum eigenvalue"),
                ("partial_r2", "Partial R²"),
                ("f_stat", "First-stage F"),
                ("f_p_value", "F p-value"),
                ("size_10", "10% size critical value"),
                ("size_15", "15% size critical value"),
                ("size_20", "20% size critical value"),
                ("size_25", "25% size critical value"),
                ("critical_values_unavailable_reason", "Unavailable reason"),
                ("bias_5", "5% relative-bias critical value"),
                ("bias_10", "10% relative-bias critical value"),
                ("bias_20", "20% relative-bias critical value"),
                ("bias_30", "30% relative-bias critical value"),
                ("bias_unavailable_reason", "Bias-table unavailable reason"),
            ],
        );
    }
    Ok(report)
}

pub fn hausman(
    fit: &yss_sci_contract::causal::iv::InstrumentalVariableFit,
) -> Result<yss_sci_contract::causal::iv::HausmanTest, SciError> {
    yss_sci::causal::iv::fit::endogeneity(fit)?
        .0
        .ok_or_else(|| computation_failed(SciOperationCode::InstrumentalVariables))
}
