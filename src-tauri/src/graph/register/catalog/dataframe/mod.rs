//! DataFrame 和 DataSeries 相关节点

mod df_adf_nodes;
mod dummy_nodes;
mod gls_nodes;
mod info_nodes;
mod iv_2sls_nodes;
mod iv_liml_nodes;
mod logit_nodes;
mod nodes;
mod ols_nodes;
mod panel_did_auxiliary;
mod panel_did_engine;
mod panel_did_nodes;
mod panel_nodes;
mod prais_nodes;
mod prediction_nodes;
mod probit_nodes;
mod series_compare_nodes;
mod series_nodes;
mod transform_nodes;
mod ts_align_nodes;
mod var_nodes;
mod var_types;
mod vec_coint_nodes;
mod vec_coint_vecrank_nodes;
mod wls_nodes;
mod xt_align_nodes;

use crate::graph::register::NodeRegistry;

pub use info_nodes::OLSResult;
pub use logit_nodes::{LogitConfigure, LogitModel};
pub use ols_nodes::{
    OLSClusterConfig, OLSConfigure, OLSFixedScaleConfig, OLSHACConfig, OLSModel, OLSNeweyConfig,
    VCEHC0, VCEHC1, VCEHC2, VCEHC3, VCENonRobust,
};
pub use panel_did_engine::{
    ComputeDidFakeGroupRequest, DidFakeGroupEnginePayload, DidPlaceboFakeGroupBlock,
    compute_fake_group_ri,
};
pub use prais_nodes::{PraisConfigure, PraisModel};
pub use probit_nodes::{ProbitConfigure, ProbitModel};

pub fn register(registry: &NodeRegistry) {
    nodes::register(registry);
    series_nodes::register(registry);
    series_compare_nodes::register(registry);
    transform_nodes::register(registry);
    df_adf_nodes::register(registry);
    ols_nodes::register(registry);
    var_nodes::register(registry);
    vec_coint_nodes::register(registry);
    vec_coint_vecrank_nodes::register(registry);
    wls_nodes::register(registry);
    gls_nodes::register(registry);
    prais_nodes::register(registry);
    iv_2sls_nodes::register(registry);
    logit_nodes::register(registry);
    panel_nodes::register(registry);
    panel_did_nodes::register(registry);
    probit_nodes::register(registry);
    dummy_nodes::register(registry);
    prediction_nodes::register(registry);
    ts_align_nodes::register(registry);
    xt_align_nodes::register(registry);
}
