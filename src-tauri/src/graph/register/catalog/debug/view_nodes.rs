//! View 节点：查看各种类型数据的具体内容
//!
//! 通过 open_window 将 metadata 存入 WindowDataStore；DataFrame/DataSeries 另存 page source 供分页拉取。

use crate::execution::{ExecutionEffect, WindowDataSource};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::dataframe::OLSResult;
use crate::graph::value::{DataType, DataValue};
use polars::prelude::DataFrame;
use serde_json::json;
use std::sync::Arc;

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
        DataValue::Array(arr) => serde_json::Value::Array(arr.iter().map(scalar_to_json).collect()),
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

fn dataframe_metadata(df: &DataFrame) -> serde_json::Value {
    let columns: Vec<String> = df.columns().iter().map(|c| c.name().to_string()).collect();
    json!({
        "viewType": "data_view",
        "dataType": "dataframe",
        "title": "View: DataFrame",
        "columns": columns,
        "totalRows": df.height(),
    })
}

fn series_metadata(series: &polars::prelude::Series) -> serde_json::Value {
    let name = series.name().to_string();
    json!({
        "viewType": "data_view",
        "dataType": "series",
        "title": format!("View: {}", if name.is_empty() { "Series" } else { &name }),
        "name": name,
        "dtype": format!("{:?}", series.dtype()),
        "length": series.len(),
    })
}

fn struct_view_json(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
    type_key: &str,
    handle_id: &str,
) -> Result<serde_json::Value, String> {
    if type_key == "OLSResult" {
        let handle = ctx.get_handle(handle_id)?;
        let ols = handle
            .downcast::<OLSResult>()
            .map_err(|_| format!("Handle '{}' is not OLSResult", handle_id))?;
        let structured = serde_json::to_value(ols.as_ref())
            .map_err(|e| format!("View: failed to serialize OLSResult: {}", e))?;
        return Ok(json!({
            "viewType": "data_view",
            "dataType": "struct",
            "structKind": "ols_result",
            "title": ols.title.clone(),
            "typeKey": type_key,
            "handleId": handle_id,
            "structured": structured,
        }));
    }

    Ok(json!({
        "viewType": "data_view",
        "dataType": "struct",
        "structKind": "unknown",
        "title": format!("View: {}", type_key),
        "typeKey": type_key,
        "handleId": handle_id,
        "message": format!("Struct type '{}' has no dedicated viewer yet.", type_key),
    }))
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("View", vec!["Debug".to_string(), "Data".to_string()])
        .with_ui_style("debug")
        .with_localized_description("在窗口中查看数据（DataFrame、DataSeries 或标量），执行后数据仍保留", "View data in a window (DataFrame, DataSeries, or scalar). Data persists after execution.")
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

            match &input_value {
                DataValue::Null => {
                    let view_json = json!({
                        "viewType": "data_view",
                        "dataType": "null",
                        "title": "View: (null)",
                        "message": "No data connected",
                    });
                    let json_str = serde_json::to_string(&view_json)
                        .map_err(|e| format!("View: failed to serialize: {}", e))?;
                    ctx.open_window("data_view".to_string(), json_str);
                }
                DataValue::DataFrame(id) => {
                    let df = ctx.get_dataframe(id)?;
                    let view_json = dataframe_metadata(&df);
                    let json_str = serde_json::to_string(&view_json)
                        .map_err(|e| format!("View: failed to serialize: {}", e))?;
                    ctx.open_source_window(
                        "data_view".to_string(),
                        json_str,
                        WindowDataSource::DataFrame(df),
                    );
                }
                DataValue::DataSeries(v) => {
                    let series = ctx.get_series(&v.id)?;
                    let view_json = series_metadata(&series);
                    let json_str = serde_json::to_string(&view_json)
                        .map_err(|e| format!("View: failed to serialize: {}", e))?;
                    ctx.open_source_window(
                        "data_view".to_string(),
                        json_str,
                        WindowDataSource::Series(series),
                    );
                }
                DataValue::Struct { type_key, handle_id } => {
                    let view_json = struct_view_json(ctx, type_key, handle_id)?;
                    let json_str = serde_json::to_string(&view_json)
                        .map_err(|e| format!("View: failed to serialize: {}", e))?;
                    ctx.open_window("data_view".to_string(), json_str);
                }
                scalar => {
                    let view_json = json!({
                        "viewType": "data_view",
                        "dataType": "scalar",
                        "title": "View: Scalar",
                        "value": scalar_to_json(scalar),
                        "valueType": format!("{:?}", scalar.value_type().unwrap_or(DataType::Any)),
                    });
                    let json_str = serde_json::to_string(&view_json)
                        .map_err(|e| format!("View: failed to serialize: {}", e))?;
                    ctx.open_window("data_view".to_string(), json_str);
                }
            }

            ctx.log("View: opened data view window".to_string());
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::NamedFrom;

    #[test]
    fn dataframe_metadata_is_source_only_without_rows() {
        let df = polars::df!("a" => [1, 2, 3]).unwrap();
        let meta = dataframe_metadata(&df);
        assert_eq!(meta["dataType"], "dataframe");
        assert_eq!(meta["totalRows"], 3);
        assert!(meta.get("rows").is_none());
    }

    #[test]
    fn series_metadata_is_source_only_without_values() {
        let series = polars::prelude::Series::new("x".into(), &[1i64, 2, 3]);
        let meta = series_metadata(&series);
        assert_eq!(meta["dataType"], "series");
        assert_eq!(meta["length"], 3);
        assert!(meta.get("values").is_none());
    }
}
