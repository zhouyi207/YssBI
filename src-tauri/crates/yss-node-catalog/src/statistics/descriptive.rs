//! Executable descriptive statistics, retaining the inventory's method identities.
use super::*;

pub(super) const THEIL_ID: &str = "yssbi.statistics.inequality.theil";

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(THEIL_ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(THEIL_ID, "title")?,
                documentation_key: Some(node_key(THEIL_ID, "documentation")?),
                aliases_key: Some(node_key(THEIL_ID, "aliases")?),
                category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                THEIL_ID,
                vec![
                    data_input("series", "Values / group means", series_type()?)?,
                    bounded_user_data_input(
                        "weights",
                        "Weights (group population)",
                        series_type()?,
                        0,
                        Some(1),
                    )?,
                    data_output("result", "Result", report_type()?)?,
                ],
                vec![],
                vec![],
            )?,
            parameters: assembled_parameters(
                THEIL_ID,
                vec![choice_parameter(
                    "theil_form",
                    "individual",
                    &["individual", "grouped"],
                )?],
            )?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        THEIL_ID,
    ));
    for (locale, title, description, form, form_description) in [
        (
            "en-US",
            "Theil index",
            "Theil T for individual values or population-weighted group means, using natural logarithms.",
            "Data form",
            "Individual: equal weights. Grouped: supply group means and add one Weights input in Details for group populations or population shares. Group means measure between-group inequality only.",
        ),
        (
            "zh-CN",
            "泰尔指数",
            "计算个体数据或人口加权组均值的 Theil T 指数，使用自然对数。",
            "数据形式",
            "个体形式等权计算；分组形式输入组均值，并在详细面板添加一个 Weights 输入，连接组人数或人口占比。组均值仅反映组间差异。",
        ),
    ] {
        fragment.messages.extend([
            (locale, node_key_text(THEIL_ID, "title"), Text(title)),
            (
                locale,
                node_key_text(THEIL_ID, "documentation"),
                Text(description),
            ),
            (
                locale,
                node_key_text(THEIL_ID, "aliases"),
                Aliases(&["inequality.theil", "Theil T", "泰尔指数"]),
            ),
            (
                locale,
                "parameters.statistics.theil_form.title".into(),
                Text(form),
            ),
            (
                locale,
                "parameters.statistics.theil_form.description".into(),
                Text(form_description),
            ),
        ]);
    }
    Ok(())
}
