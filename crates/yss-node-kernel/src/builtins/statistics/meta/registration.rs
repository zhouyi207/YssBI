//! Runtime contracts matching the catalog's study-summary interfaces.
use super::*;

pub(in crate::builtins::statistics) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "continuous",
        "binary",
        "single_proportion",
        "mean",
        "correlation",
        "or_hr",
        "combine_p",
        "inverse_variance",
        "fixed_effect",
        "random_effect",
        "cochran_q",
        "i_squared",
        "tau_squared",
        "regression",
        "egger",
        "begg",
        "leave_one_out",
        "sensitivity",
        "forest",
        "funnel",
    ] {
        let keys: &[&str] = match method {
            "continuous" => &[
                "treatment_mean",
                "treatment_sd",
                "treatment_n",
                "reference_mean",
                "reference_sd",
                "reference_n",
            ],
            "binary" => &[
                "treatment_events",
                "treatment_n",
                "reference_events",
                "reference_n",
            ],
            "single_proportion" => &["events", "sample_size"],
            "mean" => &["mean", "sd", "sample_size"],
            "correlation" => &["correlation", "sample_size"],
            "or_hr" => &["ratio", "lower", "upper"],
            "combine_p" => &["p_values"],
            _ => &["effects", "variances"],
        };
        let mut inputs = keys.iter().map(|key| Input::fixed(key)).collect::<Vec<_>>();
        if method == "regression" {
            inputs.push(Input::repeated("moderators", 1..=usize::MAX));
        }
        if method == "combine_p" {
            inputs.push(Input::repeated("weights", 0..=1));
        }
        let outputs = if matches!(
            method,
            "continuous"
                | "binary"
                | "single_proportion"
                | "mean"
                | "correlation"
                | "or_hr"
                | "inverse_variance"
                | "fixed_effect"
                | "random_effect"
                | "regression"
                | "leave_one_out"
                | "sensitivity"
        ) {
            2
        } else {
            1
        };
        let family = if matches!(method, "forest" | "funnel") {
            "plot"
        } else {
            "meta"
        };
        install(
            builder,
            &format!("yssbi.statistics.{family}.{method}"),
            inputs,
            &parameters(method),
            outputs,
            move |inv| execute(method, inv),
        );
    }
}
fn parameters(method: &str) -> Vec<&'static str> {
    let mut keys = vec![];
    if !matches!(
        method,
        "combine_p" | "cochran_q" | "i_squared" | "tau_squared" | "begg"
    ) {
        keys.push("confidence_level");
    }
    if matches!(method, "continuous" | "binary" | "single_proportion") {
        keys.push("effect_measure");
    }
    if matches!(method, "binary" | "single_proportion") {
        keys.push("continuity_correction");
    }
    if method == "or_hr" {
        keys.push("source_confidence_level");
    }
    if method == "combine_p" {
        keys.push("p_method");
    }
    if matches!(
        method,
        "random_effect"
            | "tau_squared"
            | "inverse_variance"
            | "regression"
            | "leave_one_out"
            | "sensitivity"
            | "forest"
            | "funnel"
    ) {
        keys.push("estimator");
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
        keys.push("inference");
    }
    if method == "forest" {
        keys.push("exponentiate");
    }
    keys
}
