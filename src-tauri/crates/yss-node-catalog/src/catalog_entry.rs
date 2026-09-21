//! Shared assembly for catalog entries awaiting interfaces and execution kernels.

use crate::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, iid, leaf,
    sid,
};
use crate::{Aliases, Text};
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
        let title: &'static str = Box::leak(format!("nodes.{}.title", entry.id).into_boxed_str());
        let aliases: &'static str =
            Box::leak(format!("nodes.{}.aliases", entry.id).into_boxed_str());
        fragment.messages.extend([
            ("en-US", title, Text(entry.en)),
            ("zh-CN", title, Text(entry.zh)),
            ("en-US", aliases, Aliases(entry.aliases)),
            ("zh-CN", aliases, Aliases(entry.aliases)),
        ]);
        // An empty interface is intentional: these entries must stay unavailable
        // until a method-specific contract and its matching kernel are delivered.
        let protocol = NodeProtocol {
            type_id: sid(entry.id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: iid(title)?,
                documentation_key: None,
                aliases_key: Some(iid(aliases)?),
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
                cache: CachePolicy::PerRun,
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(leaf(protocol, entry.id));
    }
    Ok(())
}

pub(crate) fn documentation(entries: &[Entry], id: &str, locale: &str) -> Option<Box<str>> {
    let entry = entries.iter().find(|entry| entry.id == id)?;
    let ids = entry
        .source_ids
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let locale = locale.trim().replace('_', "-").to_ascii_lowercase();
    let description = if locale == "zh" || locale.starts_with("zh-") {
        format!(
            "# {}\n\n目录入口，暂不可执行。输入、输出、参数和执行内核尚未定义。\n\n方法：`{}`\n\n分类：`{}`\n\n原始用途：{}\n\n原始方法编号：{}（已从待登记清单移除）。\n\n{}",
            entry.zh, entry.method, entry.category, entry.product_form, ids, entry.scope_note,
        )
    } else {
        format!(
            "# {}\n\nCatalog entry only; unavailable for execution. Inputs, outputs, parameters and the execution kernel are not defined yet.\n\nMethod: `{}`\n\nCategory: `{}`\n\nOriginal product form (source language): {}\n\nOriginal method IDs: {} (removed from the pending inventory).\n\nScope note (source language): {}",
            entry.en, entry.method, entry.category, entry.product_form, ids, entry.scope_note,
        )
    };
    Some(description.into_boxed_str())
}
