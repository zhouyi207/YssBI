//! 内置节点目录

pub mod control;
pub mod dataframe;
pub mod debug;
pub mod distribution;
pub mod docs;
pub mod event;
pub mod logic;
pub mod math;
pub mod plot;
pub mod value;

use super::NodeRegistry;

/// 注册所有内置节点
pub fn register_builtin_nodes(registry: &NodeRegistry) {
    math::register(registry);
    control::register(registry);
    debug::register(registry);
    logic::register(registry);
    value::register(registry);
    dataframe::register(registry);
    event::register(registry);
    plot::register(registry);
    distribution::register(registry);
}
