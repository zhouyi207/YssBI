//! 信息展示节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSResult {
    pub title: String,
    pub model_basic_info: ModelBasicInfo,
    pub coefficients: Vec<Coefficient>,
    pub diagnostic_info: DiagnosticInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBasicInfo {
    pub model_type: String,
    pub method: String,
    pub num_observation: usize,
    pub r_squared: f64,
    pub adj_r_squared: f64,
    pub f_statistic: f64,
    pub prob_f_statistic: f64,
    pub df_model: usize,
    pub df_residual: usize,
    pub df_total: usize,
    pub ss_model: f64,
    pub ss_residual: f64,
    pub ss_total: f64,
    pub ms_model: f64,
    pub ms_residual: f64,
    pub ms_total: f64,
    pub covariance_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coefficient {
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub t_value: f64,
    pub p_value: f64,
    #[serde(rename = "confidence_interval_0.025")]
    pub ci_lower: f64,
    #[serde(rename = "confidence_interval_0.975")]
    pub ci_upper: f64,
    pub is_significant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub cond_no: f64,
}

pub fn register(registry: &NodeRegistry) {
    register_ols_summary(registry);
}

fn register_ols_summary(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Display OLS regression results in a new window")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "OLS Result",
            DataRole::Custom("ols_result".to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        let input_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("ols_result".to_string())))?;

        let handle_id = input_value
            .as_handle_id()
            .ok_or("OLS Summary: input is not a Struct handle")?
            .to_string();

        let handle = ctx.get_handle(&handle_id)?;
        let ols_result = handle
            .downcast_ref::<OLSResult>()
            .ok_or("OLS Summary: handle is not an OLSResult")?;

        let json_data = serde_json::to_string(ols_result)
            .map_err(|e| format!("OLS Summary: failed to serialize: {}", e))?;

        ctx.open_window("ols_summary".to_string(), json_data);

        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
