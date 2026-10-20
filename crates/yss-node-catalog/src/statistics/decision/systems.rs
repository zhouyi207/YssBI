use super::*;
const NODES: &[&str] = &["vikor", "coupling_coordination", "obstacle_degree"];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in NODES {
        let mut parameters = vec![];
        if method != "coupling_coordination" {
            parameters.push(parameter(
                "cost_criteria",
                numeric_data_series_type(),
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
                numeric_data_series_type(),
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
                numeric_data_series_type(),
                0,
                Some(1),
            )?,
            data_output("result", "Summary", report_type()?)?,
            fixed_numeric_table("scores", "Observations", fields)?,
            data_output("weights", "Criterion weights", numeric_data_series_type())?,
        ];
        emit(fragment, &format!("decision.{method}"), ports, parameters)?;
    }
    Ok(())
}
