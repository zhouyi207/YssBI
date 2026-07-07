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
        "Do" => (DO_ZH, DO_EN),
        "Merge" => (MERGE_ZH, MERGE_EN),
        "Sleep" => (SLEEP_ZH, SLEEP_EN),
        "For Loop" => (FOR_LOOP_ZH, FOR_LOOP_EN),
        "Switch" => (SWITCH_ZH, SWITCH_EN),
        "While Loop" => (WHILE_LOOP_ZH, WHILE_LOOP_EN),
        _ => return None,
    })
}

pub const BRANCH_ZH: &str = include_str!("zh/branch.md");
pub const BRANCH_EN: &str = include_str!("en/branch.md");
pub const SEQUENCE_ZH: &str = include_str!("zh/sequence.md");
pub const SEQUENCE_EN: &str = include_str!("en/sequence.md");
pub const DO_ZH: &str = include_str!("zh/do.md");
pub const DO_EN: &str = include_str!("en/do.md");
pub const MERGE_ZH: &str = include_str!("zh/merge.md");
pub const MERGE_EN: &str = include_str!("en/merge.md");
pub const SLEEP_ZH: &str = include_str!("zh/sleep.md");
pub const SLEEP_EN: &str = include_str!("en/sleep.md");
pub const FOR_LOOP_ZH: &str = include_str!("zh/for_loop.md");
pub const FOR_LOOP_EN: &str = include_str!("en/for_loop.md");
pub const SWITCH_ZH: &str = include_str!("zh/switch.md");
pub const SWITCH_EN: &str = include_str!("en/switch.md");
pub const WHILE_LOOP_ZH: &str = include_str!("zh/while_loop.md");
pub const WHILE_LOOP_EN: &str = include_str!("en/while_loop.md");
