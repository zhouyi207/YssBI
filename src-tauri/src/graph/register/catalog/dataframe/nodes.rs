//! DataFrame 节点定义

use crate::database::polars_dtype_to_data_type;
use crate::graph::node::{NodeDefinition, PinResolverContext};
use crate::graph::pin::{
    DataRole, PinDataTypeDefinition, PinDefinition, PinDirection, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::{Column, DataFrame};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_dataframe(registry);
    register_decompose_dataframe(registry);
    register_combine_dataframe(registry);
    register_filter_dataframe(registry);
}

fn register_get_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description("按 ID 获取 DataFrame", "Get a DataFrame by ID")
        .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
            "DataFrame",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        ))])
        .with_output_schema_resolver(Arc::new(|ctx| {
            let df_id = ctx.instance_params.dataframe_id()?;
            let provider = ctx.schema_provider.as_ref()?;
            provider(df_id)
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

fn register_decompose_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Decompose DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description("将 DataFrame 分解为各列 DataSeries", "Decompose a DataFrame into individual columns")
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

            let column_names = ctx.list_database_columns(&df_id)?;
            for col_name in column_names {
                let series = ctx.load_database_data_series(&df_id, &col_name)?;
                let element_type = polars_dtype_to_data_type(series.dtype());
                let series_id = ctx.put_data_series(series)?;
                let role = PinRole::Data(DataRole::Custom(col_name));
                let value = DataValue::DataSeries(DataSeriesValue::with_element_type(series_id, element_type));
                if let Err(_) = ctx.emit_output_by_role(&role, value) {}
            }
            Ok(())
        }));
    registry.register(definition);
}

fn register_combine_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Combine DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description(
            "将多个 DataSeries 合并为 DataFrame（Decompose DataFrame 的逆操作）",
            "Combine DataSeries into a DataFrame (opposite of Decompose DataFrame)",
        )
        .with_output_schema_resolver(Arc::new(|ctx| {
            let mut indexed: Vec<(usize, &crate::graph::node::DataSchema)> = ctx
                .input_schemas
                .iter()
                .filter_map(|(role, schema)| {
                    let idx = role.index()?;
                    if !role.matches_family(&PinRole::Data(DataRole::Inputs(0))) {
                        return None;
                    }
                    Some((idx, schema))
                })
                .collect();
            indexed.sort_by_key(|(i, _)| *i);

            let columns: Vec<crate::graph::node::ColumnSchema> = indexed
                .into_iter()
                .enumerate()
                .filter_map(|(i, (_, schema))| {
                    let col = schema.columns.first()?;
                    let name = if col.name.is_empty() || col.name == "literal" {
                        format!("col_{}", i)
                    } else {
                        col.name.clone()
                    };
                    Some(crate::graph::node::ColumnSchema {
                        name,
                        data_type: col.data_type.clone(),
                    })
                })
                .collect();
            if columns.is_empty() {
                None
            } else {
                Some(crate::graph::node::DataSchema { columns })
            }
        }))
        .with_pin_slots(vec![
            PinSlot::repeatable(
                PinDefinition::data_input(
                    "Column",
                    DataRole::Inputs(0),
                    PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
                )
                .with_optional(true),
                "Column",
                1,
                None,
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "DataFrame",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
            let series_vec: Vec<polars::prelude::Series> = values
                .into_iter()
                .filter_map(|v| {
                    if let DataValue::DataSeries(dsv) = v {
                        ctx.get_data_series(&dsv.id).ok()
                    } else {
                        None
                    }
                })
                .collect();

            if series_vec.is_empty() {
                return Err(
                    "Combine DataFrame: at least one DataSeries input must be connected"
                        .to_string(),
                );
            }

            let max_len = series_vec.iter().map(|s| s.len()).max().unwrap_or(0);
            let columns: Vec<Column> = series_vec
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let name = s.name().to_string();
                    let name = if name.is_empty() || name == "literal" {
                        format!("col_{}", i).into()
                    } else {
                        name.into()
                    };
                    let s = if s.len() < max_len {
                        s.extend_constant(polars::prelude::AnyValue::Null, max_len - s.len())
                            .unwrap_or(s)
                    } else {
                        s
                    };
                    Column::from(s.with_name(name))
                })
                .collect();

            let df = DataFrame::new(max_len, columns)
                .map_err(|e| format!("Combine DataFrame: {}", e))?;
            let id = ctx.put_dataframe(df)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), DataValue::DataFrame(id))?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_filter_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Filter DataFrame", vec!["Data".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description("按布尔 DataSeries 条件过滤行（保留为 true 的行）", "Filter rows by a Boolean DataSeries mask (keep rows where condition is true)")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "DataFrame",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "Condition",
                DataRole::Custom("condition".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "DataFrame",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )),
        ])
        .with_output_schema_resolver(Arc::new(|ctx| {
            ctx.input_schemas
                .get(&PinRole::Data(DataRole::Input))
                .cloned()
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let df_id = match &df_value {
                DataValue::DataFrame(id) => id.clone(),
                DataValue::Null => {
                    return Err("Filter DataFrame: input is not connected. Connect a Get DataFrame node.".to_string())
                }
                other => {
                    return Err(format!(
                        "Filter DataFrame: input is not a DataFrame (got {:?}).",
                        other.value_type().unwrap_or(DataType::Any)
                    ))
                }
            };

            let cond_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("condition".to_string())))?;
            let cond_id = match &cond_value {
                DataValue::DataSeries(v) => v.id.clone(),
                DataValue::Null => {
                    return Err("Filter DataFrame: Condition is not connected. Connect a Boolean DataSeries (e.g. from Compare nodes).".to_string())
                }
                other => {
                    return Err(format!(
                        "Filter DataFrame: Condition must be a Boolean DataSeries (got {:?}).",
                        other.value_type().unwrap_or(DataType::Any)
                    ))
                }
            };

            let df = ctx.get_dataframe(&df_id)?;
            let mask_series = ctx.get_data_series(&cond_id)?;
            let mask = mask_series
                .bool()
                .map_err(|e| format!("Filter DataFrame: Condition must be Boolean DataSeries: {}", e))?;

            if mask.len() != df.height() {
                return Err(format!(
                    "Filter DataFrame: Condition length {} does not match DataFrame rows {}",
                    mask.len(),
                    df.height()
                ));
            }

            let filtered = df.filter(mask).map_err(|e| format!("Filter DataFrame: {}", e))?;
            let out_id = ctx.put_dataframe(filtered)?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataFrame(out_id),
            )?;
            Ok(())
        }));
    registry.register(definition);
}
