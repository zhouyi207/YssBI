//! Built-in node definitions, creation descriptors, and node catalog localization.

#![deny(unused_must_use)]

mod builtin;
mod core_nodes;
mod dataframe;
mod distribution;
mod documentation;
mod localization;
mod plot;
mod project;
mod statistics;

pub(crate) const fn data_connections(
    direction: yss_node_protocol::PortDirection,
) -> yss_node_protocol::ConnectionsPerPort {
    use yss_node_protocol::{ConnectionsPerPort, PortDirection};
    match direction {
        PortDirection::Input => ConnectionsPerPort::Single,
        PortDirection::Output => ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
    }
}

pub use builtin::{
    BuiltinAssemblyError, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
    register_builtin_nodes,
};
pub(crate) const REROUTE_NODE_TYPE: &str = "yssbi.core.reroute";
pub(crate) const REROUTE_INPUT_PORT: &str = "input";
pub(crate) const REROUTE_OUTPUT_PORT: &str = "output";
pub use core_nodes::reroute::validate_reroute_protocol_contract;
pub use dataframe::{
    DATAFRAME_COLUMNS_RESOLVER, DATAFRAME_PANEL_SCHEMA_RESOLVER, DATAFRAME_RESOURCE_SCHEMA_RESOLVER,
};
pub use project::{
    FUNCTION_CALL_ARGUMENTS_RESOLVER, FUNCTION_CALL_RESULTS_RESOLVER,
    FUNCTION_ENTRY_PARAMETERS_RESOLVER, FUNCTION_RETURN_RESULTS_RESOLVER,
};
pub fn reroute_node_type() -> yss_node_protocol::NodeTypeId {
    yss_node_protocol::NodeTypeId::new(REROUTE_NODE_TYPE)
        .expect("built-in reroute identifier is valid")
}

