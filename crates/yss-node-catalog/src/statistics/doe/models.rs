use super::*;
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for (id, surface) in [
        ("yssbi.statistics.doe.response_surface", true),
        ("yssbi.statistics.doe.dose_response", false),
    ] {
        let mut ports = vec![data_input("y", "Y", numeric_data_series_type())?];
        ports.push(if surface {
            bounded_user_data_input(
                "factors",
                "Continuous factor",
                numeric_data_series_type(),
                1,
                None,
            )?
        } else {
            data_input("dose", "Nonnegative dose", numeric_data_series_type())?
        });
        ports.extend([
            data_output("result", "Fit and interpretation", report_type()?)?,
            fixed_numeric_table(
                "observations",
                "Observed and fitted responses",
                &["observation", "response", "fitted", "residual"],
            )?,
        ]);
        let mut parameters = if surface {
            vec![]
        } else {
            vec![
                positive_integer_parameter("max_iterations", 500)?,
                decimal_parameter("tolerance", "0.0000001")?,
            ]
        };
        node_scoped_keys(id, &mut parameters)?;
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
                interface: assembled_interface(id, ports, vec![], vec![])?,
                parameters: assembled_parameters(id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            id,
        ));
    }
    Ok(())
}
