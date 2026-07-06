pub fn apply_docs(mut def: crate::graph::node::NodeDefinition, name: &str) -> crate::graph::node::NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Event Begin" => (EVENT_BEGIN_ZH, EVENT_BEGIN_EN),
        _ => return None,
    })
}

pub const EVENT_BEGIN_ZH: &str = include_str!("zh/event_begin.md");
pub const EVENT_BEGIN_EN: &str = include_str!("en/event_begin.md");
