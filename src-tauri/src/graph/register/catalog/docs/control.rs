pub fn apply_docs(mut def: crate::graph::node::NodeDefinition, name: &str) -> crate::graph::node::NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Branch" => (BRANCH_ZH, BRANCH_EN),
        "Sequence" => (SEQUENCE_ZH, SEQUENCE_EN),
        _ => return None,
    })
}

pub const BRANCH_ZH: &str = include_str!("zh/branch.md");
pub const BRANCH_EN: &str = include_str!("en/branch.md");
pub const SEQUENCE_ZH: &str = include_str!("zh/sequence.md");
pub const SEQUENCE_EN: &str = include_str!("en/sequence.md");
