use std::collections::{BTreeMap, BTreeSet};

use crate::database::session_api::DatabaseCatalogSnapshot;
use std::hash::{Hash, Hasher};
use thiserror::Error;
use yss_database_contract::{DatabaseDecl, DatabaseId};
use yss_execution::plan::{
    CanonicalDecimal, CompiledExecutionPackage, CompiledFunctionBundle,
    CompiledParameterBundleBuilder, CompiledParameterHandle, ExecutionPlan, PlanGraphId,
    PlanInputBinding, PlanInputSource, PlanNodeId, PlanObservationIntent, PlanOperation,
    PlanOperationKind, PlanParameterFieldId, PlanParameterPayload, PlanParameterScalar,
    PlanParameterSchemaId, PlanParameterValue, PlanPortAddress, PlanProvenance, PlanSourceIdentity,
    ValueRef,
};
use yss_graph_analysis::{GraphPlotDataKind, GraphResultCategory, GraphStatisticalReportKind};
use yss_graph_analysis_contract::{
    CompilationBasis, ResourceKey as GraphResourceKey, ResourceObservedState,
    ResourceVersion as GraphResourceVersion,
};
use yss_graph_compiler::{
    GraphCompiledPackage, GraphInputSource, GraphObservationIntent, GraphParameterScalar,
    GraphParameterValue, GraphSourceIdentity,
};
use yss_graph_document::{GraphResourcePath, GraphRevision};
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, FunctionCatalogEntry, FunctionSignature, GraphResourceId,
    ResourceCatalogFingerprint, ResourceCatalogSnapshot, VariableValueContract,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphResourceSnapshot {
    project_instance_id: yss_project_identity::ProjectInstanceId,
    authority_generation: u64,
    functions: BTreeMap<GraphResourcePath, FunctionSignature>,
    variables: BTreeMap<GraphResourceId, VariableValueContract>,
    databases: BTreeMap<DatabaseId, DatabaseDecl>,
}

