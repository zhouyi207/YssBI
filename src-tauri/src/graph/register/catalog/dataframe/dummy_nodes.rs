//! 哑变量相关节点

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue, DummyInfo};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_add_dummy_info(registry);
}

fn register_add_dummy_info(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Add Dummy Info",
        vec!["Data".to_string(), "Transform".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description("为 Categorical DataSeries 标注哑变量编码元数据，供 OLS 回归使用", "Annotate a Categorical DataSeries with dummy variable encoding metadata for OLS regression")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Drop Category",
                DataRole::Custom("drop_category".to_string()),
                PinDataTypeDefinition::concrete(DataType::String),
            )
            .with_optional(true)
            .with_metadata(true, "text"),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Role",
                DataRole::Custom("role".to_string()),
                PinDataTypeDefinition::concrete(DataType::String),
            )
            .with_optional(true)
            .with_metadata(true, "dropdown")
            .with_widget_options(vec![
                "General".to_string(),
                "Individual".to_string(),
                "Time".to_string(),
            ]),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Series",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Categorical))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let dsv = match &series_value {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err("Add Dummy Info: input is not a DataSeries".to_string()),
        };

        let drop_category = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("drop_category".to_string())))
            .ok()
            .and_then(|v| match v {
                DataValue::String(s) if !s.is_empty() => Some(s),
                _ => None,
            });

        let role_str = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("role".to_string())))
            .ok()
            .and_then(|v| match v {
                DataValue::String(s) => Some(s),
                _ => None,
            })
            .unwrap_or_else(|| "General".to_string());

        let role = match role_str.as_str() {
            "Individual" => CategoricalRole::Individual,
            "Time" => CategoricalRole::Time,
            _ => CategoricalRole::General,
        };

        let dummy_info = DummyInfo { drop_category, role };
        let output = DataSeriesValue {
            id: dsv.id,
            element_type: dsv.element_type,
            dummy_info: Some(dummy_info),
            time_series_state: dsv.time_series_state.clone(),
        };

        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataSeries(output),
        )?;
        Ok(())
    }));
    registry.register(definition);
}
