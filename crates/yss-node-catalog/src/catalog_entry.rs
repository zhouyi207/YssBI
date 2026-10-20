//! Shared assembly for catalog entries awaiting interfaces and execution kernels.

use crate::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, leaf,
    node_key, node_key_text, pure, sid,
};
use yss_node_protocol::*;

pub(crate) struct Entry {
    pub(crate) id: &'static str,
    pub(crate) method: &'static str,
    pub(crate) source_ids: &'static [u16],
    pub(crate) category: &'static str,
    /// Names `catalog.inventory.forms.<form>`.
    pub(crate) form: &'static str,
}

pub(crate) fn append(
    entries: &[Entry],
    fragment: &mut ProviderFragment,
    icon: &'static str,
    style: &'static str,
) -> Result<(), BuiltinAssemblyError> {
    for entry in entries {
        // An empty interface is intentional: these entries must stay unavailable
        // until a method-specific contract and its matching kernel are delivered.
        let protocol = NodeProtocol {
            type_id: sid(entry.id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(entry.id, "title")?,
                aliases_key: Some(node_key(entry.id, "aliases")?),
                category_id: sid(entry.category, NodeCategoryId::new)?,
                icon_id: sid(icon, IconId::new)?,
                style_id: sid(style, NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(entry.id, vec![], vec![], vec![])?,
            parameters: assembled_parameters(entry.id, vec![])?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: pure(),
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(leaf(protocol, entry.id));
    }
    Ok(())
}

/// Every module's entries; help generated from their messages applies when a node has no
/// authored page.
pub(crate) fn inventory() -> impl Iterator<Item = &'static Entry> {
    crate::statistics::INVENTORY
        .iter()
        .chain(crate::dataframe::INVENTORY)
}

pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    let entry = inventory().find(|entry| entry.id == id)?;
    let ids = entry
        .source_ids
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let text = |key: &str| crate::localization::catalog_text(locale, key);
    let title = text(&node_key_text(entry.id, "title"));
    let form = text(&format!("catalog.inventory.forms.{}", entry.form));
    let mut description = yss_i18n::format(
        &text("catalog.inventory.documentation"),
        &[
            ("title", title.as_ref()),
            ("method", entry.method),
            ("category", entry.category),
            ("form", form.as_ref()),
            ("ids", ids.as_str()),
        ],
    );
    let note_key = node_key_text(entry.id, "scope_note");
    if let Some(note) = crate::localization::optional_catalog_text(locale, &note_key) {
        description.push_str("\n\n");
        description.push_str(&yss_i18n::format(
            &text("catalog.inventory.scope_note"),
            &[("note", note)],
        ));
    }
    Some(description.into_boxed_str())
}
