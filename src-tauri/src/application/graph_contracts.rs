use std::collections::{BTreeMap, BTreeSet};

use crate::database::session_api::DatabaseCatalogSnapshot;
use crate::database_contract::{DatabaseDecl, DatabaseId};
use crate::graph::resource_catalog::{
    FunctionCatalogEntry, FunctionSignature, GraphResourceId, ResourceCatalogFingerprint,
    ResourceCatalogSnapshot, VariableValueContract,
};
use crate::graph::schema::{ColumnSchema, DataSchema};
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::GraphResourcePath;
use crate::project::ProjectComputationSettings;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphResourceSnapshot {
    project_instance_id: crate::project::ProjectInstanceId,
    authority_generation: u64,
    functions: BTreeMap<GraphResourcePath, FunctionSignature>,
    variables: BTreeMap<GraphResourceId, VariableValueContract>,
    databases: BTreeMap<DatabaseId, DatabaseDecl>,
}

impl ProjectGraphResourceSnapshot {
    pub fn new(
        project_instance_id: crate::project::ProjectInstanceId,
        authority_generation: u64,
        functions: BTreeMap<GraphResourcePath, FunctionSignature>,
        variables: BTreeMap<GraphResourceId, VariableValueContract>,
        databases: BTreeMap<DatabaseId, DatabaseDecl>,
    ) -> Self {
        Self {
            project_instance_id,
            authority_generation,
            functions,
            variables,
            databases,
        }
    }

    pub fn project_instance_id(&self) -> &crate::project::ProjectInstanceId {
        &self.project_instance_id
    }

    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }
}

#[derive(Debug, Error)]
pub enum GraphContractMappingError {
    #[error("database schema is missing from the catalog snapshot")]
    MissingDatabaseSchema { database: DatabaseId },
    #[error("catalog snapshot contains an undeclared database schema")]
    UnexpectedDatabaseSchema { database: DatabaseId },
    #[error("database schema cannot be represented by the Graph catalog")]
    InvalidDatabaseSchema {
        database: DatabaseId,
        #[source]
        source: crate::graph::error::GraphCatalogError,
    },
}

pub fn build_resource_catalog(
    project: &ProjectGraphResourceSnapshot,
    databases: &DatabaseCatalogSnapshot,
) -> Result<ResourceCatalogSnapshot, GraphContractMappingError> {
    let mut functions = BTreeMap::new();
    for (path, signature) in &project.functions {
        functions.insert(path.clone(), FunctionCatalogEntry::new(signature.clone()));
    }

    let variables = project.variables.clone();
    let declared_database_ids = project.databases.keys().cloned().collect::<BTreeSet<_>>();
    let mut schemas = BTreeSet::new();
    let mut database_catalog = BTreeMap::new();
    for schema in databases.schemas() {
        let database = schema.database().clone();
        if !declared_database_ids.contains(&database) {
            return Err(GraphContractMappingError::UnexpectedDatabaseSchema { database });
        }
        schemas.insert(database.clone());
        database_catalog.insert(
            GraphResourceId::new(format!("databases/{}", database.as_str())),
            DataSchema {
                columns: schema
                    .columns()
                    .iter()
                    .map(|column| ColumnSchema {
                        name: column.name().as_str().to_owned(),
                        data_type: column.data_type().clone(),
                    })
                    .collect(),
            },
        );
    }
    for database in declared_database_ids {
        if !schemas.contains(&database) {
            return Err(GraphContractMappingError::MissingDatabaseSchema { database });
        }
    }

    // The catalog fingerprint is a Graph compile fact. It is deliberately
    // represented as a closed value rather than exposing Project/Database
    // storage or a mutable map to Graph.
    let fingerprint = ResourceCatalogFingerprint::from_bytes([0; 32]);
    Ok(ResourceCatalogSnapshot::new(
        functions,
        variables,
        database_catalog,
        fingerprint,
    ))
}

pub fn graph_compile_settings(settings: &ProjectComputationSettings) -> GraphCompileSettings {
    GraphCompileSettings {
        absolute_tolerance: settings.numeric.tolerance.absolute,
        relative_tolerance: settings.numeric.tolerance.relative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_contract::DataType;
    use crate::database::schema_snapshot::database_column_fact_fixture;
    use crate::database::session_api::{
        DatabaseCatalogSnapshotFixtureSchema, database_catalog_snapshot_fixture,
    };
    use crate::database_contract::{
        DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
        DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine,
    };
    use crate::graph::resource_catalog::VariableValueContract;
    use std::num::NonZeroU64;

    #[test]
    fn project_and_database_snapshots_map_to_complete_graph_catalog_and_settings() {
        let database = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: true,
            name: "Sales".into(),
        };
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            database.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&database),
            ),
        )])
        .unwrap();
        let schema = database_catalog_snapshot_fixture(
            "session".into(),
            NonZeroU64::new(1).unwrap(),
            observations,
            Box::new([DatabaseCatalogSnapshotFixtureSchema {
                database: database.id.clone(),
                runtime_revision: 0,
                schema_revision: 0,
                columns: Box::new([database_column_fact_fixture(
                    crate::tabular::contract::TabularColumnName::try_from("amount").unwrap(),
                    DataType::Float64,
                    false,
                )]),
            }]),
        )
        .unwrap();
        let function_path = GraphResourcePath::new("functions/forecast.yssbi-function").unwrap();
        let mut functions = BTreeMap::new();
        functions.insert(
            function_path.clone(),
            FunctionSignature::new(vec![DataType::Float64], Some(DataType::Float64)),
        );
        let variable_id = GraphResourceId::new("variables/input");
        let mut variables = BTreeMap::new();
        variables.insert(variable_id, VariableValueContract::new(DataType::Float64));
        let mut databases = BTreeMap::new();
        databases.insert(database.id.clone(), database);

        let project = ProjectGraphResourceSnapshot::new(
            crate::project::ProjectInstanceId::from_existing("project".into()),
            7,
            functions,
            variables,
            databases,
        );
        let catalog = build_resource_catalog(&project, &schema).unwrap();
        assert!(catalog.function_signature(&function_path).is_some());
        assert!(
            catalog
                .database_schema(&GraphResourceId::new("databases/sales"))
                .is_some()
        );
        let settings = graph_compile_settings(&ProjectComputationSettings::default());
        assert_eq!(settings.absolute_tolerance, 1e-12);
        assert_eq!(settings.relative_tolerance, 1e-9);
    }
}
