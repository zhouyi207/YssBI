//! 内置节点目录

pub mod math;
pub mod control;
pub mod debug;
pub mod logic;
pub mod value;
pub mod dataframe;
pub mod event;
pub mod plot;
pub mod distribution;

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
