use super::*;
const RESOLVER: &str = "yssbi.statistics.doe.schema.design";
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(RESOLVER, SchemaResolverId::new)?);
    for (id, uniform) in [
        ("yssbi.statistics.doe.family", false),
        ("yssbi.statistics.doe.orthogonal", false),
        ("yssbi.statistics.doe.uniform_design", true),
    ] {
        let mut parameters = vec![positive_integer_parameter("factors", 3)?];
        if uniform {
            parameters.extend([
                positive_integer_parameter("runs", 12)?,
                positive_integer_parameter("candidates", 32)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]);
        } else {
            parameters.push(minimum_integer_parameter("levels", 2, 2)?);
        }
        node_scoped_keys(id, &mut parameters)?;
        let mut design = data_output("design", "Coded design", concrete("tabular.dataframe")?)?;
        design.schema = Some(SchemaExpr::Derived {
            resolver: sid(RESOLVER, SchemaResolverId::new)?,
            dependencies: vec![SchemaDependency::Parameter(sid(
                "factors",
                ParameterKey::new,
            )?)],
        });
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
                        data_output("result", "Design summary", report_type()?)?,
                        design,
                    ],
                    vec![],
                    vec![],
                )?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use yss_node_protocol::{ParameterIssueKind, ParameterValues, validate_parameter_values};

    #[test]
    fn factorial_and_orthogonal_parameters_reject_single_level_factors() {
        let system = crate::build_builtin_node_system().unwrap();
        for id in [
            "yssbi.statistics.doe.family",
            "yssbi.statistics.doe.orthogonal",
        ] {
            let protocol = system.registry.protocol(&id.parse().unwrap()).unwrap();
            let mut values = ParameterValues::new();
            let levels = ParameterKey::new("levels").unwrap();
            values.insert(levels.clone(), 1.into());
            let issues = validate_parameter_values(protocol, &values, system.registry.as_ref());
            assert!(
                issues.iter().any(|issue| issue.key.as_str() == "levels"
                    && matches!(issue.kind, ParameterIssueKind::Constraint)),
                "{id} must reject the single-level design that SCI cannot execute"
            );

            values.insert(levels, 2.into());
            assert!(
                validate_parameter_values(protocol, &values, system.registry.as_ref()).is_empty()
            );
        }
    }
}
