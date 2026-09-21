//! Statistical catalog entries awaiting interfaces and kernels.

use super::*;
use crate::catalog_entry::{self, Entry};

mod entries;
use entries::ENTRIES;

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    catalog_entry::append(ENTRIES, fragment, "builtin.statistics", "builtin.dataframe")
}

pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    catalog_entry::documentation(ENTRIES, id, locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn inventory_entries_are_visible_leaf_placeholders_covering_every_statistical_category() {
        let system = crate::build_builtin_node_system().unwrap();
        let catalog = system.catalog.localize(&system.registry, "zh-CN");
        let mut sources = BTreeSet::new();
        let mut categories = BTreeSet::new();
        for entry in ENTRIES {
            for source in entry.source_ids {
                assert!(
                    sources.insert(source),
                    "source {source} has duplicate entries"
                );
            }
            let id = NodeTypeId::new(entry.id).unwrap();
            let registered = system.registry.get(&id).unwrap();
            assert_eq!(
                registered
                    .implementation()
                    .unwrap()
                    .implementation_identity(),
                entry.id
            );
            let protocol = registered.protocol();
            assert!(
                protocol.interface.ports.is_empty(),
                "{} needs an explicit interface review",
                entry.id
            );
            assert!(protocol.parameters.parameters.is_empty());
            let item = catalog
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == entry.id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), entry.category);
            assert!(item.documentation.is_some());
            categories.insert(entry.category);
        }
        assert_eq!(
            categories,
            super::super::CATEGORIES
                .iter()
                .map(|&(id, _, _)| id)
                .collect()
        );
        assert_eq!(ENTRIES.len(), 309);
        assert_eq!(sources.len(), 310);
    }
}
