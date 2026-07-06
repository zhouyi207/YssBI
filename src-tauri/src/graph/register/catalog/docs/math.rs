use crate::graph::node::NodeDefinition;

pub fn apply_docs(mut def: NodeDefinition, name: &str) -> NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "Add (+)" => (ADD_ZH, ADD_EN),
        "Subtract (-)" => (SUBTRACT_ZH, SUBTRACT_EN),
        "Multiply (*)" => (MULTIPLY_ZH, MULTIPLY_EN),
        "Divide (/)" => (DIVIDE_ZH, DIVIDE_EN),
        "Ln" => (LN_ZH, LN_EN),
        "Log2" => (LOG2_ZH, LOG2_EN),
        "Log10" => (LOG10_ZH, LOG10_EN),
        "Exp" => (EXP_ZH, EXP_EN),
        "Sqrt" => (SQRT_ZH, SQRT_EN),
        "Square" => (SQUARE_ZH, SQUARE_EN),
        _ => return None,
    })
}

pub const ADD_ZH: &str = include_str!("zh/add.md");
pub const ADD_EN: &str = include_str!("en/add.md");
pub const SUBTRACT_ZH: &str = include_str!("zh/subtract.md");
pub const SUBTRACT_EN: &str = include_str!("en/subtract.md");
pub const MULTIPLY_ZH: &str = include_str!("zh/multiply.md");
pub const MULTIPLY_EN: &str = include_str!("en/multiply.md");
pub const DIVIDE_ZH: &str = include_str!("zh/divide.md");
pub const DIVIDE_EN: &str = include_str!("en/divide.md");
pub const LN_ZH: &str = include_str!("zh/ln.md");
pub const LN_EN: &str = include_str!("en/ln.md");
pub const LOG2_ZH: &str = include_str!("zh/log2.md");
pub const LOG2_EN: &str = include_str!("en/log2.md");
pub const LOG10_ZH: &str = include_str!("zh/log10.md");
pub const LOG10_EN: &str = include_str!("en/log10.md");
pub const EXP_ZH: &str = include_str!("zh/exp.md");
pub const EXP_EN: &str = include_str!("en/exp.md");
pub const SQRT_ZH: &str = include_str!("zh/sqrt.md");
pub const SQRT_EN: &str = include_str!("en/sqrt.md");
pub const SQUARE_ZH: &str = include_str!("zh/square.md");
pub const SQUARE_EN: &str = include_str!("en/square.md");
