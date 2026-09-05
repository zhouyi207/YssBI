use std::collections::{BTreeMap, BTreeSet};

use std::hash::{Hash, Hasher};
use thiserror::Error;
use yss_database_contract::{DatabaseDecl, DatabaseId};
use yss_database_runtime::session_api::DatabaseCatalogSnapshot;
use yss_execution::plan::{
    CanonicalDecimal, CompiledExecutionPackage, CompiledFunctionBundle,
    CompiledParameterBundleBuilder, CompiledParameterHandle, ExecutionPlan, KernelId, PlanGraphId,
    PlanInputBinding, PlanInputCoercion, PlanInputCoercionKind, PlanInputSource,
    PlanKernelSpecialization, PlanNodeId, PlanObservationIntent, PlanOperation, PlanOutputBinding,
    PlanOutputRef, PlanParameterFieldId, PlanParameterPayload, PlanParameterScalar,
    PlanParameterSchemaId, PlanParameterValue, PlanPortAddress, PlanProvenance, PlanSourceIdentity,
    PlanTypeBinding, ValueRef,
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
use yss_graph_document::GraphResourcePath;
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
    #[error("function document could not be captured")]
    FunctionDocument {
        graph: GraphResourcePath,
        #[source]
        source: yss_project_filesystem::ProjectFilesystemError,
    },
    #[error("database schema is missing from the catalog snapshot")]
    MissingDatabaseSchema { database: DatabaseId },
    #[error("catalog snapshot contains an undeclared database schema")]
    UnexpectedDatabaseSchema { database: DatabaseId },
}

