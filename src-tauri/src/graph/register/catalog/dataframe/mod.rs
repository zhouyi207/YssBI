//! DataFrame 和 DataSeries 相关节点

mod nodes;
mod series_nodes;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    nodes::register(registry);
    series_nodes::register(registry);
}
