//! Descriptive summaries and inequality statistics.
use super::*;

pub(super) const THEIL_ID: &str = "yssbi.statistics.inequality.theil";
pub(super) const GINI_ID: &str = "yssbi.statistics.inequality.gini";
pub(super) const DAGUM_ID: &str = "yssbi.statistics.inequality.dagum_gini";

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    append_description(fragment)?;
    for (id, en, zh, en_help, zh_help, aliases) in [
        (
            GINI_ID,
            "Gini coefficient",
            "Gini 基尼系数",
            "Empirical Gini for nonnegative observations with a positive mean. Returns one structured result; observations have equal weight.",
            "计算非负观测、正均值数据的经验基尼系数，个体等权，输出单个结构化结果。",
            &["inequality.gini", "Gini", "基尼系数"][..],
        ),
        (
            DAGUM_ID,
            "Dagum Gini decomposition",
            "Dagum 基尼系数",
            "Connect values and aligned group labels. Decomposes empirical Gini into within-group, net between-group and transvariation contributions, with group and pairwise details.",
            "连接数值及对齐的分组标签，将经验基尼系数分解为组内差异、组间净差异和超变密度，并输出分组与组对明细。",
            &["inequality.dagum_gini", "Dagum Gini", "基尼分解"][..],
        ),
    ] {
        let mut ports = vec![data_input("series", "Values", series_type()?)?];
        if id == DAGUM_ID {
            let members = [
                "core.numeric",
                "core.categorical",
                "core.ordinal",
                "core.binary",
            ]
            .into_iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?;
            ports.push(data_input(
                "groups",
                "Group labels",
                normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
                    BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                        context: "Dagum group labels",
                        value: error.to_string().into(),
                    }
                })?,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    documentation_key: Some(node_key(id, "documentation")?),
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, vec![])?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: execution(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
        for (locale, title, help) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
            fragment.messages.extend([
                (locale, node_key_text(id, "title"), Text(title)),
                (locale, node_key_text(id, "documentation"), Text(help)),
                (locale, node_key_text(id, "aliases"), Aliases(aliases)),
            ]);
        }
    }
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

fn append_description(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.statistics.describe";
    let mut input_types = vec![concrete("tabular.dataframe")?];
    for kind in SemanticType::ALL {
        if yss_data_contract::aggregation::supports_description(kind) {
            input_types.push(data_series_type(concrete(kind.type_id())?));
        }
    }
    let mut source = data_input("source", "Data", TypeExpr::Union(input_types))?;
    source.consumption = Some(InputConsumption::Streaming);
    let result = data_output("result", "Result", report_type()?)?;
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(ID, "title")?,
                documentation_key: Some(node_key(ID, "documentation")?),
                aliases_key: Some(node_key(ID, "aliases")?),
                category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(ID, vec![source, result], vec![], vec![])?,
            parameters: assembled_parameters(ID, vec![])?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: execution(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        ID,
    ));
    for (locale, title, help) in [
        (
            "en-US",
            "Data Description",
            "Returns a structured statistical result for each supported column of a DataFrame or DataSeries.",
        ),
        (
            "zh-CN",
            "数据描述",
            "对数据帧或数据序列逐列进行描述统计，输出结构化结果并在详细信息中展示。",
        ),
    ] {
        fragment.messages.extend([
            (locale, node_key_text(ID, "title"), Text(title)),
            (locale, node_key_text(ID, "documentation"), Text(help)),
            (
                locale,
                node_key_text(ID, "aliases"),
                Aliases(&["describe", "summary", "descriptive statistics", "描述统计"]),
            ),
        ]);
    }
    Ok(())
}
