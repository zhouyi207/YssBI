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

pub use builtin::{BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system};
#[cfg(test)]
pub(crate) use builtin::{
    builtin_bundle_parts_for_test, register_builtin_nominal_validators_for_test,
    validate_builtin_bundle_for_test,
};
pub use dataframe::DATAFRAME_RESOURCE_SCHEMA_RESOLVER;
pub(crate) use localization::authoritative_static_descriptor;
pub use localization::{
    BuiltinCatalog, BuiltinLocalizationBundle, CatalogResourceEntry, CatalogResourcePath,
    I18nBundleInventory, I18nBundleValidationError, LocalizedCatalog, LocalizedCatalogDto,
    LocalizedCatalogItemDto, LocalizedCategoryDto, LocalizedParameterDto, LocalizedPortDto,
    NodeCreationDescriptor, ResourceBoundCreateArgsDto, normalize_search_text,
};

#[cfg(test)]
mod tests;