pub(crate) fn capture_function_dependencies(
    captured: &crate::execution::ApplicationSession,
    document: &yss_graph_document::GraphDocument,
    mut catalog: ResourceCatalogSnapshot,
) -> Result<ResourceCatalogSnapshot, GraphContractMappingError> {
    fn callees(document: &yss_graph_document::GraphDocument) -> Vec<GraphResourcePath> {
        document
            .nodes
            .values()
            .filter(|node| node.node_type.as_str() == "yssbi.project.function.call")
            .filter_map(|node| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == "target")
                    .and_then(|(_, value)| value.as_str())
                    .and_then(|path| GraphResourcePath::new(path).ok())
            })
            .collect()
    }
    let mut pending = callees(document);
    let mut seen = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) || catalog.function_signature(&path).is_none() {
            continue;
        }
        if let Some(body) = catalog.function_document(&path) {
            pending.extend(callees(body));
            continue;
        }
        let resource = captured
            .project()
            .read_graph_resource_snapshot(captured.project_instance_id(), &path)
            .map_err(|source| GraphContractMappingError::FunctionDocument {
                graph: path.clone(),
                source,
            })?;
        pending.extend(callees(&resource.document));
        catalog = catalog.with_function_document(&path, resource.document);
    }
    Ok(catalog)
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
) -> CompilationBasis {
    CompilationBasis {
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
    #[error("graph package contains a resolved type unsupported by execution")]
    UnsupportedResolvedType,
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
            let parameter_handles = operation
                .parameters()
                .iter()
                .map(|(key, handle)| {
                    Ok((
                        PlanParameterFieldId::new(key.clone())
                            .map_err(GraphPackageMappingError::ParameterIdentity)?,
                        CompiledParameterHandle::new(handle.as_str().to_owned().into_boxed_str())
                            .map_err(GraphPackageMappingError::ParameterIdentity)?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, GraphPackageMappingError>>()?;
            let inputs = operation
                .inputs()
                .iter()
                .map(|binding| {
                    let port = PlanPortAddress::new(binding.address().to_string().into_boxed_str())
                        .map_err(GraphPackageMappingError::Identity)?;
                    let source = plan_input_source(binding.source())?;
                    let contract = binding.contract();
                    Ok(PlanInputBinding::new(
                        port,
                        source,
                        yss_execution::plan::PlanInputContract {
                            group: contract
                                .group
                                .map(|group| {
                                    yss_execution::plan::PlanInputGroupId::new(
                                        group.to_string().into_boxed_str(),
                                    )
                                })
                                .transpose()
                                .map_err(GraphPackageMappingError::Identity)?,
                            expected_type: yss_graph_type_mapping::data_type_from_resolved_type(
                                &contract.expected_type,
                            )
                            .ok_or(GraphPackageMappingError::UnsupportedResolvedType)?,
                            coercions: contract
                                .coercions
                                .iter()
                                .copied()
                                .map(plan_coercion_kind)
                                .collect(),
                        },
                    ))
                })
                .collect::<Result<Vec<_>, GraphPackageMappingError>>()?
                .into_boxed_slice();
            let observation_intents = operation
                .observation_intents()
                .iter()
                .map(|intent| match intent {
                    GraphObservationIntent::InspectInput { source } => {
                        Ok(PlanObservationIntent::InspectInput {
                            source: plan_input_source(source)?,
                        })
                    }
                })
                .collect::<Result<Vec<_>, GraphPackageMappingError>>()?
                .into_boxed_slice();
            let output_graph = source.graph().clone();
            let outputs = operation
                .outputs()
                .iter()
                .map(|binding| {
                    let port = PlanPortAddress::new(binding.port().to_owned().into_boxed_str())
                        .map_err(GraphPackageMappingError::Identity)?;
                    Ok(PlanOutputBinding::new(
                        PlanOutputRef::new(output_graph.clone(), port),
                        ValueRef::new(binding.value().index()),
                        map_output_contract(binding.contract())?,
                    ))
                })
                .collect::<Result<Vec<_>, GraphPackageMappingError>>()?
                .into_boxed_slice();
            let specialization = plan_specialization(operation.specialization())?;
            Ok(PlanOperation::new(
                source,
                yss_execution::plan::PlanNodeTypeId::new(operation.node_type().as_str().into())
                    .map_err(GraphPackageMappingError::Identity)?,
                parameter_handles,
                inputs,
                observation_intents,
                outputs,
                specialization,
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

fn plan_specialization(
    value: &yss_graph_analysis::GraphKernelSpecialization,
) -> Result<PlanKernelSpecialization, GraphPackageMappingError> {
    let implementation = KernelId::new(value.implementation.clone())
        .map_err(GraphPackageMappingError::OperationKind)?;
    let bindings = |values: &[yss_graph_analysis::GraphPortTypeBinding]| {
        values
            .iter()
            .map(|binding| {
                let port = PlanPortAddress::new(binding.address.to_string().into_boxed_str())
                    .map_err(GraphPackageMappingError::Identity)?;
                let data_type =
                    yss_graph_type_mapping::data_type_from_resolved_type(&binding.value_type)
                        .ok_or(GraphPackageMappingError::UnsupportedResolvedType)?;
                Ok(PlanTypeBinding::new(port, data_type))
            })
            .collect::<Result<Vec<_>, GraphPackageMappingError>>()
            .map(Vec::into_boxed_slice)
    };
    let coercions = value
        .coercions
        .iter()
        .map(|coercion| {
            let port = PlanPortAddress::new(coercion.address.to_string().into_boxed_str())
                .map_err(GraphPackageMappingError::Identity)?;
            let kind = plan_coercion_kind(coercion.kind);
            Ok(PlanInputCoercion::new(port, kind))
        })
        .collect::<Result<Vec<_>, GraphPackageMappingError>>()?
        .into_boxed_slice();
    Ok(PlanKernelSpecialization::new(
        implementation,
        bindings(&value.input_types)?,
        bindings(&value.output_types)?,
        coercions,
    ))
}

fn plan_coercion_kind(kind: yss_graph_protocol::InputCoercionKind) -> PlanInputCoercionKind {
    match kind {
        yss_graph_protocol::InputCoercionKind::WidenInt64ToFloat64 => {
            PlanInputCoercionKind::WidenInt64ToFloat64
        }
        yss_graph_protocol::InputCoercionKind::BroadcastScalarToSeries => {
            PlanInputCoercionKind::BroadcastScalarToSeries
        }
    }
}

fn map_output_contract(
    contract: &yss_graph_compiler::GraphOutputContract,
) -> Result<yss_execution::plan::PlanOutputContract, GraphPackageMappingError> {
    use yss_data_contract::DataType;
    use yss_graph_protocol::RelationalScalarType;
    Ok(yss_execution::plan::PlanOutputContract {
        data_type: yss_graph_type_mapping::data_type_from_resolved_type(&contract.value_type)
            .ok_or(GraphPackageMappingError::UnsupportedResolvedType)?,
        schema: contract.schema.as_ref().map(|schema| {
            schema
                .fields
                .iter()
                .map(|field| yss_execution::plan::PlanOutputField {
                    name: field.name.0.clone(),
                    data_type: match field.scalar_type {
                        RelationalScalarType::Boolean => DataType::Boolean,
                        RelationalScalarType::Int64 => DataType::Int64,
                        RelationalScalarType::Float64 => DataType::Float64,
                        RelationalScalarType::String => DataType::String,
                        RelationalScalarType::Date => DataType::Date,
                        RelationalScalarType::DateTime => DataType::Datetime,
                        RelationalScalarType::Unknown => DataType::Any,
                    },
                    lineage: field.lineage.as_ref().map(|lineage| {
                        yss_execution::plan::PlanFieldLineage {
                            source_identity: lineage.source.clone(),
                            field_identity: lineage.field.clone(),
                        }
                    }),
                })
                .collect()
        }),
        category: map_result_category(contract.category),
        source: plan_source_identity(&contract.source)?,
    })
}

fn plan_input_source(
    source: &GraphInputSource,
) -> Result<PlanInputSource, GraphPackageMappingError> {
    Ok(match source {
        GraphInputSource::Value(value) => PlanInputSource::Value(ValueRef::new(value.index())),
        GraphInputSource::Parameter(handle) => PlanInputSource::Parameter(
            CompiledParameterHandle::new(handle.as_str().to_owned().into_boxed_str())
                .map_err(GraphPackageMappingError::ParameterIdentity)?,
        ),
    })
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
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use yss_data_contract::DataType;
    use yss_database_contract::{
        DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
        DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };
    use yss_database_edit::EditHistory;
    use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
    use yss_database_runtime::{DatabaseInstance, DatabaseState};
    use yss_graph_resource_contract::{FunctionParameterContract, VariableValueContract};

    #[test]
    fn package_mapping_preserves_named_parameters_group_order_and_each_output_contract() {
        use yss_graph_compiler::{
            GraphInputBinding, GraphInputContract, GraphOperation, GraphOutputBinding,
            GraphOutputContract, GraphParameterHandle, GraphParameterPayload, GraphValueRef,
        };
        use yss_graph_document::{NodeId, PortAddress, PortInstanceId};
        use yss_graph_protocol::{PortKey, ResolvedType, TypeId};
        let graph = GraphResourcePath::new("events/Contract.yssbi-event").unwrap();
        let node = NodeId::new();
        let integer = ResolvedType::Nominal(TypeId::new("core.int64").unwrap());
        let groups = [
            PortInstanceId::from_bytes([9; 16]),
            PortInstanceId::from_bytes([1; 16]),
        ];
        let inputs = groups
            .iter()
            .flat_map(|group| {
                ["value", "weight"].map(|key| {
                    GraphInputBinding::new(
                        PortAddress::instance(node, PortKey::new(key).unwrap(), *group),
                        GraphInputSource::Parameter(GraphParameterHandle::new("alpha")),
                        GraphInputContract {
                            group: Some(*group),
                            expected_type: integer.clone(),
                            coercions: Box::new([]),
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        let outputs = [
            GraphResultCategory::Value,
            GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::OlsSummary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, category)| {
            let address = PortAddress::declared(
                node,
                PortKey::new(if index == 0 { "value" } else { "report" }).unwrap(),
            );
            GraphOutputBinding::new(
                address.to_string(),
                GraphValueRef::new(index as u32),
                GraphOutputContract {
                    value_type: integer.clone(),
                    schema: None,
                    category,
                    source: GraphSourceIdentity::new(graph.clone(), Some(node), Some(address)),
                },
            )
        })
        .collect::<Vec<_>>();
        let specialization = yss_graph_analysis::GraphKernelSpecialization {
            implementation: "kernel.weighted-summary".into(),
            input_types: inputs
                .iter()
                .map(|input| yss_graph_analysis::GraphPortTypeBinding {
                    address: input.address().clone(),
                    value_type: integer.clone(),
                })
                .collect(),
            output_types: outputs
                .iter()
                .map(|output| yss_graph_analysis::GraphPortTypeBinding {
                    address: output.contract().source.port().unwrap().clone(),
                    value_type: integer.clone(),
                })
                .collect(),
            coercions: Box::new([]),
        };
        let package = GraphCompiledPackage::new(
            graph.clone(),
            yss_graph_analysis_contract::CompileId::new(1),
            Box::new([GraphOperation::new(
                GraphSourceIdentity::new(graph.clone(), Some(node), None),
                "test.custom.weighted_summary".parse().unwrap(),
                BTreeMap::from([
                    ("alpha".into(), GraphParameterHandle::new("alpha")),
                    ("label".into(), GraphParameterHandle::new("label")),
                ]),
                inputs.into_boxed_slice(),
                Box::new([]),
                outputs.into_boxed_slice(),
                specialization,
            )]),
            BTreeMap::from([
                (
                    GraphParameterHandle::new("alpha"),
                    GraphParameterPayload::new(
                        "int",
                        GraphParameterValue::Scalar(GraphParameterScalar::Integer(3)),
                    ),
                ),
                (
                    GraphParameterHandle::new("label"),
                    GraphParameterPayload::new(
                        "string",
                        GraphParameterValue::Scalar(GraphParameterScalar::String(
                            "variables/plain-text".into(),
                        )),
                    ),
                ),
            ]),
        );
        let basis = yss_execution::plan::PlanCompilationBasis::new(
            yss_execution::plan::PlanProjectSessionId::from_existing("session".into()),
            yss_execution::plan::PlanRegistryFingerprint::from_bytes([0; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let mapped = execution_package_from_graph(package, basis).unwrap();
        let operation = &mapped.plan().operations()[0];
        assert_eq!(
            operation.node_type().as_str(),
            "test.custom.weighted_summary"
        );
        assert_eq!(operation.kernel_id().as_str(), "kernel.weighted-summary");
        assert_eq!(operation.parameters().len(), 2);
        assert_eq!(
            mapped.parameters().entries()[&CompiledParameterHandle::new("label".into()).unwrap()]
                .value(),
            &PlanParameterValue::Scalar(PlanParameterScalar::String("variables/plain-text".into()))
        );
        assert_eq!(
            operation
                .inputs()
                .iter()
                .map(|input| input.contract().group.as_ref().unwrap().as_str())
                .collect::<Vec<_>>(),
            [
                groups[0].to_string(),
                groups[0].to_string(),
                groups[1].to_string(),
                groups[1].to_string()
            ]
        );
        assert_eq!(
            operation.outputs()[0].contract().category,
            yss_execution::plan::ResultCategory::Value
        );
        assert_eq!(
            operation.outputs()[1].contract().category,
            yss_execution::plan::ResultCategory::StatisticalReport(
                yss_execution::plan::StatisticalReportKind::OlsSummary
            )
        );
        yss_execution::state::ExecutionRuntimeState::new(
            yss_execution::identity::ExecutionSessionId::new(uuid::Uuid::nil()),
            yss_execution::identity::RuntimeGeneration::INITIAL,
        )
        .prepare_compiled_package(mapped, yss_execution::identity::RuntimeGeneration::INITIAL)
        .unwrap();
    }

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
        let dataframe = polars::df!("amount" => &[1.0_f64]).unwrap();
        let runtime = DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing("session".into()),
                    NonZeroU64::new(1).unwrap(),
                    None,
                    vec![database.clone()].into(),
                    observations,
                ),
                [DatabaseInstance {
                    decl: database.clone(),
                    state: DatabaseState::Loaded {
                        dataframe: Arc::new(dataframe.clone()),
                        original: Arc::new(dataframe),
                        history: EditHistory::new(),
                    },
                }],
            )
            .unwrap();
        let schema = yss_database_runtime::session_api::catalog_snapshot(&runtime).unwrap();
        let function_path = GraphResourcePath::new("functions/forecast.yssbi-function").unwrap();
        let mut functions = BTreeMap::new();
        functions.insert(
            function_path.clone(),
            FunctionSignature::new(
                vec![FunctionParameterContract::new(
                    yss_graph_document::FunctionParameterId::new("x"),
                    "X",
                    DataType::Float64,
                )],
                Some(DataType::Float64),
            ),
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
