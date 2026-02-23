//! DataFrame 节点定义

use crate::database::polars_dtype_to_data_type;
use crate::graph::node::{NodeDefinition, PinResolverContext};
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinDirection, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_dataframe(registry);
    register_decompose_dataframe(registry);
}

fn register_get_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_description("Get a DataFrame by ID")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output(
                "DataFrame", DataRole::Output, PinDataTypeDefinition::concrete(DataType::DataFrame),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let params = ctx.get_instance_params();
            let dataframe_id = params.dataframe_id().ok_or("Get DataFrame: dataframe_id not set")?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataFrame(dataframe_id.to_string()),
            )?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_decompose_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Decompose DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_description("Decompose a DataFrame into individual columns")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataFrame", DataRole::Input, PinDataTypeDefinition::concrete(DataType::DataFrame),
            )),
            PinSlot::derived_from_input(
                PinRole::Data(DataRole::Input),
                PinDirection::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
            ),
        ])
        .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
            let mut pins = vec![];
            if let Some(schema) = ctx.input_schemas.get(&PinRole::Data(DataRole::Input)) {
                for col in &schema.columns {
                    pins.push(
                        PinDefinition::data_output(
                            &col.name,
                            DataRole::Custom(col.name.clone()),
                            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(col.data_type.clone()))),
                        )
                        .with_dynamic(true),
                    );
                }
            }
            Ok(pins)
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let df_id = match &df_value {
                DataValue::DataFrame(id) => id.clone(),
                DataValue::Null => {
                    return Err("Decompose DataFrame: input is not connected (got Null). Connect a Get DataFrame node.".to_string())
                }
                other => {
                    return Err(format!(
                        "Decompose DataFrame: input is not a DataFrame reference (got {:?}). Connect a Get DataFrame node.",
                        other.value_type().unwrap_or(crate::graph::DataType::Any)
                    ))
                }
            };

            let df = ctx.get_dataframe(&df_id)?;
            for col in df.get_columns() {
                let col_name = col.name().to_string();
                let series = col.clone().take_materialized_series();
                let series_id = ctx.put_series(series)?;
                let element_type = polars_dtype_to_data_type(col.dtype());
                let role = PinRole::Data(DataRole::Custom(col_name));
                let value = DataValue::DataSeries(DataSeriesValue::with_element_type(series_id, element_type));
                if let Err(_) = ctx.emit_output_by_role(&role, value) {}
            }
            Ok(())
        }));
    registry.register(definition);
}
