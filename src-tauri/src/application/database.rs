use std::path::Path;

use polars::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::database::database_schema::polars_dtype_to_raw_string;
use crate::database::{DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::project::{unique_name, ProjectState};
use crate::schema::DatabaseEngineDTO;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadDatabaseResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseMetaResult {
    pub id: String,
    pub name: String,
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub dtype: String,
}

pub fn load_database(
    state: &ProjectState,
    engine: DatabaseEngineDTO,
) -> Result<LoadDatabaseResult, String> {
    let engine_domain = DatabaseEngine::try_from(engine.clone())
        .map_err(|e| format!("Invalid engine config: {}", e))?;

    let mut lazy_frame = engine_domain
        .build_lazy()
        .map_err(|e| format!("Failed to build lazy frame: {}", e))?;

    let schema = lazy_frame
        .collect_schema()
        .map_err(|e| format!("Failed to collect schema: {}", e))?;

    let columns = column_info_from_schema(schema.as_ref());
    let column_count = columns.len();
    let row_count = count_lazy_rows(&lazy_frame)?;
    let id = format!("db-{}", Uuid::new_v4());
    let base_name = base_name_from_engine_dto(&engine).unwrap_or_else(|| id.clone());
    let name = unique_database_name(state, &base_name);

    let decl = DatabaseDecl {
        id: id.clone(),
        engine: engine_domain,
        schema_version: 1,
        required: false,
        name: Some(name.clone()),
    };

    let instance = DatabaseInstance {
        decl,
        state: DatabaseState::Lazy { lazy_frame },
    };
    state.add_database(instance);

    Ok(LoadDatabaseResult {
        id,
        name,
        row_count,
        column_count,
        columns,
    })
}

pub fn get_database_meta(state: &ProjectState, id: &str) -> Result<DatabaseMetaResult, String> {
    let view = state
        .access_database(id, crate::database::DatabaseAccess::Preview)
        .map_err(|e| format!("Failed to access database: {}", e))?;

    let df = &view.dataframe;
    let columns = column_info_from_schema(df.schema().as_ref());
    let (name, row_count) = database_name_and_row_count(state, id)?;

    Ok(DatabaseMetaResult {
        id: id.to_string(),
        name,
        row_count,
        column_count: columns.len(),
        columns,
    })
}

pub fn name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

fn base_name_from_engine_dto(engine: &DatabaseEngineDTO) -> Option<String> {
    match engine {
        DatabaseEngineDTO::Csv { path, .. } => Some(name_from_path(path)),
        DatabaseEngineDTO::Parquet { path, .. } => Some(name_from_path(path)),
        DatabaseEngineDTO::Sql { table, .. } => Some(table.clone()),
        DatabaseEngineDTO::Excel { sheet, .. } => Some(sheet.clone()),
        _ => None,
    }
}

fn base_name_from_engine(engine: &DatabaseEngine) -> String {
    match engine {
        DatabaseEngine::Csv { path, .. } | DatabaseEngine::Parquet { path, .. } => {
            name_from_path(path)
        }
        DatabaseEngine::Sql { table, .. } => table.clone(),
        DatabaseEngine::Excel { sheet, .. } => sheet.clone(),
        DatabaseEngine::InMemory { name } => name.clone(),
    }
}

fn unique_database_name(state: &ProjectState, base_name: &str) -> String {
    let store = state.project_store.read().unwrap();
    let existing: Vec<String> = store
        .databases
        .values()
        .map(|db| base_name_from_engine(&db.decl.engine))
        .collect();
    unique_name::unique_name(base_name, existing.iter().map(|s| s.as_str()))
}

fn database_name_and_row_count(state: &ProjectState, id: &str) -> Result<(String, usize), String> {
    let store = state.project_store.read().unwrap();
    let db = store.databases.get(id).ok_or("Database not found")?;
    let name = db
        .decl
        .name
        .clone()
        .unwrap_or_else(|| base_name_from_engine(&db.decl.engine));

    let row_count = match &db.state {
        DatabaseState::Loaded { dataframe, .. } => dataframe.height(),
        DatabaseState::Lazy { lazy_frame } => {
            let lazy_frame = lazy_frame.clone();
            drop(store);
            return Ok((name, count_lazy_rows(&lazy_frame)?));
        }
        // 后台异步物化中；调用方理解为「行数未知」即可。
        DatabaseState::Pending => 0,
        DatabaseState::Failed { .. } => 0,
    };

    Ok((name, row_count))
}

fn column_info_from_schema(schema: &Schema) -> Vec<ColumnInfo> {
    schema
        .iter_names()
        .filter_map(|name| {
            schema.get(name).map(|dt| ColumnInfo {
                name: name.to_string(),
                dtype: polars_dtype_to_raw_string(dt),
            })
        })
        .collect()
}

fn count_lazy_rows(lazy_frame: &LazyFrame) -> Result<usize, String> {
    let count_df = lazy_frame
        .clone()
        .select([len()])
        .collect()
        .map_err(|e| format!("Failed to get row count: {}", e))?;

    Ok(count_df
        .columns()
        .first()
        .and_then(|s| s.u32().ok())
        .and_then(|ca| ca.get(0))
        .map(|v| v as usize)
        .unwrap_or(0))
}
