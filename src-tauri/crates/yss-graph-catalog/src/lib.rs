//! Graph-owned built-in catalog and localization contracts.

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

pub use builtin::{
    BuiltinAssemblyError, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
#[cfg(any(test, feature = "test-support"))]
pub use builtin::{builtin_bundle_parts_for_test, validate_builtin_bundle_for_test};
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
pub fn reroute_node_type() -> yss_graph_protocol::NodeTypeId {
    yss_graph_protocol::NodeTypeId::new(REROUTE_NODE_TYPE)
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
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        CatalogResourceEntry, CatalogResourcePath, ResourceBoundCreateArgs,
        build_builtin_node_system,
    };
    use yss_graph_protocol::{NodeTypeId, PortCardinality};

    #[test]
    fn numeric_type_class_contains_only_int64_and_float64() {
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

        assert_eq!(members, ["core.float64", "core.int64"]);
    }

    #[test]
    fn variable_resource_has_one_localized_read_action() {
        let system = build_builtin_node_system().expect("production built-ins must assemble");
        let resource_path = CatalogResourcePath::new("variables/score");
        let entries = [CatalogResourceEntry {
            name: "Score".into(),
            node_type_id: NodeTypeId::new("yssbi.project.variable.get")
                .expect("built-in node type is valid"),
            resource_path: resource_path.clone(),
            resource_revision: 3,
            create_args: ResourceBoundCreateArgs::Variable,
            technical_terms: vec!["variable".into()],
        }];

        let catalog = system
            .catalog
            .localize_with_resources(&system.registry, "zh-CN", &entries);
        let titles = catalog
            .items
            .iter()
            .filter(|item| item.resource_path.as_ref() == Some(&resource_path))
            .map(|item| (item.node_type_id.as_ref(), item.title.as_ref()))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(titles.len(), 1);
        assert_eq!(titles["yssbi.project.variable.get"], "读取变量 · Score");
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
