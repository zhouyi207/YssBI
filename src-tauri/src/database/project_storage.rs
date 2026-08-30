use std::path::Path;

use super::{DatabaseInstance, DatabaseState, EditHistory};
use yss_database_contract::{DatabaseDecl, DatabaseEngine};
use yss_duckdb::{drop_data_table, read_table_meta};

pub fn bind_duckdb_instance(decl: &DatabaseDecl, project_root: Option<&Path>) -> DatabaseInstance {
    let DatabaseEngine::DuckDb { path, table, .. } = &decl.engine else {
        unreachable!("bind_duckdb_instance expects DuckDb engine");
    };

    let state = match project_root {
        Some(root) => {
            let abs = root.join(path);
            match read_table_meta(&abs, table) {
                Ok(meta) => DatabaseState::DuckDb {
                    duckdb_path: abs.to_string_lossy().to_string(),
                    table: table.clone(),
                    row_count: meta.row_count,
                    columns: meta.columns,
                    history: EditHistory::new(),
                },
                Err(error) => DatabaseState::Failed { error },
            }
        }
        None => DatabaseState::Failed {
            error: "Project path not set; cannot bind DuckDB database".into(),
        },
    };

    DatabaseInstance {
        decl: decl.clone(),
        state,
    }
}

pub fn remove_duckdb_table_if_needed(
    engine: &DatabaseEngine,
    project_root: Option<&Path>,
) -> Result<(), String> {
    let Some((relative_path, table)) = engine.duckdb_table() else {
        return Ok(());
    };
    let Some(root) = project_root else {
        return Ok(());
    };
    drop_data_table(&root.join(relative_path), table)
}
