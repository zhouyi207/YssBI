use crate::execution::{
    ResultSource, ResultSourceRecord, SourceDescriptor, SourceId, SourceKind, SourcePresentation,
    SourceRenderer,
};
use crate::graph::register::catalog::dataframe::OLSResult;
use crate::graph::value::{DataType, DataValue};
use polars::prelude::{DataFrame, Series};
use serde_json::json;
use std::any::Any;
use std::sync::Arc;

pub fn build_dataframe_source(
    source_id: SourceId,
    title: impl Into<String>,
    df: Arc<DataFrame>,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    let columns: Vec<String> = df.columns().iter().map(|c| c.name().to_string()).collect();
    ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::Dataframe,
            SourceRenderer::Dataframe,
            title,
            execution_time_ms,
        )
        .with_columns(columns, df.height()),
        source: ResultSource::DataFrame(df),
    }
}

pub fn build_series_source(
    source_id: SourceId,
    title: impl Into<String>,
    series: Series,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    let name = series.name().to_string();
    let dtype = format!("{:?}", series.dtype());
    let length = series.len();
    ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::Series,
            SourceRenderer::Series,
            title,
            execution_time_ms,
        )
        .with_series(name, dtype, length),
        source: ResultSource::Series(series),
    }
}

pub fn build_json_source_from_data_value(
    source_id: SourceId,
    title: impl Into<String>,
    value: &DataValue,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    match value {
        DataValue::Null => ResultSourceRecord {
            descriptor: base_descriptor(
                source_id,
                SourceKind::Null,
                SourceRenderer::Null,
                title,
                execution_time_ms,
            )
            .with_message("No data".to_string()),
            source: ResultSource::Json(json!({"message": "No data"})),
        },
        scalar => {
            let value_type = format!("{:?}", scalar.value_type().unwrap_or(DataType::Any));
            ResultSourceRecord {
                descriptor: base_descriptor(
                    source_id,
                    SourceKind::Scalar,
                    SourceRenderer::Scalar,
                    title,
                    execution_time_ms,
                )
                .with_value_type(value_type.clone()),
                source: ResultSource::Json(json!({
                    "value": data_value_to_json(scalar),
                    "valueType": value_type,
                })),
            }
        }
    }
}

pub fn build_struct_source(
    source_id: SourceId,
    type_key: &str,
    handle_id: &str,
    handle: Option<Arc<dyn Any + Send + Sync>>,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
    if type_key == "OLSResult" {
        let handle = handle.ok_or_else(|| format!("Handle '{}' not found", handle_id))?;
        let ols = handle
            .downcast::<OLSResult>()
            .map_err(|_| format!("Handle '{}' is not OLSResult", handle_id))?;
        let structured = serde_json::to_value(ols.as_ref())
            .map_err(|e| format!("Failed to serialize OLSResult: {}", e))?;
        return Ok(ResultSourceRecord {
            descriptor: base_descriptor(
                source_id,
                SourceKind::Struct,
                SourceRenderer::StructOls,
                ols.title.clone(),
                execution_time_ms,
            )
            .with_struct(
                type_key.to_string(),
                handle_id.to_string(),
                "ols_result".to_string(),
            ),
            source: ResultSource::Json(json!({
                "structured": structured,
                "typeKey": type_key,
                "handleId": handle_id,
                "structKind": "ols_result",
            })),
        });
    }

    Ok(ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::Struct,
            SourceRenderer::StructGeneric,
            format!("View: {}", type_key),
            execution_time_ms,
        )
        .with_message(format!(
            "Struct type '{}' has no dedicated viewer yet.",
            type_key
        ))
        .with_struct(
            type_key.to_string(),
            handle_id.to_string(),
            "unknown".to_string(),
        ),
        source: ResultSource::Json(json!({
            "typeKey": type_key,
            "handleId": handle_id,
            "structKind": "unknown",
            "message": format!("Struct type '{}' has no dedicated viewer yet.", type_key),
        })),
    })
}

pub fn build_window_source_record(
    source_id: SourceId,
    window_type: &str,
    payload_json: &str,
    execution_time_ms: Option<u64>,
) -> Result<(ResultSourceRecord, SourcePresentation), String> {
    let mut payload = serde_json::from_str::<serde_json::Value>(payload_json)
        .map_err(|e| format!("Invalid window payload JSON: {}", e))?;
    if let Some(ms) = execution_time_ms {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("executionTimeMs".to_string(), json!(ms));
        }
    }

    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| default_window_title(window_type))
        .to_string();

    let descriptor = descriptor_from_payload(source_id.clone(), window_type, &payload, &title);
    let presentation = build_presentation(source_id.clone(), window_type, &descriptor);
    let record = ResultSourceRecord {
        descriptor,
        source: ResultSource::Json(payload),
    };
    Ok((record, presentation))
}

