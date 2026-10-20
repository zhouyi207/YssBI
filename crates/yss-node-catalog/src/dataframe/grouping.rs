use super::*;
use std::sync::Arc;
use yss_node_registry::StructuralNodeRole;

const NODES: &[(&str, Option<StructuralNodeRole>)] = &[
    ("yssbi.dataframe.groupby.groups", None),
    (
        "yssbi.dataframe.groupby.apply",
        Some(StructuralNodeRole::GroupApply),
    ),
    (
        "yssbi.dataframe.groupby.transform",
        Some(StructuralNodeRole::GroupTransform),
    ),
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment.types.push(TypeRegistration {
        id: sid(yss_data_contract::GROUPED_DATAFRAME_TYPE_ID, TypeId::new)?,
        title_key: iid("types.grouped_dataframe.title")?,
        classes: Default::default(),
    });
    for &(id, role) in NODES {
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
                    data_input("groups", "Groups", group_type)?,
                    data_output("result", "Result", dataframe_type()?, None)?,
                ],
                parameters,
            )
        } else {
            (
                vec![
                    data_input("source", "Source", dataframe_type()?)?,
                    data_output(
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
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(match role {
            Some(role) => RegisteredNode::structural(Arc::new(protocol), role),
            None => leaf(protocol, id),
        });
    }
    Ok(())
}
