use crate::execution::serialize_struct_handle;
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

pub fn data_series_table_columns(data_series: &Series) -> Vec<String> {
    let name = data_series.name();
    let value_col = if name.is_empty() {
        "value".to_string()
    } else {
        name.to_string()
    };
    vec!["#".to_string(), value_col]
}

pub fn build_data_series_source(
    source_id: SourceId,
    title: impl Into<String>,
    data_series: Series,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    let name = data_series.name().to_string();
    let dtype = format!("{:?}", data_series.dtype());
    let length = data_series.len();
    let columns = data_series_table_columns(&data_series);
    ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::DataSeries,
            SourceRenderer::DataSeries,
            title,
            execution_time_ms,
        )
        .with_data_series(name, dtype, length)
        .with_columns(columns, length),
        source: ResultSource::DataSeries(data_series),
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
        DataValue::Array(_) | DataValue::Object(_) => {
            build_json_tree_source(source_id, title, value, execution_time_ms)
        }
        scalar if is_scalar_data_value(scalar) => {
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
        other => build_json_tree_source(source_id, title, other, execution_time_ms),
    }
}

pub fn build_struct_source(
    source_id: SourceId,
    type_key: &str,
    handle_id: &str,
    handle: Option<Arc<dyn Any + Send + Sync>>,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
    let title = struct_source_title(type_key, handle.as_ref());
    let value = serialize_struct_value(type_key, handle_id, handle.as_ref())?;
    Ok(build_json_payload_tree_source(
        source_id,
        title,
        &json!({
            "value": value,
            "valueType": type_key,
            "typeKey": type_key,
            "handleId": handle_id,
        }),
        execution_time_ms,
    )
    .with_struct_meta(type_key, handle_id))
}

/// Resolved runtime payload used to build a result source without graph callbacks.
pub enum ResolvedSourceValue {
    Null,
    DataFrame(Arc<DataFrame>),
    DataSeries(Series),
    Struct {
        type_key: String,
        handle_id: String,
        handle: Option<Arc<dyn Any + Send + Sync>>,
    },
    Value(DataValue),
}

pub fn build_source_from_resolved(
    source_id: SourceId,
    title: impl Into<String>,
    value: &DataValue,
    resolved: ResolvedSourceValue,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
    let title = title.into();
    match resolved {
        ResolvedSourceValue::Null => Ok(build_json_source_from_data_value(
            source_id,
            if title.is_empty() {
                default_view_title(value, None)
            } else {
                title
            },
            value,
            execution_time_ms,
        )),
        ResolvedSourceValue::DataFrame(df) => Ok(build_dataframe_source(
            source_id,
            if title.is_empty() {
                default_view_title(value, None)
            } else {
                title
            },
            df,
            execution_time_ms,
        )),
        ResolvedSourceValue::DataSeries(data_series) => {
            let resolved_title = if title.is_empty() {
                default_view_title(value, Some(&data_series))
            } else {
                title
            };
            Ok(build_data_series_source(
                source_id,
                resolved_title,
                data_series,
                execution_time_ms,
            ))
        }
        ResolvedSourceValue::Struct {
            type_key,
            handle_id,
            handle,
        } => build_struct_source(source_id, &type_key, &handle_id, handle, execution_time_ms).map(
            |record| {
                if title.is_empty() {
                    record
                } else {
                    ResultSourceRecord {
                        descriptor: SourceDescriptor {
                            title,
                            ..record.descriptor
                        },
                        source: record.source,
                    }
                }
            },
        ),
        ResolvedSourceValue::Value(other) => Ok(build_json_source_from_data_value(
            source_id,
            if title.is_empty() {
                default_view_title(&other, None)
            } else {
                title
            },
            &other,
            execution_time_ms,
        )),
    }
}

/// Build a result source record from a runtime `DataValue`, resolving handles through callbacks.
pub fn build_source_from_data_value(
    source_id: SourceId,
    title: impl Into<String>,
    value: &DataValue,
    execution_time_ms: Option<u64>,
    get_dataframe: &mut dyn FnMut(&str) -> Result<Arc<DataFrame>, String>,
    get_data_series: &dyn Fn(&str) -> Result<Series, String>,
    get_handle: &dyn Fn(&str) -> Option<Arc<dyn Any + Send + Sync>>,
) -> Result<ResultSourceRecord, String> {
    let resolved = match value {
        DataValue::Null => ResolvedSourceValue::Null,
        DataValue::DataFrame(id) => ResolvedSourceValue::DataFrame(get_dataframe(id)?),
        DataValue::DataSeries(v) => ResolvedSourceValue::DataSeries(get_data_series(&v.id)?),
        DataValue::Struct {
            type_key,
            handle_id,
        } => ResolvedSourceValue::Struct {
            type_key: type_key.clone(),
            handle_id: handle_id.clone(),
            handle: get_handle(handle_id),
        },
        other => ResolvedSourceValue::Value(other.clone()),
    };
    build_source_from_resolved(source_id, title, value, resolved, execution_time_ms)
}

