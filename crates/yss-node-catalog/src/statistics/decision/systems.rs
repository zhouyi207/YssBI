use super::*;
const NODES: &[(&str, &str, &str)] = &[
    ("vikor", "VIKOR compromise ranking", "VIKOR"),
    (
        "coupling_coordination",
        "Coupling coordination",
        "耦合协调度",
    ),
    ("obstacle_degree", "Obstacle degree", "障碍度"),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in NODES {
        let mut parameters = vec![];
        if method != "coupling_coordination" {
            parameters.push(parameter(
                "cost_criteria",
                series_type()?,
                ParameterEditorSpec::Auto,
                DataValue::List(vec![]),
                vec![],
            )?);
        }
        if method == "vikor" {
            parameters.push(decimal_parameter("majority_weight", "0.5")?);
        }
        if method == "obstacle_degree" {
            parameters.push(toggle_parameter("rescale", true)?);
        }
        let fields: &[&str] = match method {
            "vikor" => &[
                "observation",
                "compromise_score",
                "group_utility",
                "individual_regret",
                "rank",
            ],
            "coupling_coordination" => &[
                "observation",
                "coupling",
                "coordination_index",
                "coordination_degree",
            ],
            _ => &[
                "observation",
                "criterion",
                "deviation",
                "weighted_deviation",
                "obstacle_percent",
            ],
        };
        let ports = vec![
            bounded_user_data_input(
                "criteria",
                "Criterion",
                series_type()?,
                if method == "coupling_coordination" {
                    2
                } else {
                    1
                },
                None,
            )?,
            bounded_user_data_input(
                "criterion_weights",
                "Criterion weights",
                series_type()?,
                0,
                Some(1),
            )?,
            data_output("result", "Summary", report_type()?)?,
            fixed_numeric_table("scores", "Observations", fields)?,
            data_output("weights", "Criterion weights", series_type()?)?,
        ];
        emit(
            fragment,
            &format!("decision.{method}"),
            en,
            zh,
            ports,
            parameters,
        )?;
    }
    Ok(())
}
