use super::catalog::capture_localized_project_facts;
mod schema_observations;
use crate::session::ApplicationSession;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use yss_database_contract::{DatabaseDecl, DatabaseId};
use yss_database_runtime::session_api::{DatabaseCatalogSnapshot, catalog_snapshot};
use yss_graph_analysis::direct_function_dependencies;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_graph_execution::plan::{PlanGraphId, PlanNodeId, PlanOutputRef, PlanPortAddress};
use yss_graph_resource_contract::{
    ColumnSchema, DataSchema, FunctionCatalogEntry, FunctionSignature, GraphResourceId,
    ResourceCatalogSnapshot,
};

#[derive(Debug, Error)]
pub enum GraphInputError {
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] super::catalog::ProjectCatalogReadError),
    #[error("database catalog snapshot failed")]
    Database(#[source] yss_database_runtime::error::DatabaseError),
    #[error("graph resource contract mapping failed")]
    Contract(#[source] GraphContractMappingError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGraphResourceSnapshot {
    functions: BTreeMap<GraphResourcePath, FunctionSignature>,
    databases: BTreeMap<DatabaseId, DatabaseDecl>,
}

impl ProjectGraphResourceSnapshot {
    pub fn new(
        functions: BTreeMap<GraphResourcePath, FunctionSignature>,
        databases: BTreeMap<DatabaseId, DatabaseDecl>,
    ) -> Self {
        Self {
            functions,
            databases,
        }
    }

    pub(super) fn function_signature(
        &self,
        path: &GraphResourcePath,
    ) -> Option<&FunctionSignature> {
        self.functions.get(path)
    }
}

#[derive(Debug, Error)]
pub enum GraphContractMappingError {
    #[error("function document could not be captured")]
    FunctionDocument {
        graph: GraphResourcePath,
        #[source]
        source: yss_project::ProjectOperationError,
    },
    #[error("database schema is missing from the catalog snapshot")]
    MissingDatabaseSchema { database: DatabaseId },
    #[error("catalog snapshot contains an undeclared database schema")]
    UnexpectedDatabaseSchema { database: DatabaseId },
}

pub(crate) fn capture_function_dependencies(
    captured: &crate::session::ApplicationSession,
    document: &yss_graph_document::GraphDocument,
    mut catalog: ResourceCatalogSnapshot,
) -> Result<ResourceCatalogSnapshot, GraphContractMappingError> {
    let registry = captured.graph().registry();
    let mut pending = direct_function_dependencies(document, registry).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) || catalog.function_signature(&path).is_none() {
            continue;
        }
        if let Some(body) = catalog.function_document(&path) {
            pending.extend(direct_function_dependencies(body, registry));
            continue;
        }
        let resource = captured
            .project()
            .read_graph_resource_snapshot(captured.project_instance_id(), &path)
            .map_err(|source| GraphContractMappingError::FunctionDocument {
                graph: path.clone(),
                source,
            })?;
        pending.extend(direct_function_dependencies(&resource.document, registry));
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
                        semantic: column.semantic().cloned(),
                        physical_type: Some(column.physical_type().to_owned()),
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

    Ok(ResourceCatalogSnapshot::new(functions, database_catalog))
}

