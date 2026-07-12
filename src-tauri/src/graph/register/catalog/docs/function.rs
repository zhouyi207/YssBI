//! Function shell node documentation (Function Entry / Function Return).

use crate::graph::node::NodeDefinition;

pub fn apply_docs(mut def: NodeDefinition, name: &str) -> NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Function Entry" => (ENTRY_ZH, ENTRY_EN),
        "Function Return" => (RETURN_ZH, RETURN_EN),
        _ => return None,
    })
}

pub const ENTRY_ZH: &str = include_str!("zh/function_entry.md");
pub const ENTRY_EN: &str = include_str!("en/function_entry.md");
pub const RETURN_ZH: &str = include_str!("zh/function_return.md");
pub const RETURN_EN: &str = include_str!("en/function_return.md");
