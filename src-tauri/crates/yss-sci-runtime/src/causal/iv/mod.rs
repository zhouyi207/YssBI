use crate::error::computation_failed;
use yss_sci_contract::causal::iv::InstrumentalVariableKind;
use yss_sci_contract::{SciError, SciOperationCode};

pub fn fit_instrumental_variables(
    kind: InstrumentalVariableKind,
    response: Vec<f64>,
    exogenous: &[Vec<f64>],
    endogenous: &[Vec<f64>],
    instruments: &[Vec<f64>],
    options: yss_sci_contract::regression::OlsOptions,
    small: bool,
) -> Result<serde_json::Value, SciError> {
    let fit = yss_sci::causal::iv::fit::fit_instrumental_variables(
        kind,
        response,
        exogenous,
        endogenous,
        instruments,
        options,
        small,
    )?;
    serde_json::to_value(fit)
        .map_err(|_| computation_failed(SciOperationCode::InstrumentalVariables))
}

pub fn summary(
    fit: &yss_sci_contract::causal::iv::InstrumentalVariableFit,
    options: yss_sci_contract::causal::iv::IvSummaryOptions,
) -> Result<serde_json::Value, SciError> {
    use yss_sci::causal::iv::fit as analysis;
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
            serde_json::json!({"estimates": fit.coefficients, "inference": fit.inference}),
        );
    }
    if options.first_stage {
        let (equations, statistics) = analysis::first_stage(fit)?;
        report.insert(
            "firstStage".into(),
            serde_json::json!({"equations": equations, "statistics": statistics}),
        );
    }
    if options.overidentification {
        let result = if fit.family == "iv_liml" {
            serde_json::json!(analysis::liml_overidentification(fit)?)
        } else {
            serde_json::json!(analysis::overidentification(fit)?)
        };
        report.insert("overidentification".into(), result);
    }
    if options.endogeneity {
        let (hausman, endogenous) = analysis::endogeneity(fit)?;
        report.insert(
            "endogeneity".into(),
            serde_json::json!({"hausman": hausman, "endogenous": endogenous}),
        );
    }
    Ok(report.into())
}

pub fn hausman(
    fit: &yss_sci_contract::causal::iv::InstrumentalVariableFit,
) -> Result<yss_sci_contract::causal::iv::HausmanTest, SciError> {
    yss_sci::causal::iv::fit::endogeneity(fit)?
        .0
        .ok_or_else(|| computation_failed(SciOperationCode::InstrumentalVariables))
}
