//! Executable longitudinal nodes preserve the ten inventory identities.
use super::*;

const METHODS: &[&str] = &[
    "longitudinal.gee",
    "mixed.hlm",
    "mixed.lmm",
    "mixed.glmm",
    "mixed.random_intercept",
    "mixed.random_slope",
    "mixed.crossed_effects",
    "mixed.logistic",
    "mixed.poisson",
    "mixed.negative_binomial",
];

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for &method in METHODS {
        let id = format!("yssbi.statistics.{method}");
        let gee = method == "longitudinal.gee";
        let generalized = matches!(
            method,
            "mixed.glmm" | "mixed.logistic" | "mixed.poisson" | "mixed.negative_binomial"
        );
        let mut ports = vec![
            data_input("y", "Y", numeric_or_binary_series()?)?,
            bounded_user_data_input("x", "X", numeric_data_series_type(), 0, None)?,
            bounded_user_data_input(
                "groups",
                "Grouping factor",
                label_series()?,
                if method == "mixed.crossed_effects" {
                    2
                } else {
                    1
                },
                if matches!(method, "mixed.hlm" | "mixed.crossed_effects") {
                    None
                } else {
                    Some(1)
                },
            )?,
        ];
        if matches!(method, "mixed.lmm" | "mixed.random_slope") {
            ports.push(bounded_user_data_input(
                "random_predictors",
                "Random slope",
                numeric_data_series_type(),
                if method == "mixed.random_slope" { 1 } else { 0 },
                None,
            )?);
        }
        ports.push(data_output("result", "Result", report_type()?)?);
        let mut parameters = vec![
            toggle_parameter("constant", true)?,
            positive_integer_parameter("max_iterations", 500)?,
            tolerance_parameter("0.0000001")?,
        ];
        if gee || method == "mixed.glmm" {
            parameters.push(choice_parameter(
                "longitudinal_family",
                if gee { "gaussian" } else { "binomial" },
                if gee {
                    &["gaussian", "binomial", "poisson"]
                } else {
                    &["binomial", "poisson", "negative_binomial"]
                },
            )?);
        }
        if gee {
            parameters.push(choice_parameter(
                "working_correlation",
                "exchangeable",
                &["independence", "exchangeable"],
            )?);
        } else if !generalized {
            parameters.push(choice_parameter(
                "mixed_estimation",
                "reml",
                &["reml", "ml"],
            )?);
        }
        fragment.nodes.push(leaf(
            NodeProtocol {
                type_id: sid(id.as_str(), NodeTypeId::new)?,
                catalog: NodeCatalogProtocol {
                    title_key: node_key(&id, "title")?,
                    aliases_key: Some(node_key(&id, "aliases")?),
                    category_id: sid("statistics.longitudinal", NodeCategoryId::new)?,
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
    fn longitudinal_catalog_retains_all_identities_and_complete_bilingual_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for locale in ["en-US", "zh-CN"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for &suffix in METHODS {
                let id = format!("yssbi.statistics.{suffix}");
                let protocol = system
                    .registry
                    .protocol(&id.as_str().parse().unwrap())
                    .unwrap();
                let item = catalog
                    .items
                    .iter()
                    .find(|i| i.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "statistics.longitudinal");
                let help = item.documentation.as_deref().unwrap();
                assert!(
                    help.contains("$$")
                        && help.contains("max_iterations")
                        && help.contains("group_labels"),
                    "{id}: {locale}"
                );
                assert!(!help.contains("范围待确认"));
                assert_eq!(
                    protocol
                        .interface
                        .ports
                        .iter()
                        .filter(|p| p.direction == PortDirection::Output)
                        .count(),
                    1
                );
                assert_eq!(
                    protocol.interface.ports.last().unwrap().key.as_str(),
                    "result"
                );
                let default = |key: &str| {
                    protocol
                        .parameters
                        .iter()
                        .find(|p| p.key.as_str() == key)
                        .unwrap()
                        .default_value
                        .as_ref()
                        .unwrap()
                        .value
                        .clone()
                };
                assert_eq!(default("constant"), DataValue::Bool(true));
                assert_eq!(default("max_iterations"), DataValue::Integer(500));
                if suffix == "longitudinal.gee" {
                    assert_eq!(
                        default("working_correlation"),
                        DataValue::String("exchangeable".into())
                    );
                }
            }
        }
    }
}
