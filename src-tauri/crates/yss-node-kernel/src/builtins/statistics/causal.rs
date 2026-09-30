use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{causal::iv::InstrumentalVariableKind as IvKind, regression::OlsOptions};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (name, kind) in [
        ("2sls", IvKind::TwoStageLeastSquares),
        ("liml", IvKind::LimitedInformationMaximumLikelihood),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.iv.{name}.fit"),
            vec![
                Input::fixed("response"),
                Input::repeated("predictors", 0..=usize::MAX),
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
                ]
            } else {
                &[
                    "model_summary",
                    "coefficient_table",
                    "first_stage",
                    "overidentification",
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
            Input::fixed("response"),
            Input::repeated("predictors", 0..=usize::MAX),
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
            Input::fixed("response"),
            Input::repeated("predictors", 0..=usize::MAX),
            Input::fixed("entity"),
            Input::fixed("time"),
            Input::fixed("treat"),
            Input::fixed("post"),
        ],
        &["repetitions", "seed"],
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
    let result = value(result.map_err(|_| KernelError::ScientificFailure)?, inv)?;
    Ok(vec![result])
}

fn iv(kind: IvKind, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let response = data.remove(0);
    let exog = group(inv, "predictors").len();
    let endog = group(inv, "endogenous").len();
    let constant = boolean(inv, "constant")?;
    // IV first-stage and identification diagnostics form dense observation projections.
    check_fit_workspace(response.len(), data.len(), constant, "GLS", inv)?;
    let fit = yss_sci_runtime::causal::iv::fit_instrumental_variables(
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
    let result = yss_sci_runtime::causal::did::fit_did(response, data, entity, time, treatment)
        .map_err(sci)?;
    let fit: yss_sci_contract::panel::PanelFit =
        serde_json::from_value(result).map_err(|_| KernelError::ScientificFailure)?;
    let report = value(
        yss_sci_runtime::panel::summary(&fit, Default::default()),
        inv,
    )?;
    Ok(vec![RuntimeValue::Record(std::sync::Arc::new(
        [
            ("model".into(), value(fit, inv)?),
            ("summary".into(), report),
        ]
        .into(),
    ))])
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
    if options.first_stage || options.overidentification || options.endogeneity {
        check_fit_workspace(
            fit.residuals.len(),
            fit.design.exogenous.len() + fit.design.endogenous.len() + fit.design.instruments.len(),
            fit.options.constant,
            "GLS",
            inv,
        )?;
    }
    let report = yss_sci_runtime::causal::iv::summary(&fit, options).map_err(sci)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}
