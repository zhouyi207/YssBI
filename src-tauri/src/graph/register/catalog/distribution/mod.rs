//! 统计分布节点：根据分布参数生成符合该分布的 DataSeries

mod continuous;
mod discrete;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    continuous::register(registry);
    discrete::register(registry);
}