fn descriptor_from_payload(
    source_id: SourceId,
    window_type: &str,
    payload: &serde_json::Value,
    title: &str,
) -> SourceDescriptor {
    let renderer = match window_type {
        "scatter" | "line" | "plot" | "ecdf" | "kde" | "histogram" | "correlation"
        | "correlogram" => SourceRenderer::Plot,
        _ => SourceRenderer::Info,
    };

    SourceDescriptor {
        source_id,
        kind: SourceKind::Json,
        renderer,
        title: title.to_string(),
        message: payload
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        execution_time_ms: payload.get("executionTimeMs").and_then(|v| v.as_u64()),
        columns: None,
        total_rows: None,
        name: None,
        dtype: None,
        length: None,
        value_type: None,
        type_key: None,
        handle_id: None,
        struct_kind: None,
    }
}

pub fn build_presentation(
    source_id: SourceId,
    window_type: &str,
    descriptor: &SourceDescriptor,
) -> SourcePresentation {
    let is_plot = matches!(descriptor.renderer, SourceRenderer::Plot);
    let route = if is_plot {
        "/plot"
    } else if window_type == "data_view" {
        "/dataview"
    } else {
        "/info"
    };
    SourcePresentation {
        source_id,
        route: route.to_string(),
        window_title: descriptor.title.clone(),
        plot_type: is_plot.then(|| window_type.to_string()),
    }
}

fn base_descriptor(
    source_id: SourceId,
    kind: SourceKind,
    renderer: SourceRenderer,
    title: impl Into<String>,
    execution_time_ms: Option<u64>,
) -> SourceDescriptor {
    SourceDescriptor {
        source_id,
        kind,
        renderer,
        title: title.into(),
        message: None,
        execution_time_ms,
        columns: None,
        total_rows: None,
        name: None,
        dtype: None,
        length: None,
        value_type: None,
        type_key: None,
        handle_id: None,
        struct_kind: None,
    }
}

trait DescriptorExt {
    fn with_columns(self, columns: Vec<String>, total_rows: usize) -> Self;
    fn with_series(self, name: String, dtype: String, length: usize) -> Self;
    fn with_value_type(self, value_type: String) -> Self;
    fn with_message(self, message: String) -> Self;
    fn with_struct(self, type_key: String, handle_id: String, struct_kind: String) -> Self;
}

impl DescriptorExt for SourceDescriptor {
    fn with_columns(mut self, columns: Vec<String>, total_rows: usize) -> Self {
        self.columns = Some(columns);
        self.total_rows = Some(total_rows);
        self
    }

    fn with_series(mut self, name: String, dtype: String, length: usize) -> Self {
        self.name = Some(name);
        self.dtype = Some(dtype);
        self.length = Some(length);
        self
    }

    fn with_value_type(mut self, value_type: String) -> Self {
        self.value_type = Some(value_type);
        self
    }

    fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    fn with_struct(mut self, type_key: String, handle_id: String, struct_kind: String) -> Self {
        self.type_key = Some(type_key);
        self.handle_id = Some(handle_id);
        self.struct_kind = Some(struct_kind);
        self
    }
}

fn data_value_to_json(v: &DataValue) -> serde_json::Value {
    match v {
        DataValue::Null => serde_json::Value::Null,
        DataValue::Boolean(b) => serde_json::Value::Bool(*b),
        DataValue::Int32(i) => json!(i),
        DataValue::Int64(i) => json!(i),
        DataValue::Float32(f) => serde_json::Number::from_f64(*f as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        DataValue::Float64(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        DataValue::String(s) => serde_json::Value::String(s.clone()),
        DataValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(data_value_to_json).collect())
        }
        DataValue::Object(obj) => serde_json::Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), data_value_to_json(v)))
                .collect(),
        ),
        other => serde_json::Value::String(format!("{:?}", other)),
    }
}

fn default_window_title(window_type: &str) -> &str {
    match window_type {
        "data_view" => "Data View",
        "scatter" => "Scatter Plot",
        "line" => "Line Plot",
        "ecdf" => "ECDF Plot",
        "kde" => "KDE Plot",
        "histogram" => "Histogram",
        "correlation" => "Correlation Plot",
        "correlogram" => "Correlogram",
        _ => "Results",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::SourceRenderer;

    #[test]
    fn scalar_data_value_builds_typed_descriptor() {
        let record = build_json_source_from_data_value(
            "source".to_string(),
            "Scalar",
            &DataValue::Int64(7),
            Some(12),
        );

        assert_eq!(record.descriptor.kind, SourceKind::Scalar);
        assert_eq!(record.descriptor.renderer, SourceRenderer::Scalar);
        assert_eq!(record.descriptor.execution_time_ms, Some(12));
    }

    #[test]
    fn plot_payload_builds_presentation_separate_from_descriptor() {
        let (record, presentation) = build_window_source_record(
            "plot-1".to_string(),
            "scatter",
            r#"{"title":"Scatter","points":[[1,2]]}"#,
            None,
        )
        .unwrap();

        assert_eq!(record.descriptor.renderer, SourceRenderer::Plot);
        assert_eq!(presentation.route, "/plot");
        assert_eq!(presentation.plot_type.as_deref(), Some("scatter"));
    }
}
