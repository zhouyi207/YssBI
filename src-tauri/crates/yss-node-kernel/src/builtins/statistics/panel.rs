use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::panel::{PanelEffects as Effects, PanelEstimator as Estimator, PanelOptions};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
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
        1,
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
        2,
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
    );
    let report = value(report, inv)?;
    Ok(vec![report.clone(), report])
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
    let fit =
        yss_sci_runtime::panel::fit_panel(response, data, entity, time, options).map_err(sci)?;
    Ok(vec![value(fit, inv)?])
}
