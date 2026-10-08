//! Statistical catalog entries awaiting interfaces and kernels.

use super::*;
use crate::catalog_entry::{self, Entry};
use std::collections::BTreeSet;

mod entries;
use entries::ENTRIES;

pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    let declared = fragment
        .nodes
        .iter()
        .map(|node| node.protocol().type_id.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    for entry in ENTRIES {
        if declared.contains(entry.id) {
            continue;
        }
        catalog_entry::append(
            std::slice::from_ref(entry),
            fragment,
            "builtin.statistics",
            "builtin.dataframe",
        )?;
    }
    Ok(())
}

pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    catalog_entry::documentation(ENTRIES, id, locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_completion_preserves_existing_node_declarations() {
        let entry = ENTRIES
            .iter()
            .find(|entry| entry.id == "yssbi.statistics.ml.knn")
            .unwrap();
        let mut fragment = ProviderFragment::default();
        catalog_entry::append(
            std::slice::from_ref(entry),
            &mut fragment,
            "builtin.statistics",
            "builtin.dataframe",
        )
        .unwrap();
        let expected = fragment.nodes[0].protocol().clone();

        append(&mut fragment).unwrap();

        let matching = fragment
            .nodes
            .iter()
            .filter(|node| node.protocol().type_id.as_str() == entry.id)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].protocol(), &expected);
    }

    #[test]
    fn inventory_placeholders_remain_registered_and_every_statistical_category_has_nodes() {
        let declared = super::super::defined_provider_fragment()
            .unwrap()
            .nodes
            .iter()
            .map(|node| node.protocol().type_id.clone())
            .collect::<BTreeSet<_>>();
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
            if declared.contains(&id) {
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
