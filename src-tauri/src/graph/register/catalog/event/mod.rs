//! 事件节点 - 图的入口与触发

pub mod event_begin;

pub use event_begin::EVENT_BEGIN_NODE_TYPE;

use super::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    event_begin::register(registry);
}
