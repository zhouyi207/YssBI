//! Objective weights and multi-criteria rankings with reusable weight outputs.
use super::*;
const NODES: &[&str] = &[
    "decision.weights",
    "decision.entropy_weight",
    "decision.critic",
    "decision.information_weight",
    "decision.independence_weight",
    "decision.composite_index",
    "decision.topsis",
    "decision.grey_relational",
    "decision.wrsr",
    "decision.efficacy_coefficient",
    "workflow.entropy_topsis",
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &suffix in NODES {
        let method = suffix.rsplit_once('.').map_or(suffix, |(_, method)| method);
        let customizable = matches!(
            method,
            "weights"
                | "composite_index"
                | "topsis"
                | "grey_relational"
                | "wrsr"
                | "efficacy_coefficient"
        );
        let mut ports = vec![bounded_user_data_input(
            "criteria",
            "Criterion",
            numeric_data_series_type(),
            1,
            None,
        )?];
        let mut parameters = vec![parameter(
            "cost_criteria",
            numeric_data_series_type(),
            ParameterEditorSpec::Auto,
            DataValue::List(vec![]),
            vec![],
        )?];
        if customizable {
            ports.push(bounded_user_data_input(
                "criterion_weights",
                "Criterion weights",
                numeric_data_series_type(),
                0,
                Some(1),
            )?);
            parameters.push(choice_parameter(
                "weight_method",
                if method == "weights" {
                    "entropy"
                } else {
                    "equal"
                },
                &[
                    "equal",
                    "explicit",
                    "entropy",
                    "critic",
                    "information",
                    "independence",
                ],
            )?);
        }
        if !matches!(method, "grey_relational" | "wrsr" | "efficacy_coefficient") {
            parameters.push(choice_parameter(
                "normalization",
                if matches!(method, "topsis" | "entropy_topsis") {
                    "vector"
                } else if matches!(method, "information_weight" | "independence_weight") {
                    "none"
                } else {
                    "minmax"
                },
                &["none", "minmax", "vector"],
            )?);
        }
        if method == "grey_relational" {
            parameters.push(decimal_parameter("resolution", "0.5")?);
        }
        ports.extend([
            data_output("result", "Summary", report_type()?)?,
            fixed_numeric_table(
                "scores",
                "Scores",
                if matches!(method, "topsis" | "entropy_topsis") {
                    &[
                        "observation",
                        "score",
                        "rank",
                        "distance_best",
                        "distance_worst",
                    ]
                } else {
                    &["observation", "score", "rank"]
                },
            )?,
            data_output("weights", "Criterion weights", numeric_data_series_type())?,
        ]);
        emit(fragment, suffix, ports, parameters)?;
    }
    Ok(())
}
