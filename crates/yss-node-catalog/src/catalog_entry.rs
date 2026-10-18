//! Shared assembly for catalog entries awaiting interfaces and execution kernels.

use crate::builtin::{
    BuiltinAssemblyError, ProviderFragment, add_node_messages, assembled_interface,
    assembled_parameters, leaf, node_key, sid,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use yss_i18n::SimpleBackend;
use yss_node_protocol::*;

pub(crate) struct Entry {
    pub(crate) id: &'static str,
    pub(crate) method: &'static str,
    pub(crate) source_ids: &'static [u16],
    pub(crate) category: &'static str,
    pub(crate) en: &'static str,
    pub(crate) zh: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) product_form: &'static str,
    pub(crate) scope_note: &'static str,
}

pub(crate) fn append(
    entries: &[Entry],
    fragment: &mut ProviderFragment,
    icon: &'static str,
    style: &'static str,
) -> Result<(), BuiltinAssemblyError> {
    for entry in entries {
        add_node_messages(
            &mut fragment.messages,
            entry.id,
            [entry.en, entry.zh],
            Some([entry.aliases, entry.aliases]),
        );
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
            execution: ExecutionSemantics {
                determinism: Determinism::Deterministic,
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(leaf(protocol, entry.id));
    }
    Ok(())
}

static DOCUMENTATION_MESSAGES: LazyLock<SimpleBackend> = LazyLock::new(|| {
    let mut backend = SimpleBackend::new();
    for (locale, template) in [
        (
            "en-US",
            "# %{title}\n\nCatalog entry only; unavailable for execution. Inputs, outputs, parameters and the execution kernel are not defined yet.\n\nMethod: `%{method}`\n\nCategory: `%{category}`\n\nOriginal product form (source language): %{product_form}\n\nOriginal method IDs: %{ids} (removed from the pending inventory).\n\nScope note (source language): %{scope_note}",
        ),
        (
            "zh-CN",
            "# %{title}\n\n目录入口，暂不可执行。输入、输出、参数和执行内核尚未定义。\n\n方法：`%{method}`\n\n分类：`%{category}`\n\n原始用途：%{product_form}\n\n原始方法编号：%{ids}（已从待登记清单移除）。\n\n%{scope_note}",
        ),
    ] {
        backend.add_translations(
            Cow::Borrowed(locale),
            HashMap::from([(
                Cow::Borrowed("catalog.inventory.documentation"),
                Cow::Borrowed(template),
            )]),
        );
    }
    backend
});

pub(crate) fn documentation(entries: &[Entry], id: &str, locale: &str) -> Option<Box<str>> {
    let entry = entries.iter().find(|entry| entry.id == id)?;
    let ids = entry
        .source_ids
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let title = if crate::documentation::is_chinese_locale(locale) {
        entry.zh
    } else {
        entry.en
    };
    let template = yss_i18n::translate(
        &*DOCUMENTATION_MESSAGES,
        locale,
        "catalog.inventory.documentation",
        "en-US",
    );
    let description = yss_i18n::format(
        &template,
        &[
            ("title", title),
            ("method", entry.method),
            ("category", entry.category),
            ("product_form", entry.product_form),
            ("ids", ids.as_str()),
            ("scope_note", entry.scope_note),
        ],
    );
    Some(description.into_boxed_str())
}
