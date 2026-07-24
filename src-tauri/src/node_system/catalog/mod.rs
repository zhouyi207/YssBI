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

pub use builtin::{build_builtin_provider, build_builtin_registry};
pub use dataframe::DATAFRAME_RESOURCE_SCHEMA_RESOLVER;
pub use localization::{
    BuiltinCatalog, BuiltinLocalizationBundle, CatalogResourceEntry, I18nBundleInventory,
    I18nBundleValidationError, LocalizedCatalogDto, LocalizedCatalogItemDto, LocalizedCategoryDto,
    NodeCreationDescriptor, ResourceBoundCreateArgsDto, normalize_search_text,
};

#[cfg(test)]
mod tests;
