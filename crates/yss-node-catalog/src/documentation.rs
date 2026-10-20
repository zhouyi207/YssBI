//! Node help: authored pages under `docs/src/nodes/<language>/`, indexed by the node types
//! each source-language page declares on its first line, then generated help for catalog
//! entries that have no interface yet.
use yss_node_protocol::NodeTypeId;

/// `(node type, [(language, markdown)])`, sorted by node type.
static PAGES: &[(&str, &[(&str, &str)])] = include!(concat!(env!("OUT_DIR"), "/documentation.rs"));

/// Localized help for one built-in definition, independent of project resources.
pub fn node_documentation(node_type_id: &NodeTypeId, locale: &str) -> Option<Box<str>> {
    let id = node_type_id.as_str();
    authored(id, locale)
        .map(Box::from)
        .or_else(|| crate::catalog_entry::documentation(id, locale))
}

fn authored(id: &str, locale: &str) -> Option<&'static str> {
    let index = PAGES.binary_search_by(|(node, _)| (*node).cmp(id)).ok()?;
    let (_, pages) = PAGES.get(index)?;
    // A supported locale may still lack translated help; the source pages then apply.
    let language = yss_i18n::language(yss_i18n::resolve_locale(locale));
    let source = yss_i18n::language(yss_i18n::FALLBACK_LOCALE);
    pages
        .iter()
        .find(|(page_language, _)| *page_language == language)
        .or_else(|| {
            pages
                .iter()
                .find(|(page_language, _)| *page_language == source)
        })
        .map(|(_, text)| *text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn generated_help_resolves_its_messages_and_authored_pages_supersede_scope_notes() {
        let keys = crate::BuiltinCatalog::keys().collect::<BTreeSet<_>>();
        for entry in crate::catalog_entry::inventory() {
            let form = format!("catalog.inventory.forms.{}", entry.form);
            assert!(keys.contains(form.as_str()), "{}: {form}", entry.id);
            let note = format!("nodes.{}.scope_note", entry.id);
            if authored(entry.id, yss_i18n::FALLBACK_LOCALE).is_some() {
                assert!(
                    !keys.contains(note.as_str()),
                    "{note} is superseded by a page"
                );
                continue;
            }
            let id = NodeTypeId::new(entry.id).unwrap();
            for locale in yss_i18n::LOCALES {
                let help = node_documentation(&id, locale).unwrap();
                assert!(
                    !help.contains("catalog.inventory.") && !help.contains("nodes.yssbi."),
                    "{help}"
                );
            }
        }
    }

    #[test]
    fn pages_name_registered_nodes_and_every_node_with_an_interface_has_help() {
        let system = crate::build_builtin_node_system().unwrap();
        for (node, _) in PAGES {
            assert!(
                system
                    .registry
                    .get(&NodeTypeId::new(*node).unwrap())
                    .is_some(),
                "{node} is documented but not registered"
            );
        }
        // Graph-managed function boundaries and two row operations predate authored help.
        let undocumented = [
            "yssbi.dataframe.filter.rows",
            "yssbi.dataframe.limit",
            "yssbi.project.function.entry",
            "yssbi.project.function.return",
        ];
        for (id, _) in system.registry.iter() {
            let help = node_documentation(id, yss_i18n::CHINESE);
            assert_eq!(
                help.is_none(),
                undocumented.contains(&id.as_str()),
                "{id} help availability changed"
            );
        }
    }
}
