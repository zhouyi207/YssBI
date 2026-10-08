//! Process-control and balanced crossed measurement-study declarations.
use super::*;
const ENTRIES: [(&str, &str, &str); 3] = [
    (
        "plot.control_chart",
        "Quality control chart (I / MR)",
        "质量控制图（I / MR）",
    ),
    (
        "quality.process_capability",
        "Process capability",
        "过程能力分析",
    ),
    (
        "quality.measurement_system",
        "Measurement system (crossed Gage R&R)",
        "测量系统分析（交叉 Gage R&R）",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (suffix, en, zh) in ENTRIES {
        let id = format!("yssbi.statistics.{suffix}");
        let chart = suffix == "plot.control_chart";
        let mut ports = vec![data_input("measurements", "Measurement", series_type()?)?];
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
        localize(fragment, &id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: None,
                    category_id: sid("statistics.design_quality", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment.messages.extend([
                (locale, node_key_text(&id, "title"), Text(title)),
                (locale, node_key_text(&id, "documentation"), Text(title)),
            ]);
        }
    }
    Ok(())
}
fn localize(
    fragment: &mut ProviderFragment,
    id: &str,
    parameters: &mut [Parameter],
) -> Result<(), BuiltinAssemblyError> {
    for p in parameters {
        let key = p.key.as_str();
        let (en, zh, eh, zhh) = match key {
            "chart_kind" => (
                "Chart statistic",
                "控制图统计量",
                "Individuals, or the absolute difference between consecutive measurements.",
                "单值 I 图，或相邻观测绝对差值的移动极差 MR 图；按输入顺序计算。",
            ),
            "lower_limit" => (
                "Lower specification limit",
                "规格下限",
                "Engineering specification, not an estimated control limit.",
                "工程或设计规格下限，与统计控制限不同。",
            ),
            "upper_limit" => (
                "Upper specification limit",
                "规格上限",
                "Must exceed the lower specification limit.",
                "须大于规格下限。",
            ),
            "target" => (
                "Target value",
                "目标值",
                "Target for the Cpm index; does not alter Cp or Cpk.",
                "用于 Cpm 的目标值，不改变 Cp 或 Cpk。",
            ),
            "include_interaction" => (
                "Part × operator interaction",
                "零件 × 操作者交互",
                "Retain the random interaction, or pool its sum of squares into repeatability. No automatic significance-based selection.",
                "保留随机交互项；关闭时将交互平方和与自由度合并入重复性误差。不按显著性自动切换。",
            ),
            _ => unreachable!(),
        };
        p.title_key = node_key(id, &format!("parameters.{key}.title"))?;
        p.description_key = Some(node_key(id, &format!("parameters.{key}.description"))?);
        for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zhh)] {
            fragment.messages.extend([
                (locale, p.title_key.as_str().to_string(), Text(title)),
                (
                    locale,
                    p.description_key.as_ref().unwrap().as_str().to_string(),
                    Text(help),
                ),
            ]);
        }
    }
    Ok(())
}
