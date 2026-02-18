//! 事件节点 - 图的入口与触发

pub mod event_begin;

use super::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    event_begin::register(registry);
}
