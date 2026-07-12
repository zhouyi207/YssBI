use polars::prelude::Schema;

use crate::database::{
    DatabaseEngine, DatabaseInstance, DatabaseState, DuckDbColumnMeta,
    database_schema::polars_dtype_to_raw_string,
};
use crate::schema::{ColumnInfoDTO, DatabaseDeclDTO};

pub fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

pub fn database_display_name(instance: &DatabaseInstance) -> String {
    instance
        .decl
        .name
        .clone()
        .unwrap_or_else(|| match &instance.decl.engine {
            DatabaseEngine::Csv { path, .. } => name_from_path(path),
            DatabaseEngine::Parquet { path, .. } => name_from_path(path),
            DatabaseEngine::Sql { table, .. } => table.clone(),
            DatabaseEngine::Excel { sheet, .. } => sheet.clone(),
            DatabaseEngine::DuckDb { .. } => instance.decl.id.clone(),
            DatabaseEngine::InMemory { name } => name.clone(),
        })
}

pub fn column_info_from_schema(schema: &Schema) -> Vec<ColumnInfoDTO> {
    schema
        .iter_names()
        .filter_map(|name| {
            schema.get(name).map(|dt| ColumnInfoDTO {
                name: name.to_string(),
                dtype: polars_dtype_to_raw_string(dt),
            })
        })
        .collect()
}

pub fn column_info_from_duckdb(columns: &[DuckDbColumnMeta]) -> Vec<ColumnInfoDTO> {
    columns
        .iter()
        .map(|col| ColumnInfoDTO {
            name: col.name.clone(),
            dtype: col.dtype.clone(),
        })
        .collect()
}

#[derive(Debug)]
pub enum DatabaseSchemaSnapshot {
    Ready {
        name: String,
        columns: Vec<ColumnInfoDTO>,
        row_count: usize,
        column_count: usize,
    },
    Failed {
        name: String,
        error: String,
    },
}

pub fn extract_database_schema(instance: &DatabaseInstance) -> DatabaseSchemaSnapshot {
    let name = database_display_name(instance);

    match &instance.state {
        DatabaseState::Loaded { dataframe, .. } => {
            let columns = column_info_from_schema(dataframe.schema().as_ref());
            let row_count = dataframe.height();
            let column_count = columns.len();
            DatabaseSchemaSnapshot::Ready {
                name,
                columns,
                row_count,
                column_count,
            }
        }
        DatabaseState::DuckDb {
            row_count, columns, ..
        } => {
            let columns = column_info_from_duckdb(columns);
            let column_count = columns.len();
            DatabaseSchemaSnapshot::Ready {
                name,
                columns,
                row_count: *row_count,
                column_count,
            }
        }
        DatabaseState::Failed { error } => DatabaseSchemaSnapshot::Failed {
            name,
            error: error.clone(),
        },
    }
}

pub fn apply_schema_snapshot(dto: &mut DatabaseDeclDTO, snapshot: DatabaseSchemaSnapshot) {
    match snapshot {
        DatabaseSchemaSnapshot::Ready {
            name,
            columns,
            row_count,
            column_count,
        } => {
            dto.name = Some(name);
            dto.columns = Some(columns);
            dto.row_count = Some(row_count);
            dto.column_count = Some(column_count);
        }
        DatabaseSchemaSnapshot::Failed { name, error } => {
            dto.name = Some(name);
            dto.load_error = Some(error);
        }
    }
}

pub fn enrich_database_decl_dto(dto: &mut DatabaseDeclDTO, instance: &DatabaseInstance) {
    apply_schema_snapshot(dto, extract_database_schema(instance));
}

pub fn enriched_database_dtos(
    databases: &std::collections::HashMap<String, crate::database::DatabaseDecl>,
    store: &crate::project::ProjectStore,
) -> std::collections::HashMap<String, DatabaseDeclDTO> {
    let mut enriched = std::collections::HashMap::new();
    for (id, decl) in databases.iter() {
        let mut db_dto = DatabaseDeclDTO::from(decl);
        if let Some(instance) = store.databases.get(id) {
            enrich_database_decl_dto(&mut db_dto, instance);
        }
        enriched.insert(id.clone(), db_dto);
    }
    enriched
}
