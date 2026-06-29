use polars::prelude::{DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use yss_sci::api::database::anyvalue_to_json;

/// Typed source attached to a window metadata key.
#[derive(Clone)]
pub enum WindowDataSource {
    Json(String),
    DataFrame(Arc<DataFrame>),
    Series(Series),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSourceMetadata {
    pub source_id: String,
    pub window_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowDataPageResponse {
    pub kind: String,
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

/// Temporary store for child-window sources. `get_window_data` returns metadata only;
/// source values/pages are fetched through typed commands.
#[derive(Clone)]
pub struct WindowDataStore {
    metadata: Arc<Mutex<HashMap<String, String>>>,
    sources: Arc<Mutex<HashMap<String, WindowDataSource>>>,
}

impl WindowDataStore {
    pub fn new() -> Self {
        Self {
            metadata: Arc::new(Mutex::new(HashMap::new())),
            sources: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert_json_window(
        &self,
        key: String,
        window_type: String,
        value: String,
    ) -> Result<(), String> {
        let metadata = build_json_metadata(&key, &window_type, &value)?;
        self.metadata.lock().unwrap().insert(key.clone(), metadata);
        self.sources
            .lock()
            .unwrap()
            .insert(key, WindowDataSource::Json(value));
        Ok(())
    }

    pub fn insert_source(&self, key: String, source: WindowDataSource) {
        self.sources.lock().unwrap().insert(key, source);
    }

    pub fn insert_source_window(
        &self,
        key: String,
        window_type: String,
        metadata: String,
        source: WindowDataSource,
    ) -> Result<(), String> {
        let metadata = inject_source_metadata(&metadata, &key, &window_type)?;
        self.metadata.lock().unwrap().insert(key.clone(), metadata);
        self.sources.lock().unwrap().insert(key, source);
        Ok(())
    }

    /// Non-destructive metadata read (React Strict Mode double-mount safe).
    pub fn get(&self, key: &str) -> Option<String> {
        self.metadata.lock().unwrap().get(key).cloned()
    }

    pub fn has_tabular_source(&self, key: &str) -> bool {
        matches!(
            self.sources.lock().unwrap().get(key),
            Some(WindowDataSource::DataFrame(_) | WindowDataSource::Series(_))
        )
    }

    pub fn get_source_value(&self, key: &str) -> Result<Option<String>, String> {
        let sources = self.sources.lock().unwrap();
        match sources.get(key) {
            Some(WindowDataSource::Json(value)) => Ok(Some(value.clone())),
            Some(WindowDataSource::DataFrame(_) | WindowDataSource::Series(_)) => Err(format!(
                "Window source '{}' is paged, not a JSON value",
                key
            )),
            None => Ok(None),
        }
    }

    pub fn get_page(
        &self,
        key: &str,
        offset: usize,
        limit: usize,
    ) -> Result<WindowDataPageResponse, String> {
        let limit = limit.max(1);
        let sources = self.sources.lock().unwrap();
        let source = sources
            .get(key)
            .ok_or_else(|| format!("No tabular data source for window key '{}'", key))?;

        match source {
            WindowDataSource::DataFrame(df) => {
                let total_count = df.height();
                let start = offset.min(total_count);
                let end = (offset.saturating_add(limit)).min(total_count);
                let sliced = df.slice(start as i64, (end - start) as usize);
                let columns: Vec<String> =
                    df.columns().iter().map(|c| c.name().to_string()).collect();
                let rows = dataframe_rows_to_json(&sliced);
                Ok(WindowDataPageResponse {
                    kind: "dataframe".to_string(),
                    offset: start,
                    limit,
                    total_count,
                    columns: Some(columns),
                    rows: Some(rows),
                    values: None,
                })
            }
            WindowDataSource::Series(series) => {
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
                Ok(WindowDataPageResponse {
                    kind: "series".to_string(),
                    offset: start,
                    limit,
                    total_count,
                    columns: None,
                    rows: None,
                    values: Some(values),
                })
            }
            WindowDataSource::Json(_) => Err(format!(
                "Window source '{}' is JSON, not a tabular source",
                key
            )),
        }
    }

    pub fn remove(&self, key: &str) {
        self.metadata.lock().unwrap().remove(key);
        self.sources.lock().unwrap().remove(key);
    }
}

fn build_json_metadata(key: &str, window_type: &str, value: &str) -> Result<String, String> {
    let parsed = serde_json::from_str::<serde_json::Value>(value)
        .map_err(|e| format!("Invalid window JSON for '{}': {}", window_type, e))?;

    let title = parsed
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| default_window_title(window_type))
        .to_string();
    let execution_time_ms = parsed.get("executionTimeMs").and_then(|v| v.as_u64());

    let metadata = if parsed.get("viewType").and_then(|v| v.as_str()) == Some("data_view") {
        WindowSourceMetadata {
            source_id: key.to_string(),
            window_type: window_type.to_string(),
            view_type: Some("data_view".to_string()),
            data_type: parsed
                .get("dataType")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            renderer: parsed
                .get("structKind")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| {
                    parsed
                        .get("dataType")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                }),
            title,
            message: parsed
                .get("message")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            columns: None,
            total_rows: None,
            name: parsed
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            dtype: parsed
                .get("dtype")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            length: parsed
                .get("length")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize),
            value_type: parsed
                .get("valueType")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            type_key: parsed
                .get("typeKey")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            handle_id: parsed
                .get("handleId")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            struct_kind: parsed
                .get("structKind")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            execution_time_ms,
        }
    } else {
        WindowSourceMetadata {
            source_id: key.to_string(),
            window_type: window_type.to_string(),
            view_type: Some("window_source".to_string()),
            data_type: Some("json".to_string()),
            renderer: Some(window_type.to_string()),
            title,
            message: None,
            columns: None,
            total_rows: None,
            name: None,
            dtype: None,
            length: None,
            value_type: None,
            type_key: None,
            handle_id: None,
            struct_kind: None,
            execution_time_ms,
        }
    };

    serde_json::to_string(&metadata).map_err(|e| e.to_string())
}

fn inject_source_metadata(metadata: &str, key: &str, window_type: &str) -> Result<String, String> {
    let mut parsed = serde_json::from_str::<serde_json::Value>(metadata)
        .map_err(|e| format!("Invalid window metadata JSON: {}", e))?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| "Window metadata must be a JSON object".to_string())?;
    obj.insert("sourceId".to_string(), serde_json::json!(key));
    obj.insert("windowType".to_string(), serde_json::json!(window_type));
    serde_json::to_string(&parsed).map_err(|e| e.to_string())
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

    #[test]
    fn metadata_reads_remain_non_destructive() {
        let store = WindowDataStore::new();
        store
            .insert_json_window(
                "k".to_string(),
                "ols_summary".to_string(),
                "{\"title\":\"Result\",\"a\":1}".to_string(),
            )
            .unwrap();
        let metadata = store.get("k").unwrap();
        assert!(metadata.contains("\"sourceId\":\"k\""));
        assert_eq!(store.get("k").as_deref(), Some(metadata.as_str()));
        assert_eq!(
            store.get_source_value("k").unwrap().as_deref(),
            Some("{\"title\":\"Result\",\"a\":1}")
        );
    }

    #[test]
    fn dataframe_page_slices_rows() {
        let df = df!(
            "a" => [1, 2, 3, 4, 5],
            "b" => ["x", "y", "z", "w", "v"]
        )
        .unwrap();
        let store = WindowDataStore::new();
        store.insert_source("k".to_string(), WindowDataSource::DataFrame(Arc::new(df)));

        let page = store.get_page("k", 1, 2).unwrap();
        assert_eq!(page.kind, "dataframe");
        assert_eq!(page.offset, 1);
        assert_eq!(page.total_count, 5);
        assert_eq!(page.rows.as_ref().unwrap().len(), 2);
        assert_eq!(page.rows.as_ref().unwrap()[0][0], serde_json::json!(2));
    }

    #[test]
    fn series_page_slices_values() {
        let series = Series::new("s".into(), &[10i64, 20, 30, 40]);
        let store = WindowDataStore::new();
        store.insert_source("k".to_string(), WindowDataSource::Series(series));

        let page = store.get_page("k", 2, 10).unwrap();
        assert_eq!(page.kind, "series");
        assert_eq!(page.offset, 2);
        assert_eq!(page.total_count, 4);
        assert_eq!(page.values.as_ref().unwrap().len(), 2);
    }
}
