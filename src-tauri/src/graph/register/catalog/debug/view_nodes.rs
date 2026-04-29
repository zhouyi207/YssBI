//! View 节点：查看各种类型数据的具体内容
//!
//! 通过 open_window 将数据序列化后存入 WindowDataStore，执行结束后仍可刷新查看

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, ExecRole, PinDefinition, PinRole, PinDataTypeDefinition, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use polars::prelude::DataFrame;
use serde_json::json;
use std::sync::Arc;
use yss_sci::api::database::anyvalue_to_json;

const PREVIEW_ROWS: usize = 100;

/// 将 DataValue 标量转为 JSON
fn scalar_to_json(v: &DataValue) -> serde_json::Value {
    match v {
        DataValue::Null => serde_json::Value::Null,
        DataValue::Boolean(b) => serde_json::Value::Bool(*b),
        DataValue::Int32(i) => serde_json::json!(i),
        DataValue::Int64(i) => serde_json::json!(i),
        DataValue::Float32(f) => serde_json::Number::from_f64(*f as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        DataValue::Float64(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        DataValue::String(s) => serde_json::Value::String(s.clone()),
        DataValue::Array(arr) => serde_json::Value::Array(
            arr.iter().map(scalar_to_json).collect(),
        ),
        DataValue::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), scalar_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        _ => serde_json::Value::String(format!("{:?}", v)),
    }
}

/// DataFrame 转为预览 JSON
fn dataframe_to_view_json(df: &DataFrame) -> serde_json::Value {
    let total_rows = df.height();
    let preview_count = total_rows.min(PREVIEW_ROWS);
    let sliced = df.slice(0, preview_count);

    let columns: Vec<String> = df.columns().iter().map(|c| c.name().to_string()).collect();

    let rows: Vec<Vec<serde_json::Value>> = (0..sliced.height())
        .map(|row_idx| {
            sliced
                .columns()
                .iter()
                .map(|s| {
                    s.get(row_idx)
                        .map(anyvalue_to_json)
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect()
        })
        .collect();

    json!({
        "viewType": "data_view",
        "dataType": "dataframe",
        "title": "View: DataFrame",
        "columns": columns,
        "rows": rows,
        "totalRows": total_rows,
        "previewRows": preview_count,
    })
}

/// Series 转为预览 JSON
fn series_to_view_json(series: &polars::prelude::Series) -> serde_json::Value {
    let name = series.name().to_string();
    let len = series.len();
    let dtype = format!("{:?}", series.dtype());
    let preview_count = len.min(PREVIEW_ROWS);

    let values: Vec<serde_json::Value> = (0..preview_count)
        .map(|i| {
            series
                .get(i)
                .map(anyvalue_to_json)
                .unwrap_or(serde_json::Value::Null)
        })
        .collect();

    json!({
        "viewType": "data_view",
        "dataType": "series",
        "title": format!("View: {}", if name.is_empty() { "Series" } else { &name }),
        "name": name,
        "dtype": dtype,
        "values": values,
        "length": len,
        "previewCount": preview_count,
    })
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("View", vec!["Debug".to_string(), "Data".to_string()])
        .with_ui_style("debug")
        .with_description("View data in a window (DataFrame, DataSeries, or scalar). Data persists after execution.")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "Data",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::Any),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;

            let view_json = match &input_value {
                DataValue::Null => {
                    json!({
                        "viewType": "data_view",
                        "dataType": "null",
                        "title": "View: (null)",
                        "message": "No data connected",
                    })
                }
                DataValue::DataFrame(id) => {
                    let df = ctx.get_dataframe(id)?;
                    dataframe_to_view_json(&df)
                }
                DataValue::DataSeries(v) => {
                    let series = ctx.get_series(&v.id)?;
                    series_to_view_json(&series)
                }
                DataValue::Struct { type_key, handle_id } => {
                    json!({
                        "viewType": "data_view",
                        "dataType": "struct",
                        "title": format!("View: {}", type_key),
                        "typeKey": type_key,
                        "handleId": handle_id,
                        "message": "Struct handles are not directly viewable. Use specialized nodes to extract data.",
                    })
                }
                scalar => {
                    json!({
                        "viewType": "data_view",
                        "dataType": "scalar",
                        "title": "View: Scalar",
                        "value": scalar_to_json(scalar),
                        "valueType": format!("{:?}", scalar.value_type().unwrap_or(DataType::Any)),
                    })
                }
            };

            let json_str = serde_json::to_string(&view_json)
                .map_err(|e| format!("View: failed to serialize: {}", e))?;

            ctx.open_window("data_view".to_string(), json_str);
            ctx.log("View: opened data view window".to_string());

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