impl ProjectGraphResourceSnapshot {
    pub fn new(
        project_instance_id: yss_project_identity::ProjectInstanceId,
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

    pub fn project_instance_id(&self) -> &yss_project_identity::ProjectInstanceId {
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
    // computed from the already captured declarations/contracts and does not
    // expose Project/Database storage or a mutable map to Graph.
    let fingerprint =
        ResourceCatalogFingerprint::from_bytes(catalog_fingerprint(project, databases));
    Ok(ResourceCatalogSnapshot::new(
        functions,
        variables,
        database_catalog,
        fingerprint,
    ))
}

fn catalog_fingerprint(
    project: &ProjectGraphResourceSnapshot,
    databases: &DatabaseCatalogSnapshot,
) -> [u8; 32] {
    let mut fingerprint = [0; 32];
    for lane in 0..4u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        lane.hash(&mut hasher);
        project.project_instance_id.as_str().hash(&mut hasher);
        project.authority_generation.hash(&mut hasher);
        for (path, signature) in &project.functions {
            path.as_str().hash(&mut hasher);
            signature.parameters().hash(&mut hasher);
            signature.result().hash(&mut hasher);
        }
        for (resource, contract) in &project.variables {
            resource.as_str().hash(&mut hasher);
            contract.data_type().hash(&mut hasher);
        }
        for schema in databases.schemas() {
            schema.database().as_str().hash(&mut hasher);
            schema.runtime_revision().get().hash(&mut hasher);
            schema.schema_revision().get().hash(&mut hasher);
            for column in schema.columns() {
                column.name().as_str().hash(&mut hasher);
                column.data_type().hash(&mut hasher);
                column.nullable().hash(&mut hasher);
            }
        }
        fingerprint[(lane as usize) * 8..(lane as usize + 1) * 8]
            .copy_from_slice(&hasher.finish().to_le_bytes());
    }
    fingerprint
}

/// Convert the Execution-owned validation basis into the Graph-owned basis
/// consumed by analysis and lowering.  The conversion is deliberately kept in
/// Application: neither Graph nor Execution imports the other's package model.
pub fn graph_compilation_basis(
    basis: &yss_execution::plan::PlanCompilationBasis,
) -> CompilationBasis<GraphRevision> {
    CompilationBasis {
        graph_revision: GraphRevision::new(basis.graph_revision().get()),
        registry_fingerprint: yss_graph_registry::RegistryFingerprint::from_bytes(
            basis.registry_fingerprint().as_bytes(),
        ),
        resource_versions: basis
            .resource_versions()
            .iter()
            .map(|(key, version)| {
                (
                    GraphResourceKey::new(key.as_str()),
                    GraphResourceVersion::new(version.as_str()),
                )
            })
            .collect(),
        resource_observations: basis
            .resource_observations()
            .iter()
            .map(|(key, observed)| {
                let state = match observed {
                    yss_execution::plan::PlanResourceObservedState::Present(version) => {
                        ResourceObservedState::Present(GraphResourceVersion::new(version.as_str()))
                    }
                    yss_execution::plan::PlanResourceObservedState::Absent(version) => {
                        ResourceObservedState::Absent(
                            version
                                .as_ref()
                                .map(|version| GraphResourceVersion::new(version.as_str())),
                        )
                    }
                };
                (GraphResourceKey::new(key.as_str()), state)
            })
            .collect(),
    }
}

#[derive(Debug, Error)]
pub enum GraphPackageMappingError {
    #[error("graph package identity is invalid")]
    Identity(#[source] yss_execution::plan::InvalidPlanIdentity),
    #[error("graph package parameter identity is invalid")]
    ParameterIdentity(#[source] yss_execution::plan::InvalidPlanParameterId),
    #[error("graph package contains a non-finite decimal")]
    Decimal(#[source] yss_execution::plan::CanonicalDecimalError),
    #[error("graph package contains an invalid operation kind")]
    OperationKind(#[source] yss_execution::plan::InvalidPlanIdentity),
    #[error("graph package parameter handle is duplicated")]
    DuplicateParameter(#[source] yss_execution::plan::CompiledParameterBundleError),
}

/// Map the Graph-owned lowered package into the Execution-owned immutable
/// package at the Application boundary.
pub fn execution_package_from_graph(
    package: GraphCompiledPackage,
    basis: yss_execution::plan::PlanCompilationBasis,
) -> Result<CompiledExecutionPackage, GraphPackageMappingError> {
    let operations = package
        .operations()
        .iter()
        .map(|operation| {
            let source = plan_source_identity(operation.source())?;
            let kind = PlanOperationKind::new(operation.kind().to_owned().into_boxed_str())
                .map_err(GraphPackageMappingError::OperationKind)?;
            let parameter_handles = operation
                .parameter_handles()
                .iter()
                .map(|handle| {
                    CompiledParameterHandle::new(handle.as_str().to_owned().into_boxed_str())
                        .map_err(GraphPackageMappingError::ParameterIdentity)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| {
                    let port = PlanPortAddress::new(binding.port().to_owned().into_boxed_str())
                        .map_err(GraphPackageMappingError::Identity)?;
                    let source = match binding.source() {
                        GraphInputSource::Value(value) => {
                            PlanInputSource::Value(ValueRef::new(value.index()))
                        }
                        GraphInputSource::Parameter(handle) => PlanInputSource::Parameter(
                            CompiledParameterHandle::new(
                                handle.as_str().to_owned().into_boxed_str(),
                            )
                            .map_err(GraphPackageMappingError::ParameterIdentity)?,
                        ),
                    };
                    Ok(PlanInputBinding::new(port, source))
                })
                .collect::<Result<Vec<_>, GraphPackageMappingError>>()?
                .into_boxed_slice();
            let observation_intents = operation
                .observation_intents()
                .iter()
                .map(|intent| match intent {
                    GraphObservationIntent::InspectInput { input } => {
                        PlanObservationIntent::InspectInput {
                            input: ValueRef::new(input.index()),
                        }
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let output = operation.output().map(|value| ValueRef::new(value.index()));
            Ok(PlanOperation::new(
                source,
                kind,
                map_result_category(operation.result_category()),
                parameter_handles,
                inputs,
                observation_intents,
                output,
            ))
        })
        .collect::<Result<Vec<_>, GraphPackageMappingError>>()?;

    let mut parameters = CompiledParameterBundleBuilder::new(basis.clone());
    for (handle, payload) in package.parameters() {
        let handle = CompiledParameterHandle::new(handle.as_str().to_owned().into_boxed_str())
            .map_err(GraphPackageMappingError::ParameterIdentity)?;
        let schema = PlanParameterSchemaId::new(payload.schema().to_owned().into_boxed_str())
            .map_err(GraphPackageMappingError::ParameterIdentity)?;
        let value = map_parameter_value(payload.value())?;
        parameters
            .insert(handle, PlanParameterPayload::new(schema, value))
            .map_err(GraphPackageMappingError::DuplicateParameter)?;
    }
    let graph = PlanGraphId::new(package.graph().as_str().to_owned().into_boxed_str())
        .map_err(GraphPackageMappingError::Identity)?;
    let provenance = PlanProvenance::new(
        PlanSourceIdentity::new(graph, None, None),
        basis.clone(),
        yss_execution::plan::PlanCompileId::from_existing(package.compile_id().get()),
    );
    Ok(CompiledExecutionPackage::new(
        std::sync::Arc::new(ExecutionPlan::new(operations.into_boxed_slice())),
        std::sync::Arc::new(CompiledFunctionBundle::new(basis, Box::new([]), 0)),
        std::sync::Arc::new(parameters.freeze()),
        provenance,
    ))
}

fn plan_source_identity(
    source: &GraphSourceIdentity,
) -> Result<PlanSourceIdentity, GraphPackageMappingError> {
    let graph = PlanGraphId::new(source.graph().as_str().to_owned().into_boxed_str())
        .map_err(GraphPackageMappingError::Identity)?;
    let node = source
        .node()
        .map(|node| {
            PlanNodeId::new(node.to_string().into_boxed_str())
                .map_err(GraphPackageMappingError::Identity)
        })
        .transpose()?;
    let port = source
        .port()
        .map(|port| {
            PlanPortAddress::new(port.to_string().into_boxed_str())
                .map_err(GraphPackageMappingError::Identity)
        })
        .transpose()?;
    Ok(PlanSourceIdentity::new(graph, node, port))
}

fn map_parameter_value(
    value: &GraphParameterValue,
) -> Result<PlanParameterValue, GraphPackageMappingError> {
    Ok(match value {
        GraphParameterValue::Scalar(scalar) => PlanParameterValue::Scalar(match scalar {
            GraphParameterScalar::Null => PlanParameterScalar::Null,
            GraphParameterScalar::Bool(value) => PlanParameterScalar::Bool(*value),
            GraphParameterScalar::Integer(value) => PlanParameterScalar::Integer(*value),
            GraphParameterScalar::Unsigned(value) => PlanParameterScalar::Unsigned(*value),
            GraphParameterScalar::Decimal(value) => PlanParameterScalar::Decimal(
                CanonicalDecimal::try_new(*value).map_err(GraphPackageMappingError::Decimal)?,
            ),
            GraphParameterScalar::String(value) => PlanParameterScalar::String(value.clone()),
        }),
        GraphParameterValue::Resource(resource) => PlanParameterValue::Resource(
            yss_execution::plan::PlanResourceId::new(resource.clone())
                .map_err(GraphPackageMappingError::Identity)?,
        ),
        GraphParameterValue::List(values) => PlanParameterValue::List(
            values
                .iter()
                .map(map_parameter_value)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        GraphParameterValue::Record(fields) => PlanParameterValue::Record(
            fields
                .iter()
                .map(|(field, value)| {
                    let field = PlanParameterFieldId::new(field.clone())
                        .map_err(GraphPackageMappingError::ParameterIdentity)?;
                    Ok((field, map_parameter_value(value)?))
                })
                .collect::<Result<_, GraphPackageMappingError>>()?,
        ),
    })
}

fn map_result_category(category: GraphResultCategory) -> yss_execution::plan::ResultCategory {
    use yss_execution::plan::{PlotDataKind, ResultCategory, StatisticalReportKind};
    match category {
        GraphResultCategory::Value => ResultCategory::Value,
        GraphResultCategory::PlotData(kind) => ResultCategory::PlotData(match kind {
            GraphPlotDataKind::Scatter => PlotDataKind::Scatter,
            GraphPlotDataKind::Line => PlotDataKind::Line,
            GraphPlotDataKind::Plot => PlotDataKind::Plot,
            GraphPlotDataKind::Ecdf => PlotDataKind::Ecdf,
            GraphPlotDataKind::Kde => PlotDataKind::Kde,
            GraphPlotDataKind::Histogram => PlotDataKind::Histogram,
            GraphPlotDataKind::Correlation => PlotDataKind::Correlation,
            GraphPlotDataKind::Correlogram => PlotDataKind::Correlogram,
        }),
        GraphResultCategory::StatisticalReport(kind) => {
            ResultCategory::StatisticalReport(match kind {
                GraphStatisticalReportKind::OlsSummary => StatisticalReportKind::OlsSummary,
                GraphStatisticalReportKind::BinarySummary => StatisticalReportKind::BinarySummary,
                GraphStatisticalReportKind::Iv2slsSummary => StatisticalReportKind::Iv2slsSummary,
                GraphStatisticalReportKind::IvLimlSummary => StatisticalReportKind::IvLimlSummary,
                GraphStatisticalReportKind::PraisSummary => StatisticalReportKind::PraisSummary,
                GraphStatisticalReportKind::VarSummary => StatisticalReportKind::VarSummary,
                GraphStatisticalReportKind::VarSoc => StatisticalReportKind::VarSoc,
                GraphStatisticalReportKind::PanelSummary => StatisticalReportKind::PanelSummary,
                GraphStatisticalReportKind::PanelDid => StatisticalReportKind::PanelDid,
                GraphStatisticalReportKind::DfAdfSummary => StatisticalReportKind::DfAdfSummary,
                GraphStatisticalReportKind::DfAdfSummaryList => {
                    StatisticalReportKind::DfAdfSummaryList
                }
                GraphStatisticalReportKind::VecSummary => StatisticalReportKind::VecSummary,
                GraphStatisticalReportKind::VecRankSummary => StatisticalReportKind::VecRankSummary,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::session_api::{
        DatabaseCatalogSnapshotFixtureSchema, database_catalog_snapshot_fixture,
    };
    use std::num::NonZeroU64;
    use yss_data_contract::DataType;
    use yss_database_contract::{
        DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
        DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine,
    };
    use yss_database_schema::DatabaseColumnFact;
    use yss_graph_resource_contract::VariableValueContract;

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
                columns: Box::new([DatabaseColumnFact::new(
                    yss_tabular_contract::TabularColumnName::try_from("amount").unwrap(),
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
            yss_project_identity::ProjectInstanceId::from_existing("project".into()),
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
    }
}
