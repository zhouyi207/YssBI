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
    node_scoped_keys(ID, &mut parameters)?;
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
                aliases_key: Some(node_key(ID, "aliases")?),
                category_id: sid("dataframe.series", NodeCategoryId::new)?,
                icon_id: sid("builtin.dataframe", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                ID,
                vec![
                    data_input("input", "Input series", input_type)?,
                    data_output("output", "Labeled series", output_type, None)?,
                ],
                vec![],
                vec![],
            )?,
            parameters: assembled_parameters(ID, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: pure(),
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
    Ok(())
}
