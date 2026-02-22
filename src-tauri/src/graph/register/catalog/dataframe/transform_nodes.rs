//! DataSeries 变换节点

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use ndarray::Array1;
use std::sync::Arc;
use yss_sci::tools::StandardizeTransform1D;

pub fn register(registry: &NodeRegistry) {
    register_standardize_series(registry);
    register_inverse_standardize_series(registry);
}

fn register_standardize_series(registry: &NodeRegistry) {
    let definition =
        NodeDefinition::new("Standardize Series", vec!["Data".to_string(), "Transform".to_string()])
            .with_node_type("standardize_series")
            .with_ui_style("dataframe")
            .with_description("Standardize a numeric DataSeries (z-score normalization) and output the fitted transform")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::data_input(
                    "Series",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
                )),
                PinSlot::fixed(PinDefinition::data_output(
                    "Standardized",
                    DataRole::Output,
                    PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
                )),
                PinSlot::fixed(PinDefinition::data_output(
                    "Transform",
                    DataRole::Custom("transform".to_string()),
                    PinDataTypeDefinition::concrete(DataType::Struct("StandardizeTransform1D".to_string())),
                )),
            ])
            .with_data_evaluator(Arc::new(|ctx| {
                let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
                let series_id = match &series_value {
                    DataValue::DataSeries(v) => v.id.clone(),
                    _ => return Err("Standardize Series: input is not a DataSeries".to_string()),
                };

                let series = ctx.get_series(&series_id)?;

                let f64_ca = series.f64().map_err(|e| {
                    format!("Standardize Series: cannot cast to Float64: {}", e)
                })?;
                let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
                let arr = Array1::from(values);

                let mut transform = StandardizeTransform1D::new();
                let standardized = transform.fit_transform(&arr);

                let result_series = polars::prelude::Series::from_iter(standardized.iter().copied());
                let result_id = ctx.put_series(result_series)?;
                ctx.emit_output_by_role(
                    &PinRole::Data(DataRole::Output),
                    DataValue::DataSeries(DataSeriesValue::with_element_type(result_id, DataType::Float64)),
                )?;

                let handle_id = ctx.put_handle(Box::new(transform));
                ctx.emit_output_by_role(
                    &PinRole::Data(DataRole::Custom("transform".to_string())),
                    DataValue::new_struct("StandardizeTransform1D", handle_id),
                )?;

                Ok(())
            }));
    registry.register(definition);
}

fn register_inverse_standardize_series(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Inverse Standardize Series",
        vec!["Data".to_string(), "Transform".to_string()],
    )
    .with_node_type("inverse_standardize_series")
    .with_ui_style("dataframe")
    .with_description("Reverse a standardization using a previously fitted transform")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Transform",
            DataRole::Custom("transform".to_string()),
            PinDataTypeDefinition::concrete(DataType::Struct(
                "StandardizeTransform1D".to_string(),
            )),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let series_id = match &series_value {
            DataValue::DataSeries(v) => v.id.clone(),
            _ => return Err("Inverse Standardize: input is not a DataSeries".to_string()),
        };

        let transform_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("transform".to_string())))?;
        let handle_id = transform_value
            .as_handle_id()
            .ok_or("Inverse Standardize: Transform input is not a Struct handle")?
            .to_string();

        let handle = ctx.get_handle(&handle_id)?;
        let transform = handle
            .downcast_ref::<StandardizeTransform1D>()
            .ok_or("Inverse Standardize: handle is not a StandardizeTransform1D")?;

        let series = ctx.get_series(&series_id)?;
        let f64_ca = series
            .f64()
            .map_err(|e| format!("Inverse Standardize: cannot cast to Float64: {}", e))?;
        let values: Vec<f64> = f64_ca.into_no_null_iter().collect();
        let arr = Array1::from(values);

        let result = transform.inverse_transform(&arr);

        let result_series = polars::prelude::Series::from_iter(result.iter().copied());
        let result_id = ctx.put_series(result_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                result_id,
                DataType::Float64,
            )),
        )?;

        Ok(())
    }));
    registry.register(definition);
}
