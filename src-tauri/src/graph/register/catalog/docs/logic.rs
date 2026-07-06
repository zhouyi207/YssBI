use crate::graph::node::NodeDefinition;

pub fn apply_docs(mut def: NodeDefinition, name: &str) -> NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Equal (==)" => (EQ_ZH, EQ_EN),
        "Not Equal (!=)" => (NEQ_ZH, NEQ_EN),
        "And (&&)" => (AND_ZH, AND_EN),
        "Or (||)" => (OR_ZH, OR_EN),
        "Not (!)" => (NOT_ZH, NOT_EN),
        _ => return None,
    })
}

pub const EQ_ZH: &str = include_str!("zh/equal.md");
pub const EQ_EN: &str = include_str!("en/equal.md");
pub const NEQ_ZH: &str = include_str!("zh/not_equal.md");
pub const NEQ_EN: &str = include_str!("en/not_equal.md");
pub const AND_ZH: &str = include_str!("zh/and.md");
pub const AND_EN: &str = include_str!("en/and.md");
pub const OR_ZH: &str = include_str!("zh/or.md");
pub const OR_EN: &str = include_str!("en/or.md");
pub const NOT_ZH: &str = include_str!("zh/not.md");
pub const NOT_EN: &str = include_str!("en/not.md");
