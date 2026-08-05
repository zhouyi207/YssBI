//! JSON serialization for runtime Struct handles shown in View nodes.

use crate::sci::models::regression::{
    LogitConfigure, LogitModel, OLSClusterConfig, OLSConfigure, OLSFixedScaleConfig, OLSHACConfig,
    OLSModel, OLSNeweyConfig, OLSResult, PraisConfigure, PraisModel, ProbitConfigure, ProbitModel,
    VCEHC0, VCEHC1, VCEHC2, VCEHC3, VCENonRobust,
};
use serde::Serialize;
use std::any::Any;
use std::sync::Arc;

fn try_serialize<T: Serialize + Send + Sync + 'static>(
    handle: &Arc<dyn Any + Send + Sync>,
) -> Option<serde_json::Value> {
    handle
        .clone()
        .downcast::<T>()
        .ok()
        .and_then(|value| serde_json::to_value(value.as_ref()).ok())
}

/// Serialize a known Struct handle to JSON for View / pin preview.
pub fn serialize_struct_handle(
    type_key: &str,
    handle: &Arc<dyn Any + Send + Sync>,
) -> Option<serde_json::Value> {
    match type_key {
        "OLSModel" => try_serialize::<OLSModel>(handle),
        "OLSResult" => try_serialize::<OLSResult>(handle),
        "LogitModel" => try_serialize::<LogitModel>(handle),
        "LogitConfigure" => try_serialize::<LogitConfigure>(handle),
        "ProbitModel" => try_serialize::<ProbitModel>(handle),
        "ProbitConfigure" => try_serialize::<ProbitConfigure>(handle),
        "PraisModel" => try_serialize::<PraisModel>(handle),
        "PraisConfigure" => try_serialize::<PraisConfigure>(handle),
        "OLSConfigure" => try_serialize::<OLSConfigure>(handle),
        "OLSFixedScaleConfig" => try_serialize::<OLSFixedScaleConfig>(handle),
        "OLSClusterConfig" => try_serialize::<OLSClusterConfig>(handle),
        "OLSHACConfig" => try_serialize::<OLSHACConfig>(handle),
        "OLSNeweyConfig" => try_serialize::<OLSNeweyConfig>(handle),
        "VCENonRobust" => try_serialize::<VCENonRobust>(handle),
        "VCEHC0" => try_serialize::<VCEHC0>(handle),
        "VCEHC1" => try_serialize::<VCEHC1>(handle),
        "VCEHC2" => try_serialize::<VCEHC2>(handle),
        "VCEHC3" => try_serialize::<VCEHC3>(handle),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ols_model_serializes_for_view() {
        let model = OLSModel {
            betas: vec![1.0, 2.5],
            has_constant: true,
            variable_specs: vec![],
            kept_indices: None,
        };
        let handle: Arc<dyn Any + Send + Sync> = Arc::new(model);
        let json = serialize_struct_handle("OLSModel", &handle).expect("OLSModel json");
        assert_eq!(
            json.get("betas")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(2)
        );
        assert_eq!(
            json.get("has_constant").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}
