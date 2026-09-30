//! Statistical catalog entries awaiting interfaces and kernels.

use super::*;
use crate::catalog_entry::{self, Entry};

mod entries;
use entries::ENTRIES;

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for entry in ENTRIES.iter().filter(|entry| !implemented(entry.id)) {
        catalog_entry::append(
            std::slice::from_ref(entry),
            fragment,
            "builtin.statistics",
            "builtin.dataframe",
        )?;
    }
    Ok(())
}

fn implemented(id: &str) -> bool {
    super::analyses::implemented(id)
        || super::regression_models::implemented(id)
        || super::anova::implemented(id)
        || super::multivariate::implemented(id)
        || super::association::implemented(id)
        || super::classical::implemented(id)
        || super::descriptive::implemented(id)
}

pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    catalog_entry::documentation(ENTRIES, id, locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn inventory_placeholders_remain_registered_and_every_statistical_category_has_nodes() {
        let system = crate::build_builtin_node_system().unwrap();
        let catalog = system.catalog.localize(&system.registry, "zh-CN");
        let mut sources = BTreeSet::new();
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
            let item = catalog
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == entry.id)
                .unwrap();
            assert_eq!(item.category_id.as_ref(), entry.category);
            assert!(item.documentation.is_some());
            if implemented(entry.id) {
                assert!(!protocol.interface.ports.is_empty());
                continue;
            }
            assert!(
                protocol.interface.ports.is_empty(),
                "{} needs an explicit interface review",
                entry.id
            );
            assert!(protocol.parameters.is_empty());
        }
        assert_eq!(
            catalog
                .items
                .iter()
                .map(|item| item.category_id.as_ref())
                .filter(|category| category.starts_with("statistics."))
                .collect::<BTreeSet<_>>(),
            super::super::CATEGORIES
                .iter()
                .map(|&(id, _, _)| id)
                .collect()
        );
    }
}
