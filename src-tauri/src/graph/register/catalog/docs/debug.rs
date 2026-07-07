pub fn apply_docs(
    mut def: crate::graph::node::NodeDefinition,
    name: &str,
) -> crate::graph::node::NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Print" => (PRINT_ZH, PRINT_EN),
        "View" => (VIEW_ZH, VIEW_EN),
        _ => return None,
    })
}

pub const PRINT_ZH: &str = include_str!("zh/print.md");
pub const PRINT_EN: &str = include_str!("en/print.md");
pub const VIEW_ZH: &str = include_str!("zh/view.md");
pub const VIEW_EN: &str = include_str!("en/view.md");
