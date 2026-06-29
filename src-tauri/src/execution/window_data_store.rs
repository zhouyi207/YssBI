use polars::prelude::{DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use yss_sci::api::database::anyvalue_to_json;

pub type SourceId = String;

/// Typed source data retained in the backend for lazy frontend reads.
#[derive(Clone)]
pub enum ResultSource {
    Json(serde_json::Value),
    DataFrame(Arc<DataFrame>),
    Series(Series),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Json,
    Dataframe,
    Series,
    Scalar,
    Null,
    Struct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRenderer {
    Dataframe,
    Series,
    Scalar,
    Null,
    StructOls,
    StructGeneric,
    Plot,
    Info,
}

/// Data-only descriptor. Window routing and layout live in `SourcePresentation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDescriptor {
    pub source_id: SourceId,
    pub kind: SourceKind,
    pub renderer: SourceRenderer,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePresentation {
    pub source_id: SourceId,
    pub route: String,
    pub window_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceValue {
    pub kind: SourceKind,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePage {
    pub kind: SourceKind,
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<serde_json::Value>>,
}

#[derive(Clone)]
pub struct ResultSourceRecord {
    pub descriptor: SourceDescriptor,
    pub source: ResultSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SourceOwner {
    Window,
    RuntimePin {
        graph_id: String,
        pin_id: String,
        run_id: String,
    },
}

#[derive(Default)]
struct ResultSourceRegistry {
    descriptors: HashMap<SourceId, SourceDescriptor>,
    sources: HashMap<SourceId, ResultSource>,
    runtime_index: HashMap<(String, String), SourceId>,
    owners: HashMap<SourceId, SourceOwner>,
}

/// Session-scoped registry for all inspectable execution results.
#[derive(Clone, Default)]
pub struct ResultSourceStore {
    registry: Arc<RwLock<ResultSourceRegistry>>,
}

impl ResultSourceStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_window_source(&self, record: ResultSourceRecord) {
        let source_id = record.descriptor.source_id.clone();
        let mut registry = self.registry.write().unwrap();
        registry
            .descriptors
            .insert(source_id.clone(), record.descriptor);
        registry.sources.insert(source_id.clone(), record.source);
        registry.owners.insert(source_id, SourceOwner::Window);
    }

    pub fn insert_runtime_pin_source(
        &self,
        graph_id: String,
        pin_id: String,
        run_id: String,
        record: ResultSourceRecord,
    ) -> SourceDescriptor {
        let source_id = record.descriptor.source_id.clone();
        let descriptor = record.descriptor.clone();
        let mut registry = self.registry.write().unwrap();
        if let Some(previous) = registry
            .runtime_index
            .insert((graph_id.clone(), pin_id.clone()), source_id.clone())
        {
            registry.descriptors.remove(&previous);
            registry.sources.remove(&previous);
            registry.owners.remove(&previous);
        }
        registry
            .descriptors
            .insert(source_id.clone(), record.descriptor);
        registry.sources.insert(source_id.clone(), record.source);
        registry.owners.insert(
            source_id,
            SourceOwner::RuntimePin {
                graph_id,
                pin_id,
                run_id,
            },
        );
        descriptor
    }

    pub fn get_descriptor(&self, source_id: &str) -> Option<SourceDescriptor> {
        self.registry
            .read()
            .unwrap()
            .descriptors
            .get(source_id)
            .cloned()
    }

    pub fn get_pin_descriptor(&self, graph_id: &str, pin_id: &str) -> Option<SourceDescriptor> {
        let registry = self.registry.read().unwrap();
        let source_id = registry
            .runtime_index
            .get(&(graph_id.to_string(), pin_id.to_string()))?;
        registry.descriptors.get(source_id).cloned()
    }

    pub fn get_value(&self, source_id: &str) -> Result<Option<SourceValue>, String> {
        let (descriptor, source) = {
            let registry = self.registry.read().unwrap();
            let descriptor = match registry.descriptors.get(source_id) {
                Some(descriptor) => descriptor.clone(),
                None => return Ok(None),
            };
            let source = registry.sources.get(source_id).cloned();
            (descriptor, source)
        };

        match source {
            Some(ResultSource::Json(value)) => Ok(Some(json_to_source_value(&descriptor, value))),
            Some(ResultSource::DataFrame(_) | ResultSource::Series(_)) => Err(format!(
                "Result source '{}' is paged, not a JSON value",
                source_id
            )),
            None => Ok(None),
        }
    }

    pub fn get_page(
        &self,
        source_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SourcePage, String> {
        let limit = limit.max(1);
        let source = {
            let registry = self.registry.read().unwrap();
            registry.sources.get(source_id).cloned()
        }
        .ok_or_else(|| format!("No data source for source id '{}'", source_id))?;

        match source {
            ResultSource::DataFrame(df) => dataframe_page(&df, offset, limit),
            ResultSource::Series(series) => series_page(&series, offset, limit),
            ResultSource::Json(_) => Err(format!(
                "Result source '{}' is JSON, not a tabular source",
                source_id
            )),
        }
    }

    pub fn clear_runtime_graph(&self, graph_id: &str) {
        let mut registry = self.registry.write().unwrap();
        let source_ids: Vec<_> = registry
            .owners
            .iter()
            .filter_map(|(source_id, owner)| match owner {
                SourceOwner::RuntimePin {
                    graph_id: owner_graph,
                    ..
                } if owner_graph == graph_id => Some(source_id.clone()),
                _ => None,
            })
            .collect();
        for source_id in source_ids {
            registry.descriptors.remove(&source_id);
            registry.sources.remove(&source_id);
            registry.owners.remove(&source_id);
        }
        registry
            .runtime_index
            .retain(|(owner_graph_id, _), _| owner_graph_id != graph_id);
    }

    pub fn clear_all(&self) {
        let mut registry = self.registry.write().unwrap();
        registry.descriptors.clear();
        registry.sources.clear();
        registry.runtime_index.clear();
        registry.owners.clear();
    }

    pub fn remove(&self, source_id: &str) {
        let mut registry = self.registry.write().unwrap();
        registry.descriptors.remove(source_id);
        registry.sources.remove(source_id);
        registry.owners.remove(source_id);
        registry.runtime_index.retain(|_, v| v != source_id);
    }
}

fn json_to_source_value(descriptor: &SourceDescriptor, json: serde_json::Value) -> SourceValue {
    SourceValue {
        kind: descriptor.kind.clone(),
        title: descriptor.title.clone(),
        message: json
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| descriptor.message.clone()),
        value: json.get("value").cloned().or_else(|| {
            if descriptor.kind == SourceKind::Json || descriptor.renderer == SourceRenderer::Plot {
                Some(json.clone())
            } else {
                None
            }
        }),
        value_type: json
            .get("valueType")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| descriptor.value_type.clone()),
        type_key: descriptor.type_key.clone(),
        handle_id: descriptor.handle_id.clone(),
        struct_kind: descriptor.struct_kind.clone(),
        structured: json.get("structured").cloned(),
    }
}

fn dataframe_page(df: &DataFrame, offset: usize, limit: usize) -> Result<SourcePage, String> {
    let total_count = df.height();
    let start = offset.min(total_count);
    let end = (offset.saturating_add(limit)).min(total_count);
    let sliced = df.slice(start as i64, end - start);
    let columns: Vec<String> = df.columns().iter().map(|c| c.name().to_string()).collect();
    let rows = dataframe_rows_to_json(&sliced);
    Ok(SourcePage {
        kind: SourceKind::Dataframe,
        offset: start,
        limit,
        total_count,
        columns: Some(columns),
        rows: Some(rows),
        values: None,
    })
}

fn series_page(series: &Series, offset: usize, limit: usize) -> Result<SourcePage, String> {
    let total_count = series.len();
    let start = offset.min(total_count);
    let end = (offset.saturating_add(limit)).min(total_count);
    let values: Vec<serde_json::Value> = (start..end)
        .map(|i| {
            series
                .get(i)
                .map(anyvalue_to_json)
                .unwrap_or(serde_json::Value::Null)
        })
        .collect();
    Ok(SourcePage {
        kind: SourceKind::Series,
        offset: start,
        limit,
        total_count,
        columns: None,
        rows: None,
        values: Some(values),
    })
}

fn dataframe_rows_to_json(df: &DataFrame) -> Vec<Vec<serde_json::Value>> {
    (0..df.height())
        .map(|row_idx| {
            df.columns()
                .iter()
                .map(|s| {
                    s.get(row_idx)
                        .map(anyvalue_to_json)
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::{NamedFrom, Series, df};

    fn descriptor(source_id: &str, kind: SourceKind, renderer: SourceRenderer) -> SourceDescriptor {
        SourceDescriptor {
            source_id: source_id.to_string(),
            kind,
            renderer,
            title: "Result".to_string(),
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
        }
    }

    #[test]
    fn descriptors_are_typed_objects() {
        let store = ResultSourceStore::new();
        store.insert_window_source(ResultSourceRecord {
            descriptor: descriptor("source-1", SourceKind::Scalar, SourceRenderer::Scalar),
            source: ResultSource::Json(serde_json::json!({"value": 42, "valueType": "Int64"})),
        });

        let descriptor = store.get_descriptor("source-1").unwrap();
        assert_eq!(descriptor.source_id, "source-1");
        assert_eq!(descriptor.kind, SourceKind::Scalar);
        assert_eq!(descriptor.renderer, SourceRenderer::Scalar);
    }

    #[test]
    fn runtime_pin_sources_replace_previous_index_entry() {
        let store = ResultSourceStore::new();
        store.insert_runtime_pin_source(
            "graph".to_string(),
            "pin".to_string(),
            "run-a".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-a", SourceKind::Scalar, SourceRenderer::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 1})),
            },
        );
        store.insert_runtime_pin_source(
            "graph".to_string(),
            "pin".to_string(),
            "run-b".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-b", SourceKind::Scalar, SourceRenderer::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 2})),
            },
        );

        assert!(store.get_descriptor("runtime-a").is_none());
        assert_eq!(
            store
                .get_pin_descriptor("graph", "pin")
                .unwrap()
                .source_id
                .as_str(),
            "runtime-b"
        );
    }

    #[test]
    fn dataframe_page_slices_rows() {
        let df = df!(
            "a" => [1, 2, 3, 4, 5],
            "b" => ["x", "y", "z", "w", "v"]
        )
        .unwrap();
        let store = ResultSourceStore::new();
        store.insert_window_source(ResultSourceRecord {
            descriptor: descriptor("k", SourceKind::Dataframe, SourceRenderer::Dataframe),
            source: ResultSource::DataFrame(Arc::new(df)),
        });

        let page = store.get_page("k", 1, 2).unwrap();
        assert_eq!(page.kind, SourceKind::Dataframe);
        assert_eq!(page.offset, 1);
        assert_eq!(page.total_count, 5);
        assert_eq!(page.rows.as_ref().unwrap().len(), 2);
        assert_eq!(page.rows.as_ref().unwrap()[0][0], serde_json::json!(2));
    }

    #[test]
    fn series_page_slices_values() {
        let series = Series::new("s".into(), &[10i64, 20, 30, 40]);
        let store = ResultSourceStore::new();
        store.insert_window_source(ResultSourceRecord {
            descriptor: descriptor("k", SourceKind::Series, SourceRenderer::Series),
            source: ResultSource::Series(series),
        });

        let page = store.get_page("k", 2, 10).unwrap();
        assert_eq!(page.kind, SourceKind::Series);
        assert_eq!(page.offset, 2);
        assert_eq!(page.total_count, 4);
        assert_eq!(page.values.as_ref().unwrap().len(), 2);
    }
}
