use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::panel::{PanelEffects as Effects, PanelEstimator as Estimator, PanelOptions};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    install(
        builder,
        "yssbi.statistics.panel.predict",
        vec![
            Input::fixed("model"),
            Input::repeated("predictors", 1..=usize::MAX),
        ],
        &[],
        1,
        predict,
    );
    install(
        builder,
        "yssbi.statistics.panel.compare",
        vec![
            Input::fixed("response"),
            Input::repeated("predictors", 1..=usize::MAX),
            Input::fixed("entity"),
            Input::fixed("time"),
        ],
        &["constant", "effects", "covariance", "estimators"],
        1,
        compare,
    );
    install(
        builder,
        "yssbi.statistics.panel.fit",
        vec![
            Input::fixed("response"),
            Input::repeated("predictors", 1..=usize::MAX),
            Input::fixed("entity"),
            Input::fixed("time"),
        ],
        &["constant", "estimator", "effects", "covariance"],
        3,
        fit,
    );
    install(
        builder,
        "yssbi.statistics.panel.summary",
        vec![Input::fixed("model")],
        &[
            "model_summary",
            "coefficient_table",
            "effects_statistics",
            "estimator_statistics",
        ],
        1,
        summary,
    );
}

fn summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit = decode_model(inv)?;
    let report = yss_sci_runtime::panel::summary(
        &fit,
        yss_sci_contract::panel::PanelSummaryOptions {
            model_summary: boolean(inv, "model_summary")?,
            coefficient_table: boolean(inv, "coefficient_table")?,
            effects_statistics: boolean(inv, "effects_statistics")?,
            estimator_statistics: boolean(inv, "estimator_statistics")?,
        },
    )
    .map_err(sci)?;
    let report = value(report, inv)?;
    Ok(vec![report])
}
fn fit(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let time = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let entity = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let response = data.remove(0);
    let options = PanelOptions {
        estimator: match text(inv, "estimator")? {
            "fixed_effects" => Estimator::FixedEffects,
            "lsdv" => Estimator::Lsdv,
            "first_difference" => Estimator::FirstDifference,
            "random_effects" => Estimator::RandomEffects,
            "maximum_likelihood" => Estimator::MaximumLikelihood,
            "between" => Estimator::Between,
            _ => return Err(KernelError::InvalidParameter),
        },
        effects: match text(inv, "effects")? {
            "entity" => Effects::Entity,
            "time" => Effects::Time,
            "two_way" => Effects::TwoWay,
            _ => return Err(KernelError::InvalidParameter),
        },
        constant: boolean(inv, "constant")?,
        covariance: text(inv, "covariance")?.into(),
    };
    // LSDV may expand one dummy per observation; RE can use dense covariance workspaces.
    check_fit_workspace(
        response.len(),
        data.len() + response.len(),
        options.constant,
        "GLS",
        inv,
    )?;
    let encoded =
        yss_sci_runtime::panel::fit_panel(response, data, entity, time, options).map_err(sci)?;
    let mut fit: yss_sci_contract::panel::PanelFit =
        serde_json::from_value(encoded).map_err(|_| KernelError::ScientificFailure)?;
    name_fit(&mut fit, inv);
    let fitted = numeric_list(&fit.estimation.fitted, inv)?;
    let residuals = numeric_list(&fit.estimation.residuals, inv)?;
    Ok(vec![value(fit, inv)?, fitted, residuals])
}

pub(super) fn name_fit(fit: &mut yss_sci_contract::panel::PanelFit, inv: &KernelInvocation<'_>) {
    fit.response_name = input_label(&inv.inputs[0], "response".into());
    let names = group(inv, "predictors")
        .iter()
        .enumerate()
        .map(|(j, v)| (format!("x{}", j + 1), input_label(v, format!("x{}", j + 1))))
        .collect::<std::collections::HashMap<_, _>>();
    for name in &mut fit.parameter_names {
        if let Some(actual) = names.get(name) {
            *name = actual.clone();
        }
    }
    for term in &mut fit.omitted_terms {
        if let Some(actual) = names.get(&term.variable) {
            term.variable = actual.clone();
        }
    }
}

