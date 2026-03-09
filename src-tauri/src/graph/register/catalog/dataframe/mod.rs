//! DataFrame 和 DataSeries 相关节点

mod nodes;
mod series_nodes;
mod transform_nodes;
mod info_nodes;
mod ols_nodes;
mod wls_nodes;
mod gls_nodes;
mod prais_nodes;
mod iv_2sls_nodes;
mod dummy_nodes;
mod prediction_nodes;
mod ts_align_nodes;

use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    nodes::register(registry);
    series_nodes::register(registry);
    transform_nodes::register(registry);
    ols_nodes::register(registry);
    wls_nodes::register(registry);
    gls_nodes::register(registry);
    prais_nodes::register(registry);
    iv_2sls_nodes::register(registry);
    dummy_nodes::register(registry);
    prediction_nodes::register(registry);
    ts_align_nodes::register(registry);
}