pub fn default_view_title(value: &DataValue, series: Option<&Series>) -> String {
    match value {
        DataValue::Null => "View: (null)".to_string(),
        DataValue::DataFrame(_) => "View: DataFrame".to_string(),
        DataValue::DataSeries(_) => {
            let name = series.map(|s| s.name().to_string()).unwrap_or_default();
            if name.is_empty() {
                "View: DataSeries".to_string()
            } else {
                format!("View: {}", name)
            }
        }
        DataValue::Struct { type_key, .. } => format!("View: {}", type_key),
        _ => "View".to_string(),
    }
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
    } else if window_type == "runtime_view" {
        "/view"
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

fn build_json_tree_source(
    source_id: SourceId,
    title: impl Into<String>,
    value: &DataValue,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    let value_type = format!("{:?}", value.value_type().unwrap_or(DataType::Any));
    ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::Json,
            SourceRenderer::Json,
            title,
            execution_time_ms,
        )
        .with_value_type(value_type.clone()),
        source: ResultSource::Json(json!({
            "value": data_value_to_json(value),
            "valueType": value_type,
        })),
    }
}

fn build_json_payload_tree_source(
    source_id: SourceId,
    title: impl Into<String>,
    payload: &serde_json::Value,
    execution_time_ms: Option<u64>,
) -> ResultSourceRecord {
    let value_type = payload
        .get("valueType")
        .and_then(|v| v.as_str())
        .unwrap_or("Json")
        .to_string();
    ResultSourceRecord {
        descriptor: base_descriptor(
            source_id,
            SourceKind::Json,
            SourceRenderer::Json,
            title,
            execution_time_ms,
        )
        .with_value_type(value_type),
        source: ResultSource::Json(payload.clone()),
    }
}

fn struct_source_title(type_key: &str, handle: Option<&Arc<dyn Any + Send + Sync>>) -> String {
    if let Some(handle) = handle {
        if type_key == "OLSResult" {
            if let Ok(ols) = handle.clone().downcast::<OLSResult>() {
                return ols.title.clone();
            }
        }
    }
    match type_key {
        "OLSModel" => "View: OLS Model".to_string(),
        "LogitModel" => "View: Logit Model".to_string(),
        "ProbitModel" => "View: Probit Model".to_string(),
        "PraisModel" => "View: Prais Model".to_string(),
        _ => format!("View: {}", type_key),
    }
}

fn serialize_struct_value(
    type_key: &str,
    handle_id: &str,
    handle: Option<&Arc<dyn Any + Send + Sync>>,
) -> Result<serde_json::Value, String> {
    let Some(handle) = handle else {
        return Ok(json!({
            "typeKey": type_key,
            "handleId": handle_id,
            "message": format!("Handle '{}' not found", handle_id),
        }));
    };

    if let Some(value) = serialize_struct_handle(type_key, handle) {
        return Ok(value);
    }

    Ok(json!({
        "typeKey": type_key,
        "handleId": handle_id,
        "message": format!("Struct type '{}' has no JSON serializer yet.", type_key),
    }))
}

fn is_scalar_data_value(value: &DataValue) -> bool {
    matches!(
        value,
        DataValue::Boolean(_)
            | DataValue::Int32(_)
            | DataValue::Int64(_)
            | DataValue::Float32(_)
            | DataValue::Float64(_)
            | DataValue::String(_)
    )
}

trait DescriptorExt {
    fn with_columns(self, columns: Vec<String>, total_rows: usize) -> Self;
    fn with_data_series(self, name: String, dtype: String, length: usize) -> Self;
    fn with_value_type(self, value_type: String) -> Self;
    fn with_message(self, message: String) -> Self;
    fn with_struct_meta(self, type_key: &str, handle_id: &str) -> Self;
}

impl DescriptorExt for SourceDescriptor {
    fn with_columns(mut self, columns: Vec<String>, total_rows: usize) -> Self {
        self.columns = Some(columns);
        self.total_rows = Some(total_rows);
        self.length = Some(total_rows);
        self
    }

    fn with_data_series(mut self, name: String, dtype: String, length: usize) -> Self {
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

    fn with_struct_meta(mut self, type_key: &str, handle_id: &str) -> Self {
        self.type_key = Some(type_key.to_string());
        self.handle_id = Some(handle_id.to_string());
        self
    }
}

trait ResultSourceRecordExt {
    fn with_struct_meta(self, type_key: &str, handle_id: &str) -> Self;
}

impl ResultSourceRecordExt for ResultSourceRecord {
    fn with_struct_meta(mut self, type_key: &str, handle_id: &str) -> Self {
        self.descriptor = self.descriptor.with_struct_meta(type_key, handle_id);
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
        "runtime_view" => "Runtime View",
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
    fn array_data_value_builds_json_renderer() {
        let record = build_json_source_from_data_value(
            "source".to_string(),
            "Array",
            &DataValue::Array(vec![DataValue::Int64(1), DataValue::Int64(2)]),
            None,
        );

        assert_eq!(record.descriptor.kind, SourceKind::Json);
        assert_eq!(record.descriptor.renderer, SourceRenderer::Json);
    }

    #[test]
    fn runtime_view_window_uses_view_route() {
        let descriptor = SourceDescriptor {
            source_id: "s".to_string(),
            kind: SourceKind::Json,
            renderer: SourceRenderer::Json,
            title: "View: OLS Model".to_string(),
            message: None,
            execution_time_ms: None,
            columns: None,
            total_rows: None,
            name: None,
            dtype: None,
            length: None,
            value_type: None,
            type_key: None,
            handle_id: None,
            struct_kind: None,
        };
        let presentation = build_presentation("s".to_string(), "runtime_view", &descriptor);
        assert_eq!(presentation.route, "/view");
        assert_eq!(presentation.window_title, "View: OLS Model");
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
