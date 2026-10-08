//! Objective weights and multi-criteria rankings with reusable weight outputs.
use super::*;
const NODES: &[(&str, &str, &str)] = &[
    ("decision.weights", "Decision weights", "权重"),
    ("decision.entropy_weight", "Entropy weights", "熵值法"),
    ("decision.critic", "CRITIC weights", "CRITIC 权重"),
    (
        "decision.information_weight",
        "Coefficient-of-variation weights",
        "信息量权重",
    ),
    (
        "decision.independence_weight",
        "Independence weights",
        "独立性权重",
    ),
    ("decision.composite_index", "Composite index", "综合指数"),
    ("decision.topsis", "TOPSIS", "TOPSIS"),
    (
        "decision.grey_relational",
        "Grey relational evaluation",
        "灰色关联法",
    ),
    ("decision.wrsr", "Weighted rank-sum ratio", "WRSR 秩和比"),
    (
        "decision.efficacy_coefficient",
        "Efficacy coefficient",
        "功效系数",
    ),
    (
        "workflow.entropy_topsis",
        "Entropy-weighted TOPSIS",
        "熵权 TOPSIS",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(suffix, en, zh) in NODES {
        let method = suffix.split('.').next_back().unwrap();
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
            series_type()?,
            1,
            None,
        )?];
        let mut parameters = vec![parameter(
            "cost_criteria",
            series_type()?,
            ParameterEditorSpec::Auto,
            DataValue::List(vec![]),
            vec![],
        )?];
        if customizable {
            ports.push(bounded_user_data_input(
                "criterion_weights",
                "Criterion weights",
                series_type()?,
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
            data_output("weights", "Criterion weights", series_type()?)?,
        ]);
        emit(fragment, suffix, en, zh, ports, parameters)?;
    }
    Ok(())
}
