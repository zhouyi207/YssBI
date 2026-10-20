//! Method-specific settings and their bilingual descriptions.
use super::*;

pub(super) fn build(id: &str, method: &str) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = Vec::new();
    if !matches!(
        method,
        "combine_p" | "cochran_q" | "i_squared" | "tau_squared" | "begg"
    ) {
        parameters.push(decimal_parameter("confidence_level", "0.95")?);
    }
    let measures: Option<(&str, &[&str])> = match method {
        "continuous" => Some(("mean_difference", &["mean_difference", "hedges_g"])),
        "binary" => Some((
            "log_odds_ratio",
            &["log_odds_ratio", "log_risk_ratio", "risk_difference"],
        )),
        "single_proportion" => Some((
            "logit_proportion",
            &["proportion", "logit_proportion", "arcsine_proportion"],
        )),
        _ => None,
    };
    if let Some((default, values)) = measures {
        parameters.push(choice_parameter("effect_measure", default, values)?);
    }
    if matches!(method, "binary" | "single_proportion") {
        parameters.push(decimal_parameter("continuity_correction", "0.5")?);
    }
    if method == "or_hr" {
        parameters.push(decimal_parameter("source_confidence_level", "0.95")?);
    }
    if method == "combine_p" {
        parameters.push(choice_parameter(
            "p_method",
            "fisher",
            &["fisher", "stouffer"],
        )?);
    }
    if matches!(method, "random_effect" | "tau_squared") {
        parameters.push(choice_parameter(
            "estimator",
            "paule_mandel",
            &["der_simonian_laird", "paule_mandel"],
        )?);
    } else if matches!(
        method,
        "inverse_variance" | "regression" | "leave_one_out" | "sensitivity" | "forest" | "funnel"
    ) {
        parameters.push(choice_parameter(
            "estimator",
            if method == "inverse_variance" {
                "fixed"
            } else {
                "paule_mandel"
            },
            &["fixed", "der_simonian_laird", "paule_mandel"],
        )?);
    }
    if matches!(
        method,
        "fixed_effect"
            | "random_effect"
            | "inverse_variance"
            | "regression"
            | "leave_one_out"
            | "sensitivity"
            | "forest"
    ) {
        parameters.push(choice_parameter(
            "inference",
            "wald",
            &["wald", "knapp_hartung"],
        )?);
    }
    if method == "forest" {
        parameters.push(toggle_parameter("exponentiate", false)?);
    }
    node_scoped_keys(id, &mut parameters)?;
    Ok(parameters)
}
