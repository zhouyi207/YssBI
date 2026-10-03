//! A distribution overview composes the existing histogram, ECDF and boxplot contracts.
use super::*;
const ID: &str = "yssbi.statistics.plot.statistical.family";
pub(super) fn implemented(id: &str) -> bool {
    id == ID
}
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    let mut bins = bounded_integer_parameter(
        "bins",
        0,
        0,
        yss_sci_contract::visualization::MAX_PLOT_BINS as i64,
    )?;
    bins.title_key = node_key(ID, "parameters.bins.title")?;
    bins.description_key = Some(node_key(ID, "parameters.bins.description")?);
    for (locale, title, help) in [
        (
            "en-US",
            "Histogram bins",
            "0 selects Sturges' rule; otherwise choose 1–128 display bins. All observations contribute to counts.",
        ),
        (
            "zh-CN",
            "直方图分箱数",
            "0 使用 Sturges 规则自动分箱；也可指定 1–128 个显示区间。全部观测参与计数。",
        ),
    ] {
        fragment.messages.extend([
            (locale, bins.title_key.as_str().to_string(), Text(title)),
            (
                locale,
                bins.description_key.as_ref().unwrap().as_str().to_string(),
                Text(help),
            ),
        ]);
    }
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(ID, "title")?,
                documentation_key: Some(node_key(ID, "documentation")?),
                aliases_key: None,
                category_id: sid("statistics.design_quality", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                ID,
                vec![
                    data_input("values", "Values", series_type()?)?,
                    data_output("result", "Histogram", concrete("plot.data")?)?,
                    data_output(
                        "ecdf",
                        "Empirical cumulative distribution",
                        concrete("plot.data")?,
                    )?,
                    data_output("boxplot", "Boxplot", concrete("plot.data")?)?,
                ],
                vec![],
                vec![],
            )?,
            parameters: assembled_parameters(ID, vec![bins])?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        ID,
    ));
    for (locale, title) in [
        ("en-US", "Statistical plots (distribution overview)"),
        ("zh-CN", "统计图（分布概览）"),
    ] {
        fragment.messages.extend([
            (locale, node_key_text(ID, "title"), Text(title)),
            (locale, node_key_text(ID, "documentation"), Text(title)),
        ]);
    }
    Ok(())
}
