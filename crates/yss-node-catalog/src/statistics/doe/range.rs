use super::*;
use crate::builtin::add_node_messages;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    let id = "yssbi.statistics.doe.range_analysis";
    let mut maximize = toggle_parameter("maximize", true)?;
    maximize.title_key = node_key(id, "parameters.maximize.title")?;
    maximize.description_key = Some(node_key(id, "parameters.maximize.description")?);
    for (locale, title, help) in [
        (
            "en-US",
            "Larger response is preferred",
            "Turn off to select the lowest factor-level mean; all tied best levels are retained.",
        ),
        (
            "zh-CN",
            "响应越大越好",
            "关闭时选择均值最低的因素水平；保留全部并列最优水平。",
        ),
    ] {
        fragment.messages.extend([
            (locale, maximize.title_key.as_str().to_string(), Text(title)),
            (
                locale,
                maximize
                    .description_key
                    .as_ref()
                    .unwrap()
                    .as_str()
                    .to_string(),
                Text(help),
            ),
        ]);
    }
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(id, "title")?,
                aliases_key: None,
                category_id: sid("statistics.design_quality", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                id,
                vec![
                    data_input("y", "Y", numeric_data_series_type())?,
                    bounded_user_data_input(
                        "factors",
                        "Experimental factor",
                        label_series()?,
                        1,
                        None,
                    )?,
                    data_output("result", "Factor ranges", report_type()?)?,
                    fixed_numeric_table(
                        "levels",
                        "Factor-level summaries",
                        &["factor", "level", "observations", "total", "mean"],
                    )?,
                ],
                vec![],
                vec![],
            )?,
            parameters: assembled_parameters(id, vec![maximize])?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        id,
    ));
    add_node_messages(
        &mut fragment.messages,
        id,
        [
            "Range analysis (factor-level effects)",
            "极差分析（因素水平效应）",
        ],
        None,
    );
    Ok(())
}