pub(crate) fn graph_result_inputs(
    graph: &GraphResourcePath,
    analysis: &yss_graph_analysis::GraphAnalysis,
    databases: &DatabaseCatalogSnapshot,
    registry_fingerprint: [u8; 32],
) -> yss_graph_execution::result::GraphResultInputs {
    use yss_graph_analysis::{GraphResolvedInputSource, GraphResolvedParameterValue};
    use yss_graph_execution::result::{GraphResultInputs, OutputResultInputs};
    use yss_graph_resource_contract::GraphDependencyKey;
    use yss_node_protocol::PortDirection;

    fn resources(
        node: &yss_graph_analysis::GraphNodeSemanticFact,
        semantics: &yss_graph_analysis::GraphSemanticSnapshot,
        databases: &DatabaseCatalogSnapshot,
        visited: &mut BTreeSet<GraphResourcePath>,
        versions: &mut BTreeMap<Box<str>, Option<[u8; 32]>>,
    ) {
        for identity in
            node.parameters
                .iter()
                .filter_map(|parameter| match &parameter.effective_value {
                    Some(GraphResolvedParameterValue::Resource(resource)) => {
                        Some(resource.as_str())
                    }
                    _ => None,
                })
        {
            for (key, observed) in semantics
                .dependencies()
                .entries()
                .iter()
                .filter(|(key, _)| key.identity() == identity)
            {
                let (key, version) = match key {
                    GraphDependencyKey::Database(identity) => (
                        format!("database:{identity}"),
                        database_result_version(identity, *observed, databases),
                    ),
                    GraphDependencyKey::Function(identity) => {
                        (format!("function:{identity}"), *observed)
                    }
                    GraphDependencyKey::FunctionBody(identity) => {
                        (format!("functionBody:{identity}"), *observed)
                    }
                };
                versions.insert(key.into(), version);
            }
            if let Ok(path) = GraphResourcePath::new(identity)
                && visited.insert(path.clone())
                && let Some(function) = semantics.functions().get(&path)
            {
                for child in function.semantics.nodes() {
                    resources(child, semantics, databases, visited, versions);
                }
            }
        }
    }

    let semantics = analysis.semantic_snapshot();
    let output_ref = |port: &yss_graph_document::PortAddress| {
        PlanOutputRef::new(
            PlanGraphId::from_existing(graph.as_str().into()),
            PlanPortAddress::from_existing(port.to_string().into()),
        )
    };
    let mut outputs = BTreeMap::new();
    let mut observers = BTreeMap::new();
    for node in semantics.nodes() {
        let mut versions = BTreeMap::new();
        resources(
            node,
            semantics,
            databases,
            &mut BTreeSet::new(),
            &mut versions,
        );
        let available = semantics.nodes_ready(&BTreeSet::from([node.node_id]));
        let mut bindings: BTreeMap<PlanPortAddress, Vec<PlanOutputRef>> = BTreeMap::new();
        for input in &node.inputs {
            if let GraphResolvedInputSource::Output(source) = &input.source {
                bindings
                    .entry(PlanPortAddress::from_existing(
                        input.address.to_string().into(),
                    ))
                    .or_default()
                    .push(output_ref(source));
            }
        }
        let inputs = OutputResultInputs {
            fingerprint: yss_canonical_hash::hash_canonical(
                "yssbi.application-result-input.v2",
                &(
                    node.execution_fingerprint(),
                    registry_fingerprint,
                    analysis.kernel_fingerprint(),
                ),
            )
            .expect("result semantic fingerprints are serializable"),
            bindings: bindings
                .into_iter()
                .map(|(input, sources)| (input, sources.into_boxed_slice()))
                .collect(),
            resources: versions,
            available,
        };
        for port in node
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
        {
            outputs.insert(output_ref(&port.address), inputs.clone());
        }
        if node
            .ports
            .iter()
            .all(|port| port.direction != PortDirection::Output)
        {
            observers.insert(
                PlanNodeId::from_existing(node.node_id.to_string().into()),
                inputs,
            );
        }
    }
    GraphResultInputs {
        semantic_input_hash: *analysis.semantic_input_hash(),
        definition_input_hash: *analysis.definition_input_hash(),
        schema_observations: semantics
            .nodes()
            .iter()
            .flat_map(|node| &node.ports)
            .filter_map(|port| match &port.schema_state {
                yss_graph_analysis::GraphSchemaState::Observed { version, .. } => Some((
                    output_ref(&port.address),
                    yss_graph_execution::result::ResultId::from_existing(*version),
                )),
                _ => None,
            })
            .collect(),
        outputs,
        observers,
    }
}

fn database_result_version(
    identity: &str,
    schema: Option<[u8; 32]>,
    databases: &DatabaseCatalogSnapshot,
) -> Option<[u8; 32]> {
    let revision = databases
        .schemas()
        .iter()
        .find(|schema| identity.strip_prefix("databases/") == Some(schema.database().as_str()))?
        .runtime_revision()
        .get();
    Some(
        yss_canonical_hash::hash_canonical("yssbi.database-result-version.v1", &(schema, revision))
            .expect("database result versions are serializable"),
    )
}

pub(crate) fn result_resource_versions(
    captured: &crate::session::ApplicationSession,
    mut catalog: ResourceCatalogSnapshot,
    databases: &DatabaseCatalogSnapshot,
    keys: &BTreeSet<Box<str>>,
) -> Result<BTreeMap<Box<str>, Option<[u8; 32]>>, GraphContractMappingError> {
    use yss_graph_resource_contract::GraphDependencyKey;
    let mut versions = BTreeMap::new();
    for key in keys {
        let dependency = match key.split_once(':') {
            Some(("database", identity)) => GraphDependencyKey::Database(identity.into()),
            Some(("function", identity)) => GraphDependencyKey::Function(identity.into()),
            Some(("functionBody", identity)) => {
                if let Ok(path) = GraphResourcePath::new(identity)
                    && catalog.function_signature(&path).is_some()
                {
                    let resource = captured
                        .project()
                        .read_graph_resource_snapshot(captured.project_instance_id(), &path)
                        .map_err(|source| GraphContractMappingError::FunctionDocument {
                            graph: path.clone(),
                            source,
                        })?;
                    catalog = catalog.with_function_document(&path, resource.document);
                }
                GraphDependencyKey::FunctionBody(identity.into())
            }
            _ => {
                versions.insert(key.clone(), None);
                continue;
            }
        };
        let observed = catalog.observed_fingerprint(&dependency);
        let version = match &dependency {
            GraphDependencyKey::Database(identity) => {
                database_result_version(identity, observed, databases)
            }
            _ => observed,
        };
        versions.insert(key.clone(), version);
    }
    Ok(versions)
}

