//! Executable multivariate analyses and parameter-derived coordinate tables.
use super::*;

const RESOLVER: &str = "yssbi.statistics.multivariate.schema.coordinates";
const SPECS: &[&str] = &[
    "yssbi.statistics.association.canonical",
    "yssbi.statistics.multivariate.exploratory_factor",
    "yssbi.statistics.multivariate.pca",
    "yssbi.statistics.multivariate.correspondence",
    "yssbi.statistics.multivariate.discriminant",
    "yssbi.statistics.multivariate.rda",
    "yssbi.statistics.multivariate.mds",
];
fn numeric_group(
    key: &'static str,
    label: &'static str,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    bounded_user_data_input(key, label, numeric_data_series_type(), min, None)
}
fn coordinate_output(
    key: &'static str,
    label: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut output = data_output(key, label, concrete("tabular.dataframe")?)?;
    output.schema = Some(SchemaExpr::Derived {
        resolver: sid(RESOLVER, SchemaResolverId::new)?,
        dependencies: vec![SchemaDependency::Parameter(sid(
            "components",
            ParameterKey::new,
        )?)],
    });
    Ok(output)
}

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(RESOLVER, SchemaResolverId::new)?);
    for &id in SPECS {
        let method = id.rsplit_once('.').map_or(id, |(_, method)| method);
        let label = TypeExpr::Generic(sid("class_label", TypeParameterId::new)?);
        let mut ports = match method {
            "canonical" => vec![
                numeric_group("x", "X variable", 1)?,
                numeric_group("y", "Y variable", 1)?,
            ],
            "correspondence" => vec![numeric_group("columns", "Count column", 2)?],
            "discriminant" => vec![
                data_input("groups", "Class", data_series_type(label.clone()))?,
                numeric_group("variables", "Training variable", 1)?,
                numeric_group("new_variables", "New variable", 0)?,
            ],
            "rda" => vec![
                numeric_group("y", "Y", 1)?,
                numeric_group("constraints", "Constraint", 1)?,
            ],
            "mds" => vec![numeric_group("variables", "Variable / distance column", 1)?],
            _ => vec![numeric_group(
                "variables",
                "Variable",
                if method == "exploratory_factor" { 3 } else { 2 },
            )?],
        };
        ports.push(data_output("result", "Result", report_type()?)?);
        match method {
            "correspondence" => {
                ports.push(coordinate_output("row_coordinates", "Row coordinates")?);
                ports.push(coordinate_output(
                    "column_coordinates",
                    "Column coordinates",
                )?);
            }
            "discriminant" => ports.push(data_output(
                "predictions",
                "Predicted class",
                data_series_type(label),
            )?),
            "mds" => ports.push(coordinate_output("coordinates", "Coordinates")?),
            _ => ports.push(coordinate_output("scores", "Scores")?),
        }
        let default_components = if matches!(
            method,
            "canonical" | "exploratory_factor" | "correspondence" | "rda"
        ) {
            1
        } else {
            2
        };
        let mut parameters = if method == "discriminant" {
            vec![
                choice_parameter("discriminant_method", "linear", &["linear", "quadratic"])?,
                choice_parameter("class_priors", "empirical", &["empirical", "equal"])?,
                decimal_parameter("shrinkage", "0")?,
            ]
        } else {
            vec![positive_integer_parameter(
                "components",
                default_components,
            )?]
        };
        match method {
            "pca" => parameters.push(toggle_parameter("standardize", true)?),
            "exploratory_factor" => parameters.extend([
                choice_parameter("rotation", "varimax", &["none", "varimax"])?,
                positive_integer_parameter("max_iterations", 500)?,
                decimal_parameter("tolerance", "0.000001")?,
            ]),
            "rda" => parameters.extend([
                toggle_parameter("standardize", false)?,
                bounded_integer_parameter("permutations", 199, 0, 9999)?,
                nonnegative_integer_parameter("seed", 42)?,
            ]),
            "mds" => {
                parameters.push(choice_parameter(
                    "input_kind",
                    "observations",
                    &["observations", "dissimilarity_matrix"],
                )?);
                parameters.push(
                    toggle_parameter("standardize", false)?
                        .when(choice_condition("input_kind", "observations")?),
                );
            }
            _ => {}
        }
        // Per-node text avoids changing shared regression parameter descriptions.
        node_scoped_keys(id, &mut parameters)?;
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id, NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(id, "title")?,
                    aliases_key: Some(node_key(id, "aliases")?),
                    category_id: sid("statistics.multivariate", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(
                    id,
                    ports,
                    if method == "discriminant" {
                        vec![sid("class_label", TypeParameterId::new)?]
                    } else {
                        vec![]
                    },
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
    #[test]
    fn multivariate_catalog_has_registered_score_schemas_and_bilingual_method_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &id in SPECS {
                let protocol = system.registry.protocol(&id.parse().unwrap()).unwrap();
                let item = catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.multivariate");
                assert!(
                    item.documentation.as_deref().unwrap().contains("$$"),
                    "{id} {locale}"
                );
                let outputs = protocol
                    .interface
                    .ports
                    .iter()
                    .filter(|port| port.direction == PortDirection::Output)
                    .collect::<Vec<_>>();
                assert_eq!(outputs[0].key.as_str(), "result");
                if id.ends_with("discriminant") {
                    assert_eq!(outputs[1].key.as_str(), "predictions");
                    assert_eq!(protocol.interface.type_parameters.len(), 1);
                } else {
                    for output in &outputs[1..] {
                        assert!(
                            matches!(output.schema.as_ref(),Some(SchemaExpr::Derived{resolver,..})if resolver.as_str()==RESOLVER)
                        );
                    }
                }
            }
        }
    }
}
