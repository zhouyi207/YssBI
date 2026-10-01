//! Bounded presentation metadata. Data arrays remain in their ordinary result paths
//! and are served by the application through structured-result table paging.
use serde_json::{Value, json};
pub fn section(
    report: &mut Value,
    id: &str,
    title: &str,
    kind: &str,
    path: &str,
    columns: &[(&str, &str)],
) {
    if report.get("report_display").is_none() {
        report["report_display"] = json!({"sections":{}});
    }
    if kind == "equation"
        && report
            .pointer(path)
            .and_then(Value::as_str)
            .is_none_or(|v| v.len() > 16_384)
    {
        // Derived text is inline in structured reports. Merely omitting its UI
        // descriptor would still let a large equation exhaust the JSON budget.
        if let Some(value) = report.pointer_mut(path) {
            *value = Value::Null;
        }
        report["presentation_unavailable"] = json!(
            "equation_text_unavailable_or_exceeds_display_limit; coefficient rows remain available"
        );
        return;
    }
    if report["report_display"]["sections"]
        .as_object()
        .is_some_and(|sections| sections.len() >= 24)
    {
        report["presentation_unavailable"] = json!("section_limit_exceeded");
        return;
    }
    let sections = report["report_display"]["sections"]
        .as_object_mut()
        .expect("owned descriptor object");
    sections.insert(id.into(),json!({"title":title,"kind":kind,"path":path,"columns":columns.iter().map(|(k,v)|(k.to_string(),json!(v))).collect::<serde_json::Map<_,_>>()}));
}
pub fn equation(response: &str, labels: &[String], coefficients: &[f64]) -> String {
    let mut expression = String::new();
    for (i, (name, &coefficient)) in labels.iter().zip(coefficients).enumerate() {
        let magnitude = coefficient.abs();
        let number = if magnitude != 0.0 && !(1e-4..1e6).contains(&magnitude) {
            format!("{magnitude:.6e}")
        } else {
            format!("{magnitude:.6}")
        };
        if i == 0 {
            if coefficient < 0.0 {
                expression.push('-');
            }
        } else {
            expression.push_str(if coefficient < 0.0 { " − " } else { " + " });
        }
        expression.push_str(&number);
        // Labels alone do not prove an intercept; a real predictor may be named const.
        expression.push_str(" × ");
        expression.push_str(name);
    }
    format!("{response} = {expression}")
}
pub fn coefficient_rows(
    names: &[String],
    estimates: &[f64],
    inference: &yss_sci_contract::regression::fit::RegressionCoefficientStatistics,
) -> Result<Vec<Value>, yss_sci_contract::SciError> {
    if names.len() != estimates.len() || !inference.has_shape(estimates.len()) {
        return Err(yss_sci_contract::SciError::InvalidInput {
            operation: yss_sci_contract::SciOperationCode::Regression,
            violation: yss_sci_contract::SciInputViolation::ShapeMismatch,
        });
    }
    Ok(names.iter().enumerate().map(|(j,name)|json!({"variable":name,"estimate":estimates[j],"standard_error":inference.standard_errors[j],"statistic":inference.statistic_values[j],"p_value":inference.p_values[j],"ci_lower":inference.confidence_interval_lower[j],"ci_upper":inference.confidence_interval_upper[j]})).collect())
}
pub const COEFFICIENT_COLUMNS: &[(&str, &str)] = &[
    ("variable", "Variable"),
    ("estimate", "Estimate"),
    ("standard_error", "Std. error"),
    ("statistic", "Statistic"),
    ("p_value", "p-value"),
    ("ci_lower", "95% CI lower"),
    ("ci_upper", "95% CI upper"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_derived_equations_do_not_consume_inline_report_budget() {
        let mut report = json!({"derived":{"equation":"x".repeat(600_000)},"coefficient_rows":[{"variable":"x","estimate":1.25}]});
        section(
            &mut report,
            "equation",
            "Equation",
            "equation",
            "/derived/equation",
            &[],
        );
        assert_eq!(report["derived"]["equation"], Value::Null);
        assert!(
            report["report_display"]["sections"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        assert!(
            report["presentation_unavailable"]
                .as_str()
                .unwrap()
                .contains("equation")
        );
        assert_eq!(report["coefficient_rows"][0]["estimate"], 1.25);
        assert!(serde_json::to_vec(&report).unwrap().len() < 1024);
    }
    #[test]
    fn equation_labels_are_not_intercept_metadata_and_small_coefficients_are_visible() {
        let text = equation(
            "response",
            &["const".into(), "_cons".into()],
            &[1e-12, -2.0],
        );
        assert!(text.contains("1.000000e-12 × const"));
        assert!(text.contains(" − 2.000000 × _cons"));
    }
}
