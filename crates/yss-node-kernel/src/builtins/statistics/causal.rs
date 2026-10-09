use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{causal::iv::InstrumentalVariableKind as IvKind, regression::OlsOptions};
mod models;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    models::register(builder);
    for (name, kind) in [
        ("2sls", IvKind::TwoStageLeastSquares),
        ("liml", IvKind::LimitedInformationMaximumLikelihood),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.iv.{name}.fit"),
            vec![
                Input::fixed("y"),
                Input::repeated("x", 0..=usize::MAX),
                Input::repeated("endogenous", 1..=usize::MAX),
                Input::repeated("instruments", 1..=usize::MAX),
            ],
            &["constant", "covariance", "small"],
            3,
            move |inv| iv(kind, inv),
        );
        install(
            builder,
            &format!("yssbi.statistics.iv.{name}.summary"),
            vec![Input::fixed("model")],
            if name == "2sls" {
                &[
                    "model_summary",
                    "coefficient_table",
                    "first_stage",
                    "overidentification",
                    "endogeneity",
                    "hypothesis_test",
                    "hypothesis",
                ]
            } else {
                &[
                    "model_summary",
                    "coefficient_table",
                    "first_stage",
                    "overidentification",
                    "hypothesis_test",
                    "hypothesis",
                ]
            },
            1,
            move |inv| summary(kind, inv),
        );
    }
    install(
        builder,
        "yssbi.statistics.panel.did.twfe",
        vec![
            Input::fixed("y"),
            Input::repeated("x", 0..=usize::MAX),
            Input::fixed("entity"),
            Input::fixed("time"),
            Input::fixed("treatment"),
        ],
        &[],
        1,
        did,
    );
    install(
        builder,
        "yssbi.statistics.panel.did.randomization",
        vec![
            Input::fixed("y"),
            Input::repeated("x", 0..=usize::MAX),
            Input::fixed("entity"),
            Input::fixed("time"),
            Input::fixed("treat"),
            Input::fixed("post"),
        ],
        &[
            "repetitions",
            "seed",
            "constant",
            "covariance",
            "use_observed_coefficient",
            "observed_coefficient",
        ],
        1,
        randomization,
    );
}

fn randomization(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let post = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let treat = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let time = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let entity = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let response = data.remove(0);
    check_fit_workspace(response.len(), data.len() + 1, true, "GLS", inv)?;
    let repetitions = integer(inv, "repetitions")?;
    if !(10..=2000).contains(&repetitions) {
        return Err(KernelError::InvalidParameter);
    }
    let result = yss_sci_runtime::causal::did::randomization_test(
        yss_sci_contract::causal::did::DidRandomizationInput {
            constant: boolean(inv, "constant")?,
            covariance: text(inv, "covariance")?.into(),
            observed_coefficient: if boolean(inv, "use_observed_coefficient")? {
                Some(number(inv, "observed_coefficient")?)
            } else {
                None
            },
            response,
            predictors: data,
            entity,
            time,
            treat,
            post,
            repetitions,
            seed: integer(inv, "seed")? as u64,
        },
        &yss_sci_contract::execution::ScientificExecutionControl::from_shared(
            inv.control.cancellation.clone(),
            inv.control.deadline,
        ),
    );
    inv.check_control()?;
    let mut report = serde_json::to_value(result.map_err(|_| KernelError::ScientificFailure)?)
        .map_err(|_| KernelError::ScientificFailure)?;
    report["estimand"] = serde_json::json!("ATT (TWFE treatment-by-post coefficient)");
    report["treatName"] = serde_json::json!(input_label(group(inv, "treat")[0], "treat".into()));
    report["postName"] = serde_json::json!(input_label(group(inv, "post")[0], "post".into()));
    report["covariance"] = serde_json::json!(text(inv, "covariance")?);
    report["constant"] = serde_json::json!(boolean(inv, "constant")?);
    let mut row = report.clone();
    for key in [
        "observed_coef",
        "p_value_ri",
        "perm_coef_mean",
        "perm_coef_std",
        "unavailableCode",
    ] {
        if row.get(key).is_none() {
            row[key] = serde_json::Value::Null;
        }
    }
    report["ri_table"] = serde_json::json!([row]);
    yss_sci_runtime::report_display::section(
        &mut report,
        "randomization",
        "DID randomization inference",
        "table",
        "/ri_table",
        &[
            ("available", "Available"),
            ("observed_coef", "Observed ATT"),
            ("p_value_ri", "Randomization p-value"),
            ("n_perm", "Requested permutations"),
            ("n_perm_valid", "Valid permutations"),
            ("n_entities", "Entities"),
            ("n_treated_entities", "Treated entities"),
            ("perm_coef_mean", "Permutation mean"),
            ("perm_coef_std", "Permutation SD"),
            ("unavailableCode", "Unavailable reason"),
        ],
    );
    let result = value(report, inv)?;
    Ok(vec![result])
}

