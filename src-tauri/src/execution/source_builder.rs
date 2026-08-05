use crate::execution::serialize_struct_handle;
use crate::execution::{
    PlotChart, Presentation, ReportKind, ResultSource, ResultSourceRecord, SourceDescriptor,
    SourceId, SourceKind,
};
use crate::graph::value::{DataType, DataValue};
use crate::sci::models::regression::OLSResult;
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
            Presentation::Inspector,
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
    vec![value_col]
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
            Presentation::Inspector,
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
                Presentation::Inspector,
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
                    Presentation::Inspector,
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

pub fn build_plot_source(
    source_id: SourceId,
    chart: PlotChart,
    payload_json: &str,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
    build_json_presentation_source(
        source_id,
        Presentation::Plot { chart },
        payload_json,
        execution_time_ms,
    )
}

pub fn build_report_source(
    source_id: SourceId,
    report: ReportKind,
    payload_json: &str,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
    build_json_presentation_source(
        source_id,
        Presentation::Report { report },
        payload_json,
        execution_time_ms,
    )
}

pub fn build_json_presentation_source(
    source_id: SourceId,
    presentation: Presentation,
    payload_json: &str,
    execution_time_ms: Option<u64>,
) -> Result<ResultSourceRecord, String> {
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
        .unwrap_or(presentation.default_title())
        .to_string();

    Ok(ResultSourceRecord {
        descriptor: json_descriptor(source_id, presentation, &payload, title, execution_time_ms),
        source: ResultSource::Json(payload),
    })
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

fn json_descriptor(
    source_id: SourceId,
    presentation: Presentation,
    payload: &serde_json::Value,
    title: String,
    execution_time_ms: Option<u64>,
) -> SourceDescriptor {
    SourceDescriptor {
        source_id,
        kind: SourceKind::Json,
        presentation,
        title,
        message: payload
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        execution_time_ms: payload
            .get("executionTimeMs")
            .and_then(|v| v.as_u64())
            .or(execution_time_ms),
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

fn base_descriptor(
    source_id: SourceId,
    kind: SourceKind,
    presentation: Presentation,
    title: impl Into<String>,
    execution_time_ms: Option<u64>,
) -> SourceDescriptor {
    SourceDescriptor {
        source_id,
        kind,
        presentation,
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
            Presentation::Inspector,
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
            Presentation::Inspector,
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
        DataValue::Boolean(_) | DataValue::Int64(_) | DataValue::Float64(_) | DataValue::String(_)
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
        DataValue::Int64(i) => json!(i),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_data_value_builds_typed_descriptor() {
        let record = build_json_source_from_data_value(
            "source".to_string(),
            "Scalar",
            &DataValue::Int64(7),
            Some(12),
        );

        assert_eq!(record.descriptor.kind, SourceKind::Scalar);
        assert_eq!(record.descriptor.presentation, Presentation::Inspector);
        assert_eq!(record.descriptor.execution_time_ms, Some(12));
    }

    #[test]
    fn array_data_value_builds_json_inspector() {
        let record = build_json_source_from_data_value(
            "source".to_string(),
            "Array",
            &DataValue::Array(vec![DataValue::Int64(1), DataValue::Int64(2)]),
            None,
        );

        assert_eq!(record.descriptor.kind, SourceKind::Json);
        assert_eq!(record.descriptor.presentation, Presentation::Inspector);
    }

    #[test]
    fn inspector_presentation_uses_view_route() {
        let descriptor = SourceDescriptor {
            source_id: "s".to_string(),
            kind: SourceKind::Json,
            presentation: Presentation::Inspector,
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
        assert_eq!(descriptor.presentation.route(), "/inspect");
    }

    #[test]
    fn plot_payload_builds_plot_presentation() {
        let record = build_plot_source(
            "plot-1".to_string(),
            PlotChart::Scatter,
            r#"{"title":"Scatter","points":[[1,2]]}"#,
            None,
        )
        .unwrap();

        assert_eq!(
            record.descriptor.presentation,
            Presentation::Plot {
                chart: PlotChart::Scatter
            }
        );
        assert_eq!(record.descriptor.presentation.route(), "/plot");
        assert_eq!(
            record
                .descriptor
                .presentation
                .plot_chart()
                .map(PlotChart::as_str),
            Some("scatter")
        );
    }
}
