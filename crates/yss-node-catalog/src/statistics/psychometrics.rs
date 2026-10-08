//! Item-scale interfaces; numerical estimators live in SCI.
use super::*;
const ENTRIES: [(&str, &str, &str); 4] = [
    (
        "reliability",
        "Reliability (Cronbach alpha)",
        "信度（Cronbach α）",
    ),
    (
        "validity",
        "Validity screening (KMO / Bartlett)",
        "效度检验（KMO / Bartlett）",
    ),
    (
        "content_validity",
        "Content validity (expert CVI)",
        "内容效度（专家 CVI）",
    ),
    (
        "item_analysis",
        "Item analysis (discrimination)",
        "题项分析（区分度）",
    ),
];
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (suffix, en, zh) in ENTRIES {
        let id = format!("yssbi.statistics.psychometrics.{suffix}");
        let mut ports = vec![
            bounded_user_data_input(
                "items",
                "Scored item",
                series_type()?,
                if suffix == "content_validity" { 1 } else { 2 },
                None,
            )?,
            data_output("result", "Scale summary", report_type()?)?,
        ];
        let mut fields = vec![
            "item",
            "mean",
            "standard_deviation",
            "corrected_item_total_correlation",
            "alpha_if_deleted",
        ];
        if suffix == "content_validity" {
            fields = vec![
                "item",
                "relevant_experts",
                "item_cvi",
                "chance_agreement",
                "modified_kappa",
            ]
        }
        if suffix == "item_analysis" {
            fields.extend([
                "low_mean",
                "high_mean",
                "t_statistic",
                "degrees_of_freedom",
                "p_value",
            ])
        }
        if suffix != "validity" {
            ports.push(fixed_numeric_table(
                "item_statistics",
                "Item statistics",
                &fields,
            )?)
        }
        if suffix == "item_analysis" {
            ports.push(fixed_numeric_table(
                "scores",
                "Total scores and groups",
                &["observation", "total", "group"],
            )?)
        }
        let parameters = if suffix == "item_analysis" {
            let mut p = decimal_parameter("tail_fraction", "0.27")?;
            p.title_key = node_key(&id, "parameters.tail_fraction.title")?;
            p.description_key = Some(node_key(&id, "parameters.tail_fraction.description")?);
            for (locale, title, help) in [
                (
                    "en-US",
                    "Tail fraction",
                    "Between 0 and 0.5; default 0.27. Boundary ties stay together; coincident cutoffs remain in the middle.",
                ),
                (
                    "zh-CN",
                    "高低分组分位比例",
                    "大于 0 且小于 0.5，默认 0.27；并列边界总分保留在同组，两个分界相等时边界留在中间组。",
                ),
            ] {
                fragment.messages.extend([
                    (locale, p.title_key.as_str().to_string(), Text(title)),
                    (
                        locale,
                        p.description_key.as_ref().unwrap().as_str().to_string(),
                        Text(help),
                    ),
                ]);
            }
            vec![p]
        } else {
            vec![]
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    documentation_key: Some(node_key(&id, "documentation")?),
                    aliases_key: None,
                    category_id: sid("statistics.psychometrics", NodeCategoryId::new)?,
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
