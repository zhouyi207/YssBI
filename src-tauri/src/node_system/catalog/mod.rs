#![deny(unused_must_use)]

//! Trusted built-in node provider and localized catalog projections.

mod builtin;
mod control;
mod core_nodes;
mod dataframe;
mod distribution;
mod localization;
mod plot;
mod project;
mod statistics;

pub use builtin::{
    BuiltinAssemblyError, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
#[cfg(test)]
pub(crate) use builtin::{
    BuiltinAssemblyTestFault, build_builtin_node_system_with_test_fault,
    builtin_bundle_parts_for_test, register_builtin_nominal_validators_for_test,
    validate_builtin_bundle_for_test,
};
pub(crate) const DATA_REROUTE_NODE_TYPE: &str = "yssbi.reroute.data";
pub(crate) const CONTROL_REROUTE_NODE_TYPE: &str = "yssbi.reroute.control";
pub(crate) const EFFECT_REROUTE_NODE_TYPE: &str = "yssbi.reroute.effect";
pub(crate) const REROUTE_INPUT_PORT: &str = "input";
pub(crate) const REROUTE_OUTPUT_PORT: &str = "output";
pub use dataframe::DATAFRAME_RESOURCE_SCHEMA_RESOLVER;
pub(crate) use localization::authoritative_static_descriptor;
pub use localization::{
    BuiltinCatalog, BuiltinLocalizationBundle, CatalogResourceEntry, CatalogResourcePath,
    I18nBundleInventory, I18nBundleValidationError, LocalizedCatalog, LocalizedCatalogDto,
    LocalizedCatalogItemDto, LocalizedCategoryDto, LocalizedParameterDto, LocalizedPortDto,
    NodeCreationDescriptor, ResourceBoundCreateArgsDto, normalize_search_text,
};

pub(in crate::node_system) use core_nodes::reroute::validate_reroute_protocol_contract;

pub(in crate::node_system) fn reroute_node_type_for_kind(
    kind: crate::node_system::protocol::PortKind,
) -> crate::node_system::protocol::NodeTypeId {
    crate::node_system::protocol::NodeTypeId::new(core_nodes::reroute::node_type_for_kind(kind))
        .expect("built-in reroute node type IDs are valid")
}

#[cfg(test)]
mod tests;
