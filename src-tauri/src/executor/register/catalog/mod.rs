//! 内置节点目录

pub mod math;
pub mod control;

use crate::executor::node::NodeRegistry;

/// 注册所有内置节点
pub fn register_builtin_nodes(registry: &NodeRegistry) {
    math::register(registry);
    control::register(registry);
}
