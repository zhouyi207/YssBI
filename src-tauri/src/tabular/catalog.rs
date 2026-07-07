use crate::database::DatabaseInstance;
use crate::execution::ExecutionDataStore;
use crate::graph::node::DataSchema;
use polars::prelude::{DataFrame, SchemaNamesAndDtypes, Series};
use std::sync::Arc;

use super::snapshot::TabularSnapshot;

#[derive(Clone)]
pub struct VariableTabularCache {
    pub schema: DataSchema,
    pub dataframe: Arc<DataFrame>,
}

pub struct TabularCatalog<'a> {
    databases: &'a mut std::collections::HashMap<String, DatabaseInstance>,
    variable_cache: &'a std::collections::HashMap<String, VariableTabularCache>,
    execution: Option<&'a ExecutionDataStore>,
}

impl<'a> TabularCatalog<'a> {
    pub fn new(
        databases: &'a mut std::collections::HashMap<String, DatabaseInstance>,
        variable_cache: &'a std::collections::HashMap<String, VariableTabularCache>,
        execution: Option<&'a ExecutionDataStore>,
    ) -> Self {
        Self {
            databases,
            variable_cache,
            execution,
        }
    }

    pub fn schema(&mut self, id: &str) -> Option<DataSchema> {
        if super::r#ref::is_variable_handle(id) {
            return self
                .variable_cache
                .get(id)
                .map(|entry| entry.schema.clone());
        }
        if let Some(df) = self.dataframe(id).ok() {
            let columns = df
                .schema()
                .iter_names_and_dtypes()
                .map(|(name, dtype)| crate::graph::node::ColumnSchema {
                    name: name.to_string(),
                    data_type: crate::database::polars_dtype_to_data_type(dtype),
                })
                .collect();
            return Some(DataSchema { columns });
        }
        None
    }

    pub fn dataframe(&mut self, id: &str) -> Result<Arc<DataFrame>, String> {
        if let Some(entry) = self.variable_cache.get(id) {
            return Ok(entry.dataframe.clone());
        }

        if let Some(store) = self.execution {
            if let Some(df) = store.get_dataframe(id) {
                return Ok(df);
            }
        }

        if let Some(db) = self.databases.get_mut(id) {
            if let crate::database::DatabaseState::DuckDb { row_count, .. } = &db.state {
                if *row_count > crate::database::MAX_GET_DATAFRAME_ROWS {
                    return Err(format!(
                        "Dataset '{id}' has {row_count} rows; exceeds in-memory graph limit ({}). \
                         Use column-scoped nodes or filter in DuckDB first.",
                        crate::database::MAX_GET_DATAFRAME_ROWS
                    ));
                }
            }
            let df = db
                .ensure_loaded()
                .map_err(|e| format!("Failed to load database '{id}': {e}"))?;
            let arc = Arc::new(df.clone());
            if let Some(store) = self.execution {
                // Cache under original id when resolving during execution.
                let _ = store;
            }
            return Ok(arc);
        }

        Err(format!("Tabular resource '{id}' not found"))
    }

    pub fn column_names(&mut self, id: &str) -> Result<Vec<String>, String> {
        if let Some(entry) = self.variable_cache.get(id) {
            return Ok(entry
                .schema
                .columns
                .iter()
                .map(|c| c.name.clone())
                .collect());
        }
        if let Some(store) = self.execution {
            if let Some(df) = store.get_dataframe(id) {
                return Ok(df
                    .schema()
                    .iter_names()
                    .map(|name| name.to_string())
                    .collect());
            }
        }
        let df = self.dataframe(id)?;
        Ok(df
            .schema()
            .iter_names()
            .map(|name| name.to_string())
            .collect())
    }

    pub fn column_series(&mut self, id: &str, column: &str) -> Result<Series, String> {
        let cache_key = format!("{id}::{column}");
        if let Some(store) = self.execution {
            if let Some(series) = store.get_data_series(&cache_key) {
                return Ok(series.clone());
            }
            if let Some(df) = store.get_dataframe(id) {
                let series = df
                    .column(column)
                    .map_err(|e| format!("Column '{column}' not found in cached DataFrame: {e}"))?
                    .clone()
                    .take_materialized_series();
                return Ok(series);
            }
        }

        if let Some(entry) = self.variable_cache.get(id) {
            let series = entry
                .dataframe
                .column(column)
                .map_err(|e| format!("Column '{column}' not found in variable tabular: {e}"))?
                .clone()
                .take_materialized_series();
            return Ok(series);
        }

        if let Some(db) = self.databases.get_mut(id) {
            return db
                .load_column_series(column)
                .map_err(|e| format!("Failed to load column '{column}' from '{id}': {e}"));
        }

        let df = self.dataframe(id)?;
        df.column(column)
            .map(|col| col.clone().take_materialized_series())
            .map_err(|e| format!("Column '{column}' not found in '{id}': {e}"))
    }
}

pub fn build_variable_cache_entry(
    snapshot: &TabularSnapshot,
) -> Result<VariableTabularCache, String> {
    let dataframe = Arc::new(snapshot.to_dataframe()?);
    let schema = snapshot.to_schema()?;
    Ok(VariableTabularCache { schema, dataframe })
}