fn predict(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::panel::PanelFit = decode_model(inv)?;
    let predictors = columns(&group(inv, "predictors"), inv, 0)?;
    let n = predictors.first().map_or(0, Vec::len);
    check_fit_workspace(n, predictors.len(), fit.estimation.constant, "OLS", inv)?;
    let values = yss_sci_runtime::panel::predict(&fit, &predictors).map_err(sci)?;
    Ok(vec![numeric_list(&values, inv)?])
}
fn compare(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let time = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let entity = data.pop().ok_or(KernelError::ShapeMismatch)?;
    let response = data.remove(0);
    let requested = text(inv, "estimators")?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if requested.is_empty()
        || requested.len() > 6
        || requested
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != requested.len()
    {
        return Err(KernelError::InvalidParameter);
    }
    check_fit_workspace(
        response.len(),
        data.len() + response.len(),
        boolean(inv, "constant")?,
        "GLS",
        inv,
    )?;
    inv.control.check_bytes(
        response
            .len()
            .checked_mul(data.len() + response.len())
            .and_then(|v| v.checked_mul(requested.len()))
            .and_then(|v| v.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)),
    )?;
    let effects = match text(inv, "effects")? {
        "entity" => Effects::Entity,
        "time" => Effects::Time,
        "two_way" => Effects::TwoWay,
        _ => return Err(KernelError::InvalidParameter),
    };
    let mut models = Vec::new();
    let mut coefficient_rows = Vec::new();
    let mut statuses = Vec::new();
    for name in requested {
        inv.check_control()?;
        let estimator = match name {
            "fixed_effects" => Estimator::FixedEffects,
            "lsdv" => Estimator::Lsdv,
            "first_difference" => Estimator::FirstDifference,
            "random_effects" => Estimator::RandomEffects,
            "maximum_likelihood" => Estimator::MaximumLikelihood,
            "between" => Estimator::Between,
            _ => return Err(KernelError::InvalidParameter),
        };
        let result = yss_sci_runtime::panel::fit_panel(
            response.clone(),
            data.clone(),
            entity.clone(),
            time.clone(),
            PanelOptions {
                estimator,
                effects,
                constant: boolean(inv, "constant")?,
                covariance: text(inv, "covariance")?.into(),
            },
        );
        inv.check_control()?;
        match result {
            Ok(encoded)=>{
                let mut fit:yss_sci_contract::panel::PanelFit=serde_json::from_value(encoded).map_err(|_|KernelError::ScientificFailure)?;name_fit(&mut fit,inv);
                for mut row in yss_sci_runtime::report_display::coefficient_rows(&fit.parameter_names,&fit.coefficients,&fit.inference).map_err(sci)? {row["estimator"]=serde_json::json!(name);coefficient_rows.push(row);}
                statuses.push(serde_json::json!({"estimator":name,"status":"success","failure":null}));models.push(serde_json::json!({"estimator":name,"model":fit}));
            },
            Err(error)=>statuses.push(serde_json::json!({"estimator":name,"status":"failed","failure":format!("{error:?}")})),
        }
    }
    let mut report = serde_json::json!({"models":models,"model_statuses":statuses,"coefficient_rows":coefficient_rows});
    yss_sci_runtime::report_display::section(
        &mut report,
        "statuses",
        "Estimator outcomes",
        "table",
        "/model_statuses",
        &[
            ("estimator", "Estimator"),
            ("status", "Status"),
            ("failure", "Failure reason"),
        ],
    );
    yss_sci_runtime::report_display::section(
        &mut report,
        "coefficients",
        "Panel model comparison",
        "table",
        "/coefficient_rows",
        &[
            ("estimator", "Estimator"),
            ("variable", "Variable"),
            ("estimate", "Coefficient"),
            ("standard_error", "Std. error"),
            ("p_value", "p-value"),
        ],
    );
    Ok(vec![value(report, inv)?])
}
