//! DataFrame 节点定义

use crate::graph::node::{NodeDefinition, PinResolverContext};
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_dataframe(registry);
    register_decompose_dataframe(registry);
}

/// Get DataFrame 节点 - 获取数据帧引用
/// 通过 instance_params.dataframe_id 绑定具体数据帧
fn register_get_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get DataFrame", vec!["Data".to_string()])
        .with_node_type("get_dataframe")
        .with_ui_style("dataframe")
        .with_description("Get a DataFrame by ID")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![PinDefinition::data_output(
                "DataFrame",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )])
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            let params = ctx.get_instance_params();
            let dataframe_id = params
                .dataframe_id()
                .ok_or("Get DataFrame: dataframe_id not set")?;

            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataFrame(dataframe_id.to_string()),
            )?;
            Ok(())
        }));

    registry.register(definition);
}

/// Decompose DataFrame 节点 - 将 DataFrame 分解为各列的 DataSeries
///
/// 静态 pins: 仅一个 DataFrame 输入
/// 动态 pins: 根据连接的 DataFrame schema 自动生成各列的输出 pin
fn register_decompose_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Decompose DataFrame", vec!["Data".to_string()])
        .with_node_type("decompose_dataframe")
        .with_ui_style("dataframe")
        .with_description("Decompose a DataFrame into individual columns")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![PinDefinition::data_input(
                "DataFrame",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )])
        }))
        .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
            let mut pins = vec![PinDefinition::data_input(
                "DataFrame",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )];

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
                _ => return Err("Decompose DataFrame: input is not a DataFrame reference".to_string()),
            };

            let df = ctx.get_dataframe(&df_id)?;

            for col in df.get_columns() {
                let col_name = col.name().to_string();
                let series = col.clone().take_materialized_series();
                let series_id = ctx.put_series(series)?;

                let role = PinRole::Data(DataRole::Custom(col_name));
                // Only emit if the dynamic pin exists (user may not have connected all columns)
                if let Err(_) = ctx.emit_output_by_role(&role, DataValue::DataSeries(series_id)) {
                    // Pin doesn't exist for this column, skip
                }
            }

            Ok(())
        }));

    registry.register(definition);
}
