//! Executable variance-analysis nodes retain their inventory identities.
use super::*;

const METHODS: &[&str] = &[
    "one_way",
    "two_way",
    "three_way",
    "factorial",
    "ancova",
    "manova",
    "repeated_measures",
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.anova.{method}");
        let mut ports = if method == "manova" {
            vec![bounded_user_data_input(
                "y",
                "Y",
                numeric_data_series_type(),
                2,
                None,
            )?]
        } else {
            vec![data_input("y", "Y", numeric_data_series_type())?]
        };
        if method == "repeated_measures" {
            ports.push(data_input("subjects", "Subject", label_series()?)?);
        }
        let (min, max) = match method {
            "one_way" => (1, Some(1)),
            "two_way" => (2, Some(2)),
            "three_way" => (3, Some(3)),
            _ => (1, None),
        };
        ports.push(bounded_user_data_input(
            "factors",
            "Factor",
            label_series()?,
            min as u16,
            max,
        )?);
        if method == "ancova" {
            ports.push(bounded_user_data_input(
                "covariates",
                "Covariate",
                numeric_data_series_type(),
                1,
                None,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        let parameters = match method {
            "one_way" => vec![],
            "repeated_measures" => vec![choice_parameter(
                "sphericity_correction",
                "greenhouse_geisser",
                &["none", "greenhouse_geisser"],
            )?],
            _ => vec![
                choice_parameter(
                    "model_terms",
                    "full_factorial",
                    &["main_effects", "full_factorial"],
                )?,
                choice_parameter(
                    "sums_of_squares",
                    "type_iii",
                    &["type_i", "type_ii", "type_iii"],
                )?,
            ],
        };
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.anova", NodeCategoryId::new)?,
                    icon_id: sid("builtin.statistics", IconId::new)?,
                    style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                    hidden: false,
                },
                interface: assembled_interface(&id, ports, vec![], vec![])?,
                parameters: assembled_parameters(&id, parameters)?,
                instance_display: NodeInstanceDisplaySpec::Static,
                execution: pure(),
                typing: NodeTypingSpec::Fixed,
                scope: NodeScope::Any,
                managed_role: None,
            },
            &id,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anova_catalog_registers_complete_interfaces_defaults_and_bilingual_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &method in METHODS {
                let id = format!("yssbi.statistics.anova.{method}");
                let protocol = system
                    .registry
                    .protocol(&id.as_str().parse().unwrap())
                    .unwrap();
                assert_eq!(
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|port| port.direction == PortDirection::Output)
                        .count(),
                    1
                );
                assert_eq!(
                    protocol.interface.ports.last().unwrap().key.as_str(),
                    "result"
                );
                let item = catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.anova");
                let help = item.documentation.as_deref().unwrap();
                assert!(help.contains("$$"), "{id} {locale}");
                assert!(!help.contains("范围待确认"));
                for (key, expected) in if method == "one_way" {
                    vec![]
                } else if method == "repeated_measures" {
                    vec![("sphericity_correction", "greenhouse_geisser")]
                } else {
                    vec![
                        ("model_terms", "full_factorial"),
                        ("sums_of_squares", "type_iii"),
                    ]
                } {
                    let field = protocol
                        .parameters
                        .iter()
                        .find(|p| p.key.as_str() == key)
                        .unwrap();
                    assert_eq!(
                        field.default_value.as_ref().map(|default| &default.value),
                        Some(&DataValue::String(expected.into()))
                    );
                }
            }
        }
    }
}