fn iv(kind: IvKind, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let response = data.remove(0);
    let exog = group(inv, "x").len();
    let endog = group(inv, "endogenous").len();
    let constant = boolean(inv, "constant")?;
    check_fit_workspace(response.len(), data.len(), constant, "OLS", inv)?;
    let mut fit = yss_sci_runtime::causal::iv::fit_instrumental_variables(
        kind,
        response,
        &data[..exog],
        &data[exog..exog + endog],
        &data[exog + endog..],
        OlsOptions {
            constant,
            covariance: super::linear::covariance(inv)?,
        },
        boolean(inv, "small")?,
    )
    .map_err(sci)?;
    fit.response_name = input_label(&inv.inputs[0], "response".into());
    fit.parameter_names = std::iter::once("_cons".to_string())
        .take(usize::from(constant))
        .chain(
            group(inv, "x")
                .into_iter()
                .chain(group(inv, "endogenous"))
                .enumerate()
                .map(|(j, v)| input_label(v, format!("x{}", j + 1))),
        )
        .collect();
    fit.instrument_names = group(inv, "instruments")
        .iter()
        .enumerate()
        .map(|(j, v)| input_label(v, format!("z{}", j + 1)))
        .collect();
    let model = value(fit, inv)?;
    Ok(vec![
        model.clone(),
        field(&model, "fitted")?.clone(),
        field(&model, "residuals")?.clone(),
    ])
}

fn did(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let treatment = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let time = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let entity = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let response = data.remove(0);
    if treatment.iter().any(|v| !matches!(*v, 0.0 | 1.0)) {
        return Err(KernelError::InvalidNumericInput);
    }
    check_fit_workspace(response.len(), data.len() + 1, true, "GLS", inv)?;
    let mut fit = yss_sci_runtime::causal::did::fit_did(response, data, entity, time, treatment)
        .map_err(sci)?;
    let treatment_index = group(inv, "x").len() + 1;
    let treatment_label = format!("x{treatment_index}");
    if fit
        .omitted_terms
        .iter()
        .any(|t| t.variable == treatment_label)
    {
        return Err(KernelError::ScientificFailure);
    }
    for name in &mut fit.parameter_names {
        if *name == treatment_label {
            *name = "ATT".into();
        }
    }
    super::panel::name_fit(&mut fit, inv);
    validate_finite(&fit)?;
    let summary = yss_sci_runtime::panel::summary(&fit, Default::default()).map_err(sci)?;
    let mut report = serde_json::json!({"model":fit,"summary":summary,"estimand":"TWFE treatment coefficient; ATT interpretation requires the DID identification assumptions","treatmentName":input_label(group(inv,"treatment")[0],"treatment".into())});
    if let Some(mut display) = report["summary"].get("report_display").cloned() {
        if let Some(sections) = display["sections"].as_object_mut() {
            for section in sections.values_mut() {
                if let Some(path) = section["path"].as_str() {
                    section["path"] = serde_json::json!(format!("/summary{path}"));
                }
            }
        }
        report["report_display"] = display;
    }
    Ok(vec![value(report, inv)?])
}

fn summary(kind: IvKind, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::causal::iv::InstrumentalVariableFit = decode_model(inv)?;
    let is_2sls = matches!(kind, IvKind::TwoStageLeastSquares);
    if fit.family != if is_2sls { "iv_2sls" } else { "iv_liml" } {
        return Err(KernelError::InvalidNumericInput);
    }
    let options = yss_sci_contract::causal::iv::IvSummaryOptions {
        model_summary: boolean(inv, "model_summary")?,
        coefficient_table: boolean(inv, "coefficient_table")?,
        first_stage: boolean(inv, "first_stage")?,
        overidentification: boolean(inv, "overidentification")?,
        endogeneity: is_2sls && boolean(inv, "endogeneity")?,
    };
    if options.first_stage || options.endogeneity || (options.overidentification && is_2sls) {
        check_fit_workspace(
            fit.residuals.len(),
            fit.design.exogenous.len() + fit.design.endogenous.len() + fit.design.instruments.len(),
            fit.options.constant,
            "OLS",
            inv,
        )?;
    }
    let mut report = yss_sci_runtime::causal::iv::summary(&fit, options).map_err(sci)?;
    if boolean(inv, "hypothesis_test")? {
        let input = yss_sci_contract::hypothesis::HypothesisTestInput {
            betas: fit.coefficients.clone(),
            cov_beta: fit.inference.covariance.clone(),
            df_residual: fit.statistics.df_residual,
            param_names: fit.parameter_names.clone(),
            hypothesis: text(inv, "hypothesis")?.into(),
        };
        let result = if fit.small {
            yss_sci_runtime::hypothesis::run_hypothesis_test(input)
        } else {
            yss_sci_runtime::hypothesis::run_asymptotic_hypothesis_test(input)
        }
        .map_err(|_| KernelError::InvalidParameter)?;
        report["hypothesisTest"] =
            serde_json::to_value(result).map_err(|_| KernelError::ScientificFailure)?;
    }
    let report = value(report, inv)?;
    Ok(vec![report])
}
