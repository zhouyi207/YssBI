//! Value-label authoring over the existing semantic-domain conversion contract.
use super::*;
use yss_data_contract::SemanticType;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.dataframe.labels";
    let choices = ["core.categorical", "core.ordinal"];
    let mut parameters = vec![
        choice_parameter("target_type", "core.categorical", &choices)?,
        parameter(
            "semantic_domain",
            concrete("core.object")?,
            ParameterEditorSpec::SemanticDomain,
            Some(TypedValue {
                value_type: concrete("core.object")?,
                value: DataValue::Object(Default::default()),
            }),
            vec![],
        )?,
    ];
    for parameter in &mut parameters {
        let (en, zh, eh, zh_help) = if parameter.key.as_str() == "target_type" {
            (
                "Value meaning",
                "取值语义",
                "Categorical or ordered levels. The original codes remain the values.",
                "选择分类或顺序语义；取值保留原始编码。",
            )
        } else {
            (
                "Codes and labels",
                "编码与标签",
                "Declare exact codes and their labels; order ordinal levels. Empty configuration inherits a compatible domain; categorical values without a domain receive same-name labels automatically.",
                "填写精确编码及含义；顺序等级从低到高排列。留空时继承兼容值域；分类没有值域时按实际取值自动生成同名标签。",
            )
        };
        parameter.title_key =
            node_key(ID, &format!("parameters.{}.title", parameter.key.as_str()))?;
        parameter.description_key = Some(node_key(
            ID,
            &format!("parameters.{}.description", parameter.key.as_str()),
        )?);
        for (locale, title, help) in [("en-US", en, eh), ("zh-CN", zh, zh_help)] {
            fragment.messages.extend([
                (
                    locale,
                    parameter.title_key.as_str().to_string(),
                    Text(title),
                ),
                (
                    locale,
                    parameter
                        .description_key
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .to_string(),
                    Text(help),
                ),
            ]);
        }
    }
    let series = |kind: &'static str| -> Result<TypeExpr, BuiltinAssemblyError> {
        Ok(data_series_type(concrete(kind)?))
    };
    let input_type = TypeExpr::Union(
        SemanticType::ALL
            .into_iter()
            .map(|kind| series(kind.type_id()))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let output_type = TypeExpr::Union(
        choices
            .into_iter()
            .map(series)
            .collect::<Result<Vec<_>, _>>()?,
    );
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(ID, "title")?,
                documentation_key: Some(node_key(ID, "documentation")?),
                aliases_key: Some(node_key(ID, "aliases")?),
                category_id: sid("dataframe.series", NodeCategoryId::new)?,
                icon_id: sid("builtin.dataframe", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                ID,
                vec![
                    streaming_input("input", "Input series", input_type)?,
                    streaming_output("output", "Labeled series", output_type, None)?,
                ],
                vec![],
                vec![],
            )?,
            parameters: assembled_parameters(ID, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: ExecutionSemantics {
                determinism: Determinism::Deterministic,
                cache: CachePolicy::PerRun,
            },
            typing: NodeTypingSpec::ShapePreservingConversion {
                input: sid("input", PortKey::new)?,
                target: ConversionTarget::Parameter(sid("target_type", ParameterKey::new)?),
                output: sid("output", PortKey::new)?,
            },
            scope: NodeScope::Any,
            managed_role: None,
        },
        ID,
    ));
    for (locale, title) in [
        ("en-US", "Data labels (value meanings)"),
        ("zh-CN", "数据标签（取值含义）"),
    ] {
        fragment.messages.extend([
            (locale, node_key_text(ID, "title"), Text(title)),
            (locale, node_key_text(ID, "documentation"), Text(title)),
            (
                locale,
                node_key_text(ID, "aliases"),
                Aliases(&["数据标签", "取值标签", "value labels", "data.labels"]),
            ),
        ]);
    }
    Ok(())
}
