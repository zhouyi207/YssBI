//! Descriptive summaries and inequality statistics.
use super::*;

pub(super) const THEIL_ID: &str = "yssbi.statistics.inequality.theil";
pub(super) const GINI_ID: &str = "yssbi.statistics.inequality.gini";
pub(super) const DAGUM_ID: &str = "yssbi.statistics.inequality.dagum_gini";

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    append_description(fragment)?;
    for (id,) in [(GINI_ID,), (DAGUM_ID,)] {
        let mut ports = vec![data_input("series", "Values", numeric_data_series_type())?];
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
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, vec![])?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
    }
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(THEIL_ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(THEIL_ID, "title")?,
                aliases_key: Some(node_key(THEIL_ID, "aliases")?),
                category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                THEIL_ID,
                vec![
                    data_input("series", "Values / group means", numeric_data_series_type())?,
                    bounded_user_data_input(
                        "weights",
                        "Weights (group population)",
                        numeric_data_series_type(),
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
            execution: pure(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        THEIL_ID,
    ));
    Ok(())
}

fn append_description(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = crate::DESCRIBE_NODE_TYPE;
    let mut input_types = vec![concrete("tabular.dataframe")?];
    for kind in SemanticType::ALL {
        if yss_data_contract::aggregation::supports_description(kind) {
            input_types.push(data_series_type(concrete(kind.type_id())?));
        }
    }
    let source = data_input("source", "Data", TypeExpr::Union(input_types))?;
    let result = data_output("result", "Result", report_type()?)?;
    fragment.nodes.push(leaf(
        NodeProtocol {
            type_id: sid(ID, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(ID, "title")?,
                aliases_key: Some(node_key(ID, "aliases")?),
                category_id: sid("statistics.descriptive", NodeCategoryId::new)?,
                icon_id: sid("builtin.statistics", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(ID, vec![source, result], vec![], vec![])?,
            parameters: assembled_parameters(ID, vec![])?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: pure(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        },
        ID,
    ));
    Ok(())
}
