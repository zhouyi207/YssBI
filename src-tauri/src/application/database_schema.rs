use crate::database::{DatabaseInstance, DatabaseState};
use crate::project::{ProjectFilesystemError, ProjectResourceSnapshot, ProjectState};
use crate::schema::{
    ColumnInfoDTO, DatabaseDeclDTO, DatabasesVariablesDTO, VariableInstanceDTO,
    column_info_from_duckdb, column_info_from_schema,
};

pub fn name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

pub fn database_display_name(instance: &DatabaseInstance) -> String {
    instance.decl.name.clone()
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
            dto.load_failed = false;
        }
        DatabaseSchemaSnapshot::Failed { name, .. } => {
            dto.name = Some(name);
            dto.load_failed = true;
        }
    }
}

pub fn enrich_database_decl_dto(dto: &mut DatabaseDeclDTO, instance: &DatabaseInstance) {
    apply_schema_snapshot(dto, extract_database_schema(instance));
}

pub fn enriched_database_dtos(
    databases: &std::collections::HashMap<String, crate::database::DatabaseDecl>,
    runtime_databases: &std::collections::HashMap<String, DatabaseInstance>,
) -> std::collections::HashMap<String, DatabaseDeclDTO> {
    let mut enriched = std::collections::HashMap::new();
    for (id, decl) in databases.iter() {
        let mut db_dto = DatabaseDeclDTO::from(decl);
        if let Some(instance) = runtime_databases.get(id) {
            enrich_database_decl_dto(&mut db_dto, instance);
        }
        enriched.insert(id.clone(), db_dto);
    }
    enriched
}

pub fn databases_variables_from_snapshot(
    snapshot: ProjectResourceSnapshot,
) -> DatabasesVariablesDTO {
    let databases = enriched_database_dtos(&snapshot.databases, &snapshot.runtime_databases);
    let variables = snapshot
        .variables
        .iter()
        .map(|(id, variable)| (id.to_string(), VariableInstanceDTO::from(variable)))
        .collect();
    DatabasesVariablesDTO {
        databases,
        variables,
    }
}

pub fn project_databases_variables(
    state: &ProjectState,
) -> Result<DatabasesVariablesDTO, ProjectFilesystemError> {
    state
        .project_resource_snapshot()
        .map(databases_variables_from_snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::DatabaseEngineDTO;

    fn database_dto() -> DatabaseDeclDTO {
        DatabaseDeclDTO {
            id: "database-id".to_string(),
            engine: DatabaseEngineDTO::InMemory {
                name: "Database".to_string(),
            },
            schema_version: 1,
            required: true,
            name: None,
            columns: None,
            row_count: None,
            column_count: None,
            load_failed: false,
        }
    }

    #[test]
    fn failed_schema_snapshot_exposes_only_machine_state() {
        let mut dto = database_dto();

        apply_schema_snapshot(
            &mut dto,
            DatabaseSchemaSnapshot::Failed {
                name: "Database".to_string(),
                error: "sensitive backend failure".to_string(),
            },
        );

        let wire = serde_json::to_value(dto).expect("database DTO should serialize");
        assert_eq!(wire.get("loadFailed"), Some(&serde_json::Value::Bool(true)));
        assert!(wire.get("loadError").is_none());
        assert!(!wire.to_string().contains("sensitive backend failure"));
    }

    #[test]
    fn ready_schema_snapshot_clears_failed_state() {
        let mut dto = database_dto();
        dto.load_failed = true;

        apply_schema_snapshot(
            &mut dto,
            DatabaseSchemaSnapshot::Ready {
                name: "Database".to_string(),
                columns: vec![ColumnInfoDTO {
                    name: "value".to_string(),
                    dtype: "Int64".to_string(),
                }],
                row_count: 3,
                column_count: 1,
            },
        );

        assert!(!dto.load_failed);
        assert_eq!(dto.row_count, Some(3));
        assert_eq!(dto.column_count, Some(1));
    }
}
