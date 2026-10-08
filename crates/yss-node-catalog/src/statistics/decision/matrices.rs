use super::*;
const NODES: &[(&str, &str, &str)] = &[
    ("ahp", "AHP judgment weights", "AHP 层次分析"),
    ("fahp", "Fuzzy AHP weights", "模糊层次法 FAHP"),
    ("dematel", "DEMATEL influences", "DEMATEL"),
    (
        "ism",
        "Interpretive structural modeling",
        "ISM 解释结构模型",
    ),
    (
        "fuzzy_evaluation",
        "Fuzzy comprehensive evaluation",
        "模糊综合评价",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &(method, en, zh) in NODES {
        let mut parameters = vec![];
        let mut ports = vec![bounded_user_data_input(
            if method == "fuzzy_evaluation" {
                "memberships"
            } else {
                "criteria"
            },
            "Matrix column",
            series_type()?,
            1,
            None,
        )?];
        if method == "fuzzy_evaluation" {
            ports.extend([
                bounded_user_data_input(
                    "criterion_weights",
                    "Criterion weights",
                    series_type()?,
                    0,
                    Some(1),
                )?,
                bounded_user_data_input(
                    "grade_scores",
                    "Grade scores",
                    series_type()?,
                    0,
                    Some(1),
                )?,
            ]);
            parameters.push(choice_parameter(
                "fuzzy_operator",
                "product_sum",
                &["product_sum", "min_max", "product_max", "min_sum"],
            )?);
        }
        ports.push(data_output("result", "Summary", report_type()?)?);
        match method {
            "ahp" | "fahp" => {
                ports.push(data_output("weights", "Criterion weights", series_type()?)?);
                if method == "ahp" {
                    parameters.push(decimal_parameter("random_index", "0")?);
                }
            }
            "dematel" => {
                parameters.extend([
                    choice_parameter("influence_normalization", "max_sum", &["max_sum", "none"])?,
                    decimal_parameter("attenuation", "1")?,
                ]);
                ports.extend([
                    fixed_numeric_table(
                        "indices",
                        "Influence indices",
                        &[
                            "criterion",
                            "outgoing",
                            "incoming",
                            "prominence",
                            "net_cause",
                            "weight",
                        ],
                    )?,
                    fixed_numeric_table(
                        "relations",
                        "Influence matrix",
                        &["source", "target", "direct", "total"],
                    )?,
                ]);
            }
            "ism" => {
                ports.extend([
                    fixed_numeric_table(
                        "indices",
                        "System levels",
                        &["criterion", "level", "driving_power", "dependence"],
                    )?,
                    fixed_numeric_table(
                        "relations",
                        "Reachability matrix",
                        &["source", "target", "reachable"],
                    )?,
                ]);
            }
            _ => ports.push(data_output(
                "combined_memberships",
                "Combined memberships",
                series_type()?,
            )?),
        }
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
