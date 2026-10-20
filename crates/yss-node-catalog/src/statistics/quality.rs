//! Process-control and balanced crossed measurement-study declarations.
use super::*;
const ENTRIES: [(&str,); 3] = [
    ("plot.control_chart",),
    ("quality.process_capability",),
    ("quality.measurement_system",),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (suffix,) in ENTRIES {
        let id = format!("yssbi.statistics.{suffix}");
        let chart = suffix == "plot.control_chart";
        let mut ports = vec![data_input(
            "measurements",
            "Measurement",
            numeric_data_series_type(),
        )?];
        let mut parameters = if chart {
            vec![choice_parameter(
                "chart_kind",
                "individuals",
                &["individuals", "moving_range"],
            )?]
        } else if suffix == "quality.process_capability" {
            ports.push(bounded_user_data_input(
                "subgroups",
                "Rational subgroup",
                label_series()?,
                0,
                Some(1),
            )?);
            vec![
                decimal_parameter("lower_limit", "0")?,
                decimal_parameter("upper_limit", "10")?,
                decimal_parameter("target", "5")?,
            ]
        } else {
            ports.extend([
                data_input("parts", "Part identifier", label_series()?)?,
                data_input("operators", "Operator identifier", label_series()?)?,
            ]);
            vec![toggle_parameter("include_interaction", true)?]
        };
        ports.push(data_output(
            "result",
            "Result",
            if chart {
                concrete("plot.data")?
            } else {
                report_type()?
            },
        )?);
        if chart {
            ports.extend([
                data_output("summary", "Control limits and signals", report_type()?)?,
                fixed_numeric_table(
                    "observations",
                    "All chart observations",
                    &[
                        "observation",
                        "value",
                        "center",
                        "lower",
                        "upper",
                        "outside",
                    ],
                )?,
            ]);
        }
        node_scoped_keys(&id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: None,
                    category_id: sid("statistics.design_quality", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}
