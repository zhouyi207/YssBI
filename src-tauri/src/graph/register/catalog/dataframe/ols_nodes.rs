//! OLS 回归节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ======================== OLS Configure 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSConfigure {
    pub constant: bool,
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_ols_configure_constant(registry);
    register_ols(registry);
}

// ======================== OLS Configure 常数节点 ========================

fn register_ols_configure_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Configure",
        vec!["Value".to_string(), "Constants".to_string()],
    )
    .with_ui_style("value")
    .with_description("OLS regression configuration (constant term, etc.)")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let config = OLSConfigure { constant: true };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS 节点 ========================

fn register_ols(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Endog",
            DataRole::Custom("endog".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            ),
            "Exog",
            1,
            None,
        ),
        PinSlot::fixed(PinDefinition::data_input(
            "Config",
            DataRole::Custom("ols_config".to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        let _endog = ctx.get_input_by_role(
            &PinRole::Data(DataRole::Custom("endog".to_string())),
        )?;

        let _exog_values = ctx.get_inputs_by_family(
            &PinRole::Data(DataRole::Inputs(0)),
        )?;

        let config_value = ctx.get_input_by_role(
            &PinRole::Data(DataRole::Custom("ols_config".to_string())),
        )?;

        let config_handle_id = config_value
            .as_handle_id()
            .ok_or("OLS: config input is not a Struct handle")?
            .to_string();

        let config_handle = ctx.get_handle(&config_handle_id)?;
        let _config = config_handle
            .downcast_ref::<OLSConfigure>()
            .ok_or("OLS: config handle is not an OLSConfigure")?;

        // TODO: 实际的 OLS 回归计算
        ctx.log("OLS: regression computation not yet implemented".to_string());

        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
