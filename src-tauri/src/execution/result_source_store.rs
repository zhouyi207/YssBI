use polars::prelude::{DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use yss_sci::api::database::anyvalue_to_json;

use crate::execution::Presentation;
use crate::graph::PinId;

pub type SourceId = String;

/// Typed source data retained in the backend for lazy frontend reads.
#[derive(Clone)]
pub enum ResultSource {
    Json(serde_json::Value),
    DataFrame(Arc<DataFrame>),
    DataSeries(Series),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Json,
    Dataframe,
    #[serde(rename = "dataseries")]
    DataSeries,
    Scalar,
    Null,
    Struct,
}

/// Inspectable result metadata. Presentation defines how the frontend opens/renders it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDescriptor {
    pub source_id: SourceId,
    pub kind: SourceKind,
    pub presentation: Presentation,
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
            Some(ResultSource::DataFrame(_) | ResultSource::DataSeries(_)) => Err(format!(
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
            ResultSource::DataSeries(data_series) => data_series_page(&data_series, offset, limit),
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

    /// Remove runtime pin sources for the given pins on a graph. Returns pin ids actually removed.
    pub fn invalidate_runtime_pins(&self, graph_id: &str, pin_ids: &[PinId]) -> Vec<PinId> {
        if pin_ids.is_empty() {
            return Vec::new();
        }
        let mut registry = self.registry.write().unwrap();
        let mut invalidated = Vec::new();
        for pin_id in pin_ids {
            let key = (graph_id.to_string(), pin_id.to_string());
            let Some(source_id) = registry.runtime_index.remove(&key) else {
                continue;
            };
            registry.descriptors.remove(&source_id);
            registry.sources.remove(&source_id);
            registry.owners.remove(&source_id);
            invalidated.push(*pin_id);
        }
        invalidated
    }

    /// Release a window-owned source. No-op when owner is RuntimePin or source is missing.
    pub fn release_window_source(&self, source_id: &str) -> Result<bool, String> {
        let mut registry = self.registry.write().unwrap();
        match registry.owners.get(source_id) {
            Some(SourceOwner::Window) => {
                registry.descriptors.remove(source_id);
                registry.sources.remove(source_id);
                registry.owners.remove(source_id);
                registry.runtime_index.retain(|_, v| v != source_id);
                Ok(true)
            }
            Some(SourceOwner::RuntimePin { .. }) | None => Ok(false),
        }
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
            if descriptor.kind == SourceKind::Json
                || matches!(
                    descriptor.presentation,
                    Presentation::Inspector
                        | Presentation::Plot { .. }
                        | Presentation::Report { .. }
                )
            {
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

fn data_series_page(
    data_series: &Series,
    offset: usize,
    limit: usize,
) -> Result<SourcePage, String> {
    let total_count = data_series.len();
    let start = offset.min(total_count);
    let end = (offset.saturating_add(limit)).min(total_count);
    let columns = crate::execution::data_series_table_columns(data_series);
    let rows: Vec<Vec<serde_json::Value>> = (start..end)
        .map(|i| {
            vec![
                data_series
                    .get(i)
                    .map(anyvalue_to_json)
                    .unwrap_or(serde_json::Value::Null),
            ]
        })
        .collect();
    Ok(SourcePage {
        kind: SourceKind::DataSeries,
        offset: start,
        limit,
        total_count,
        columns: Some(columns),
        rows: Some(rows),
        values: None,
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

    fn descriptor(source_id: &str, kind: SourceKind) -> SourceDescriptor {
        SourceDescriptor {
            source_id: source_id.to_string(),
            kind,
            presentation: Presentation::Inspector,
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
            descriptor: descriptor("source-1", SourceKind::Scalar),
            source: ResultSource::Json(serde_json::json!({"value": 42, "valueType": "Int64"})),
        });

        let descriptor = store.get_descriptor("source-1").unwrap();
        assert_eq!(descriptor.source_id, "source-1");
        assert_eq!(descriptor.kind, SourceKind::Scalar);
        assert_eq!(descriptor.presentation, Presentation::Inspector);
    }

    #[test]
    fn runtime_pin_sources_replace_previous_index_entry() {
        let store = ResultSourceStore::new();
        store.insert_runtime_pin_source(
            "graph".to_string(),
            "pin".to_string(),
            "run-a".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-a", SourceKind::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 1})),
            },
        );
        store.insert_runtime_pin_source(
            "graph".to_string(),
            "pin".to_string(),
            "run-b".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-b", SourceKind::Scalar),
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
            descriptor: descriptor("k", SourceKind::Dataframe),
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
    fn data_series_page_slices_values() {
        let data_series = Series::new("s".into(), &[10i64, 20, 30, 40]);
        let store = ResultSourceStore::new();
        store.insert_window_source(ResultSourceRecord {
            descriptor: descriptor("k", SourceKind::DataSeries),
            source: ResultSource::DataSeries(data_series),
        });

        let page = store.get_page("k", 2, 10).unwrap();
        assert_eq!(page.kind, SourceKind::DataSeries);
        assert_eq!(page.offset, 2);
        assert_eq!(page.total_count, 4);
        assert_eq!(page.rows.as_ref().unwrap().len(), 2);
        assert_eq!(page.columns.as_ref().unwrap(), &vec!["s".to_string()]);
        assert_eq!(page.rows.as_ref().unwrap()[0][0], serde_json::json!(30));
    }

    #[test]
    fn invalidate_runtime_pins_only_removes_target_pins() {
        use uuid::Uuid;
        let pin_a = PinId::from(Uuid::new_v4());
        let pin_b = PinId::from(Uuid::new_v4());
        let store = ResultSourceStore::new();
        store.insert_runtime_pin_source(
            "graph".to_string(),
            pin_a.to_string(),
            "run".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-a", SourceKind::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 1})),
            },
        );
        store.insert_runtime_pin_source(
            "graph".to_string(),
            pin_b.to_string(),
            "run".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime-b", SourceKind::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 2})),
            },
        );

        let removed = store.invalidate_runtime_pins("graph", &[pin_a]);
        assert_eq!(removed, vec![pin_a]);
        assert!(
            store
                .get_pin_descriptor("graph", &pin_a.to_string())
                .is_none()
        );
        assert!(
            store
                .get_pin_descriptor("graph", &pin_b.to_string())
                .is_some()
        );
    }

    #[test]
    fn release_window_source_noop_for_runtime_pin_owner() {
        let store = ResultSourceStore::new();
        store.insert_runtime_pin_source(
            "graph".to_string(),
            "pin".to_string(),
            "run".to_string(),
            ResultSourceRecord {
                descriptor: descriptor("runtime", SourceKind::Scalar),
                source: ResultSource::Json(serde_json::json!({"value": 1})),
            },
        );
        assert!(!store.release_window_source("runtime").unwrap());
        assert!(store.get_descriptor("runtime").is_some());
    }

    #[test]
    fn release_window_source_removes_window_owner() {
        let store = ResultSourceStore::new();
        store.insert_window_source(ResultSourceRecord {
            descriptor: descriptor("window-1", SourceKind::Scalar),
            source: ResultSource::Json(serde_json::json!({"value": 42})),
        });
        assert!(store.release_window_source("window-1").unwrap());
        assert!(store.get_descriptor("window-1").is_none());
    }
}
