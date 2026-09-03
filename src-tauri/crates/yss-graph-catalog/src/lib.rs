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
mod structured_control;

pub use builtin::{
    BuiltinAssemblyError, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
#[cfg(any(test, feature = "test-support"))]
pub use builtin::{builtin_bundle_parts_for_test, validate_builtin_bundle_for_test};
pub(crate) const DATA_REROUTE_NODE_TYPE: &str = "yssbi.reroute.data";
pub(crate) const CONTROL_REROUTE_NODE_TYPE: &str = "yssbi.reroute.control";
pub(crate) const EFFECT_REROUTE_NODE_TYPE: &str = "yssbi.reroute.effect";
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
pub fn reroute_node_type_for_kind(
    kind: yss_graph_protocol::PortKind,
) -> yss_graph_protocol::NodeTypeId {
    yss_graph_protocol::NodeTypeId::new(core_nodes::reroute::node_type_for_kind(kind))
        .expect("built-in reroute protocol identifiers are valid")
}

pub(crate) use localization::{Aliases, Message, Text};
pub use localization::{
    BuiltinCatalog, CatalogResourceEntry, CatalogResourcePath, I18nBundleValidationError,
    LocalizedCatalog, LocalizedCatalogItem, LocalizedCategory, LocalizedParameter, LocalizedPort,
    NodeCreation, ResourceBoundCreateArgs, authoritative_static_descriptor,
};

#[cfg(test)]
mod tests {
    use super::build_builtin_node_system;

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
}
