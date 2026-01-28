//! 辅助宏定义

/// 获取子图的可变引用（用于避免借用检查器问题）
#[macro_export]
macro_rules! get_subgraph_mut {
    ($project:expr, $id:expr) => {{
        if $project.events.contains_key($id) {
            $project.events.get_mut($id)
        } else if $project.functions.contains_key($id) {
            $project.functions.get_mut($id)
        } else if $project.macros.contains_key($id) {
            $project.macros.get_mut($id)
        } else {
            None
        }
    }};
}

/// 获取子图的不可变引用
#[macro_export]
macro_rules! get_subgraph {
    ($project:expr, $id:expr) => {{
        if $project.events.contains_key($id) {
            $project.events.get($id)
        } else if $project.functions.contains_key($id) {
            $project.functions.get($id)
        } else if $project.macros.contains_key($id) {
            $project.macros.get($id)
        } else {
            None
        }
    }};
}
