//! DataFrame 和 DataSeries 相关节点

mod nodes;
mod series_nodes;
mod transform_nodes;
mod info_nodes;
mod ols_nodes;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    nodes::register(registry);
    series_nodes::register(registry);
    transform_nodes::register(registry);
    info_nodes::register(registry);
    ols_nodes::register(registry);
}
