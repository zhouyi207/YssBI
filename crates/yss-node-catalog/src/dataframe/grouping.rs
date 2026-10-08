use super::*;
use std::sync::Arc;
use yss_node_registry::StructuralNodeRole;

const NODES: &[(&str, &str, &str, Option<StructuralNodeRole>)] = &[
    (
        "yssbi.dataframe.groupby.groups",
        "GroupBy",
        "分组 GroupBy",
        None,
    ),
    (
        "yssbi.dataframe.groupby.apply",
        "Apply to Groups",
        "按组调用 Apply",
        Some(StructuralNodeRole::GroupApply),
    ),
    (
        "yssbi.dataframe.groupby.transform",
        "Transform Groups",
        "按组变换 Transform",
        Some(StructuralNodeRole::GroupTransform),
    ),
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment.types.push(TypeRegistration {
        id: sid(yss_data_contract::GROUPED_DATAFRAME_TYPE_ID, TypeId::new)?,
        title_key: iid("types.grouped_dataframe.title")?,
        classes: Default::default(),
    });
    for (locale, title, prefix, help) in [
        (
            "en-US",
            "Grouped DataFrame",
            "Group Key Prefix",
            "Prefix for the group key columns prepended to Apply results. Collisions are rejected.",
        ),
        (
            "zh-CN",
            "已分组数据帧",
            "分组键前缀",
            "Apply 在结果前追加分组键列，使用此前缀；名称冲突时明确报错。",
        ),
    ] {
        fragment
            .messages
            .push((locale, "types.grouped_dataframe.title".into(), Text(title)));
        fragment
            .messages
            .push((locale, "parameters.key_prefix.title".into(), Text(prefix)));
        fragment.messages.push((
            locale,
            "parameters.key_prefix.description".into(),
            Text(help),
        ));
    }
    for &(id, en, zh, role) in NODES {
        let group_type = concrete(yss_data_contract::GROUPED_DATAFRAME_TYPE_ID)?;
        let (ports, parameters) = if let Some(role) = role {
            let mut parameters = vec![crate::project::resource_parameter("target")?];
            if role == StructuralNodeRole::GroupApply {
                let ty = concrete("core.text")?;
                parameters.push(parameter(
                    "key_prefix",
                    ty.clone(),
                    ParameterEditorSpec::Text { multiline: false },
                    Some(TypedValue {
                        value_type: ty,
                        value: DataValue::String("group.".into()),
                    }),
                    vec![],
                )?);
            }
            (
                vec![
                    streaming_input("groups", "Groups", group_type)?,
                    streaming_output("result", "Result", dataframe_type()?, None)?,
                ],
                parameters,
            )
        } else {
            (
                vec![
                    streaming_input("source", "Source", dataframe_type()?)?,
                    streaming_output(
                        "groups",
                        "Groups",
                        group_type,
                        Some(SchemaExpr::Input(port_key("source")?)),
                    )?,
                ],
                vec![nominal_parameter(
                    "keys",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?],
            )
        };
        let protocol = NodeProtocol {
            type_id: sid(id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(id, "title")?,
                documentation_key: Some(node_key(id, "documentation")?),
                aliases_key: Some(node_key(id, "aliases")?),
                category_id: sid("dataframe", NodeCategoryId::new)?,
                icon_id: sid("builtin.dataframe", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(id, ports, vec![], vec![])?,
            parameters: assembled_parameters(id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: ExecutionSemantics {
                determinism: if role.is_some() {
                    Determinism::EnvironmentDependent
                } else {
                    Determinism::Deterministic
                },
                cache: CachePolicy::PerRun,
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(match role {
            Some(role) => RegisteredNode::structural(Arc::new(protocol), role),
            None => leaf(protocol, id),
        });
        for (locale, title) in [("en-US", en), ("zh-CN", zh)] {
            fragment
                .messages
                .push((locale, node_key_text(id, "title"), Text(title)));
            fragment.messages.push((
                locale,
                node_key_text(id, "aliases"),
                Aliases(&["groupby", "apply", "transform", "分组函数"]),
            ));
            fragment.messages.push((
                locale,
                node_key_text(id, "documentation"),
                Text(help(id, locale)),
            ));
        }
    }
    Ok(())
}

fn help(id: &str, locale: &str) -> &'static str {
    match (id, crate::documentation::is_chinese_locale(locale)) {
        ("yssbi.dataframe.groupby.groups", true) => include_str!("../docs/zh/groupby_groups.md"),
        ("yssbi.dataframe.groupby.groups", false) => include_str!("../docs/en/groupby_groups.md"),
        ("yssbi.dataframe.groupby.apply", true) => include_str!("../docs/zh/groupby_apply.md"),
        ("yssbi.dataframe.groupby.apply", false) => include_str!("../docs/en/groupby_apply.md"),
        (_, true) => include_str!("../docs/zh/groupby_transform.md"),
        (_, false) => include_str!("../docs/en/groupby_transform.md"),
    }
}

pub(super) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    NODES
        .iter()
        .any(|entry| entry.0 == id)
        .then(|| help(id, locale).into())
}