pub(crate) use localization::{Aliases, Message, Text};
pub use localization::{
    BuiltinCatalog, CatalogResourceEntry, CatalogResourcePath, I18nBundleValidationError,
    LocalizedCatalog, LocalizedCatalogItem, LocalizedCategory, LocalizedParameter, LocalizedPort,
    NodeCreation, ResourceBoundCreateArgs, authoritative_static_descriptor,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::build_builtin_node_system;
    use yss_node_protocol::{NodeTypeId, PortCardinality};

    #[test]
    fn statistical_navigation_preserves_hierarchy_and_separates_postestimation() {
        let system = build_builtin_node_system().unwrap();
        let catalog = system.catalog.localize(&system.registry, "en-US");
        let categories: Vec<_> = catalog
            .categories
            .iter()
            .filter(|category| category.parent_category_id.as_deref() == Some("statistics"))
            .collect();
        assert_eq!(categories.len(), 22);
        let orders: BTreeSet<_> = categories.iter().map(|category| category.order).collect();
        assert_eq!(orders.len(), categories.len());
        for (node, expected) in [
            ("linear.fit", "regression"),
            ("linear.summary", "regression"),
            ("linear.predict", "postestimation"),
            ("logit.predict", "postestimation"),
            ("probit.predict", "postestimation"),
            ("iv.2sls.fit", "causal"),
            ("iv.liml.summary", "causal"),
            ("panel.did.twfe", "causal"),
            ("panel.fit", "panel"),
            ("adf.test", "timeseries"),
            ("vec.rank_test", "timeseries"),
        ] {
            let id = NodeTypeId::new(format!("yssbi.statistics.{node}")).unwrap();
            let protocol = system.registry.protocol(&id).unwrap();
            assert_eq!(
                protocol.catalog.category_id.as_str(),
                format!("statistics.{expected}"),
                "{node}"
            );
        }
    }

    #[test]
    fn statistical_summaries_consume_their_fit_model_without_estimation_parameters() {
        use yss_node_protocol::PortDirection;
        let system = build_builtin_node_system().unwrap();
        let mut count = 0;
        for (id, _) in system.registry.iter().filter(|(id, _)| {
            id.as_str().starts_with("yssbi.statistics.") && id.as_str().ends_with(".summary")
        }) {
            let summary = system.registry.protocol(id).unwrap();
            let inputs: Vec<_> = summary
                .interface
                .ports
                .iter()
                .filter(|port| port.direction == PortDirection::Input)
                .collect();
            assert_eq!(inputs.len(), 1, "{id}");
            assert_eq!(inputs[0].key.as_str(), "model", "{id}");
            assert!(summary.parameters.parameters.is_empty(), "{id}");
            let fit_id = NodeTypeId::new(format!(
                "{}.fit",
                id.as_str().strip_suffix(".summary").unwrap()
            ))
            .unwrap();
            let fit = system.registry.protocol(&fit_id).unwrap();
            let model = fit
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == "model")
                .unwrap();
            assert_eq!(model.direction, PortDirection::Output);
            assert_eq!(inputs[0].value_type, model.value_type, "{id}");
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn adf_test_consumes_series_and_emits_result_and_report() {
        use yss_node_protocol::{PortDirection, TypeExpr};
        let system = build_builtin_node_system().unwrap();
        let adf = system
            .registry
            .protocol(&NodeTypeId::new("yssbi.statistics.adf.test").unwrap())
            .unwrap();
        let ports: Vec<_> = adf
            .interface
            .ports
            .iter()
            .map(|port| (port.key.as_str(), port.direction))
            .collect();
        assert_eq!(
            ports,
            [
                ("series", PortDirection::Input),
                ("result", PortDirection::Output),
                ("report", PortDirection::Output)
            ]
        );
        assert_eq!(
            adf.interface.ports[1].value_type,
            TypeExpr::Concrete("statistics.result.adf".parse().unwrap())
        );
        assert_eq!(
            adf.interface.ports[2].value_type,
            TypeExpr::Concrete("statistics.report".parse().unwrap())
        );
        assert!(
            system
                .registry
                .protocol(&NodeTypeId::new("yssbi.statistics.adf.summary").unwrap())
                .is_none()
        );
    }

    #[test]
    fn builtin_output_pins_support_unbounded_fan_out() {
        use yss_node_protocol::{ConnectionsPerPort, PortDirection};
        let system = build_builtin_node_system().unwrap();
        let mut outputs = 0;
        for (node_type, _) in system.registry.iter() {
            let protocol = system.registry.protocol(node_type).unwrap();
            for port in &protocol.interface.ports {
                if port.direction == PortDirection::Output {
                    outputs += 1;
                    assert_eq!(
                        port.connections,
                        ConnectionsPerPort::Multiple {
                            max: None,
                            ordered: false
                        },
                        "{node_type}:{}",
                        port.key
                    );
                }
            }
        }
        assert!(outputs > 0);
        let subtract = system
            .registry
            .protocol(&NodeTypeId::new("yssbi.numeric.subtract").unwrap())
            .unwrap();
        assert!(
            subtract
                .interface
                .ports
                .iter()
                .filter(|port| port.direction == PortDirection::Input)
                .all(|port| port.connections == ConnectionsPerPort::Single)
        );
    }

    #[test]
    fn mathematical_constants_are_static_localized_numeric_sources() {
        let system = build_builtin_node_system().unwrap();
        for locale in ["zh-CN", "en-US"] {
            let catalog = system.catalog.localize(&system.registry, locale);
            for id in ["yssbi.constant.pi", "yssbi.constant.e"] {
                let item = catalog
                    .items
                    .iter()
                    .find(|item| item.node_type_id.as_ref() == id)
                    .unwrap();
                assert_eq!(item.category_id.as_ref(), "constants");
                assert!(!item.title.is_empty());
                assert!(item.documentation.is_some());
                assert!(item.parameters.is_empty());
                let protocol = system
                    .registry
                    .protocol(&NodeTypeId::new(id).unwrap())
                    .unwrap();
                assert_eq!(protocol.interface.ports.len(), 1);
                let port = &protocol.interface.ports[0];
                assert_eq!(port.direction, yss_node_protocol::PortDirection::Output);
                assert_eq!(
                    port.value_type,
                    yss_node_protocol::TypeExpr::Concrete("core.numeric".parse().unwrap())
                );
            }
        }
    }

    #[test]
    fn numeric_type_class_contains_the_numeric_semantic() {
        let registry = build_builtin_node_system()
            .expect("production built-in registry must assemble")
            .registry;
        let members = registry
            .types()
            .iter()
            .filter(|(_, registration)| {
                registration
                    .classes
                    .iter()
                    .any(|class| class.as_str() == "core.numeric")
            })
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(members, ["core.numeric"]);
    }

    #[test]
    fn statistics_configuration_has_no_catalog_nodes_or_data_ports() {
        let system = build_builtin_node_system().unwrap();
        for (node_type, _) in system
            .registry
            .iter()
            .filter(|(node_type, _)| node_type.as_str().starts_with("yssbi.statistics."))
        {
            assert!(!node_type.as_str().ends_with(".configure"));
            assert!(!node_type.as_str().contains(".vce."));
            let protocol = system.registry.protocol(node_type).unwrap();
            assert!(
                protocol
                    .interface
                    .ports
                    .iter()
                    .all(|port| !matches!(port.key.as_str(), "configuration" | "covariance")),
                "{node_type}"
            );
        }
    }

    #[test]
    fn analysis_catalog_excludes_retired_flow_and_split_numeric_nodes() {
        let registry = build_builtin_node_system()
            .expect("production built-ins must assemble")
            .registry;
        let registered = registry
            .iter()
            .map(|(node_type, _)| node_type.as_str())
            .collect::<BTreeSet<_>>();

        for current in [
            "yssbi.constant.get",
            "yssbi.core.reroute",
            "yssbi.numeric.add",
            "yssbi.numeric.subtract",
            "yssbi.numeric.multiply",
            "yssbi.numeric.divide",
        ] {
            assert!(registered.contains(current), "current node '{current}'");
        }
        for removed in [
            "yssbi.project.event.begin",
            "yssbi.project.variable.set",
            "yssbi.debug.print",
            "yssbi.control.branch",
            "yssbi.control.sequence",
            "yssbi.control.loop",
            "yssbi.control.do",
            "yssbi.control.merge",
            "yssbi.control.sleep",
            "yssbi.reroute.control",
            "yssbi.reroute.effect",
            "yssbi.reroute.data",
            "yssbi.numeric.add.int64",
            "yssbi.numeric.add.float64",
            "yssbi.numeric.series.add",
            "yssbi.numeric.subtract.int64",
            "yssbi.numeric.subtract.float64",
            "yssbi.numeric.series.subtract",
            "yssbi.numeric.multiply.int64",
            "yssbi.numeric.multiply.float64",
            "yssbi.numeric.series.multiply",
            "yssbi.numeric.divide.int64",
            "yssbi.numeric.divide.float64",
            "yssbi.numeric.series.divide",
        ] {
            assert!(!registered.contains(removed), "retired node '{removed}'");
        }

        let add = registry
            .protocol(&NodeTypeId::new("yssbi.numeric.add").unwrap())
            .expect("unified Add protocol exists");
        assert!(matches!(
            add.interface.ports[0].cardinality,
            PortCardinality::UserCreated { min: 2, max: None }
        ));
    }
}