pub(crate) struct GraphResolutionContext {
    pub(super) project: crate::graph::catalog::LocalizedCatalogProjectFacts,
    pub(super) database: yss_database_runtime::session_api::DatabaseCatalogSnapshot,
    pub(super) graph_catalog: yss_graph_resource_contract::ResourceCatalogSnapshot,
    pub(super) basis: yss_graph_analysis_contract::GraphAnalysisBasis,
    pub(super) registry_fingerprint: [u8; 32],
}

impl GraphResolutionContext {
    pub(crate) fn capture(
        captured: &ApplicationSession,
        document: &GraphDocument,
    ) -> Result<Self, GraphInputError> {
        let mut context = Self::capture_catalog(captured)?;
        context.include_functions(captured, document)?;
        Ok(context)
    }

    pub(crate) fn capture_catalog(captured: &ApplicationSession) -> Result<Self, GraphInputError> {
        let project =
            capture_localized_project_facts(captured).map_err(GraphInputError::Catalog)?;
        Self::from_project_facts(captured, project)
    }

    pub(super) fn from_project_facts(
        captured: &ApplicationSession,
        project: crate::graph::catalog::LocalizedCatalogProjectFacts,
    ) -> Result<Self, GraphInputError> {
        let database = catalog_snapshot(captured.database()).map_err(GraphInputError::Database)?;
        yss_database_runtime::session_api::revalidate_declaration_observations(
            captured.database(),
            project.resources().database_observations(),
        )
        .map_err(GraphInputError::Database)?;
        let graph_catalog = build_resource_catalog(project.resources().graph(), &database)
            .map_err(GraphInputError::Contract)?;
        let registry_fingerprint = captured.graph().registry_fingerprint();
        let basis = yss_graph_analysis_contract::GraphAnalysisBasis {
            kernel_fingerprint: captured.execution().kernels().fingerprint().as_bytes(),
            registry_fingerprint: yss_node_registry::RegistryFingerprint::from_bytes(
                registry_fingerprint,
            ),
        };
        Ok(Self {
            project,
            database,
            graph_catalog,
            basis,
            registry_fingerprint,
        })
    }

    pub(crate) fn result_resource_versions(
        &self,
        captured: &ApplicationSession,
        keys: &std::collections::BTreeSet<Box<str>>,
    ) -> Result<BTreeMap<Box<str>, Option<[u8; 32]>>, GraphInputError> {
        crate::graph::inputs::result_resource_versions(
            captured,
            self.graph_catalog.clone(),
            &self.database,
            keys,
        )
        .map_err(GraphInputError::Contract)
    }

    pub(crate) fn include_functions(
        &mut self,
        captured: &ApplicationSession,
        document: &GraphDocument,
    ) -> Result<(), GraphInputError> {
        self.graph_catalog = crate::graph::inputs::capture_function_dependencies(
            captured,
            document,
            self.graph_catalog.clone(),
        )
        .map_err(GraphInputError::Contract)?;
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        captured: &ApplicationSession,
        graph_path: &GraphResourcePath,
        document: &std::sync::Arc<GraphDocument>,
        locale: &str,
    ) -> yss_graph_analysis::GraphAnalysis {
        let mut observations =
            schema_observations::capture(captured, graph_path, document, &self.graph_catalog);
        loop {
            let analysis = captured
                .graph()
                .resolve_graph_document_with_observations(
                    graph_path,
                    document,
                    &self.basis,
                    &self.graph_catalog,
                    self.project.resources().entries(),
                    locale,
                    &observations,
                )
                .map_semantic_snapshot(|semantics| {
                    semantics.with_execution_kernel_support(&|id| {
                        captured.execution().kernels().supports(id)
                    })
                });
            if observations.is_empty() {
                return analysis;
            }
            let matching = captured.execution().matching_schema_results(
                graph_path.as_str(),
                &graph_result_inputs(
                    graph_path,
                    &analysis,
                    &self.database,
                    self.registry_fingerprint,
                ),
            );
            let before = observations.len();
            observations.retain(|address, observation| {
                let output = PlanOutputRef::new(
                    PlanGraphId::from_existing(graph_path.as_str().into()),
                    PlanPortAddress::from_existing(address.to_string().into()),
                );
                matching
                    .get(&output)
                    .is_some_and(|id| id.get() == observation.version)
            });
            if before == observations.len() {
                return analysis;
            }
            // Only removal is permitted: stale facts and their dependents cannot
            // revive each other. Tentative analyses never update ResultStore.
        }
    }

    pub(crate) fn revalidate(&self, captured: &ApplicationSession) -> Result<(), GraphInputError> {
        crate::graph::catalog::revalidate_project_catalog_facts(captured, &self.project)
            .map_err(GraphInputError::Catalog)?;
        yss_database_runtime::session_api::revalidate_declaration_observations(
            captured.database(),
            self.project.resources().database_observations(),
        )
        .map_err(GraphInputError::Database)?;
        yss_database_runtime::session_api::revalidate_catalog_snapshot(
            captured.database(),
            &self.database,
        )
        .map_err(GraphInputError::Database)
    }
}

#[cfg(test)]
mod tests;
