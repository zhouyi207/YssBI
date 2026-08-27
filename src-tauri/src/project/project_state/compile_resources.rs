use super::*;

#[derive(Clone)]
pub(in crate::project) struct CompileResourceSnapshot {
    pub(in crate::project) versions: crate::node_system::analysis::ResourceVersionSet,
    pub(in crate::project::project_state) resource_states:
        crate::node_system::analysis::ResourceObservationSet,
    pub(in crate::project::project_state) function_names:
        BTreeMap<crate::graph_document::GraphResourcePath, Box<str>>,
    pub(in crate::project::project_state) functions: BTreeMap<
        crate::graph_document::GraphResourcePath,
        crate::node_system::document::FunctionDocument,
    >,
    pub(in crate::project::project_state) function_graphs:
        BTreeMap<crate::graph_document::GraphResourcePath, crate::graph_document::GraphDocument>,
    pub(in crate::project::project_state) variables:
        std::collections::HashMap<crate::variable::VariableId, crate::variable::VariableInstance>,
    pub(in crate::project::project_state) database_names:
        BTreeMap<crate::node_system::plan::ResourceId, Box<str>>,
    pub(in crate::project::project_state) database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
}

impl CompileResourceSnapshot {
    pub(in crate::project) fn schema_resolvers(
        &self,
    ) -> crate::node_system::compiler::SchemaResolverSet {
        let mut resolvers = crate::node_system::compiler::SchemaResolverSet::new();
        resolvers.insert(
            crate::node_system::protocol::SchemaResolverId::new(
                crate::node_system::catalog::DATAFRAME_RESOURCE_SCHEMA_RESOLVER,
            )
            .expect("built-in dataframe schema resolver ID is valid"),
            ProjectDatabaseSchemaResolver,
        );
        resolvers
    }
}

struct ProjectDatabaseSchemaResolver;

impl crate::node_system::compiler::SchemaResolver for ProjectDatabaseSchemaResolver {
    fn resolve(
        &self,
        context: &mut crate::node_system::compiler::SchemaResolutionContext<'_, '_>,
    ) -> Result<
        crate::node_system::compiler::SchemaFact,
        crate::node_system::compiler::SchemaResolutionError,
    > {
        let resource = context
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "dataframe")
            .and_then(|(_, value)| value.as_str())
            .ok_or_else(|| {
                crate::node_system::compiler::SchemaResolutionError::new(
                    "dataframe source requires a database resource",
                )
            })?;
        let id = resource.strip_prefix("databases/").ok_or_else(|| {
            crate::node_system::compiler::SchemaResolutionError::new(format!(
                "database resource '{resource}' is not canonical"
            ))
        })?;
        let fields = context
            .resources
            .as_deref_mut()
            .ok_or_else(|| {
                crate::node_system::compiler::SchemaResolutionError::new(
                    "database schema resolution requires analysis resources",
                )
            })?
            .resolve_database(id)
            .map_err(|error| {
                crate::node_system::compiler::SchemaResolutionError::from_resource(&error)
            })?;
        let fields = fields.value.columns;
        Ok(crate::node_system::compiler::SchemaFact::new(
            crate::node_system::protocol::SchemaExpr::Input(
                crate::node_system::protocol::PortKey::new("dataframe").unwrap(),
            ),
            fields
                .iter()
                .map(|column| crate::node_system::protocol::SchemaField {
                    name: crate::node_system::protocol::SchemaColumnRef(column.name.clone().into()),
                    scalar_type:
                        crate::node_system::protocol::RelationalScalarType::from_database_dtype(
                            &column.dtype,
                        ),
                    lineage: Some(crate::node_system::protocol::SchemaFieldLineage {
                        source: resource.into(),
                        field: column.name.clone().into(),
                    }),
                }),
        ))
    }
}

impl ResourceSnapshot for CompileResourceSnapshot {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        self.versions.clone()
    }

    fn version(
        &self,
        key: &crate::node_system::analysis::ResourceKey,
    ) -> Option<crate::node_system::analysis::ResourceVersion> {
        self.versions.get(key).cloned()
    }

    fn observed_state(
        &self,
        key: &crate::node_system::analysis::ResourceKey,
    ) -> crate::node_system::analysis::ResourceObservedState {
        self.resource_states.get(key).cloned().unwrap_or(
            crate::node_system::analysis::ResourceObservedState::Absent(None),
        )
    }

    fn function_name(&self, path: &crate::graph_document::GraphResourcePath) -> Option<&str> {
        self.function_names.get(path).map(AsRef::as_ref)
    }

    fn function_document(
        &self,
        path: &crate::graph_document::GraphResourcePath,
    ) -> Option<&crate::node_system::document::FunctionDocument> {
        self.functions.get(path)
    }

    fn function_graph_document(
        &self,
        path: &crate::graph_document::GraphResourcePath,
    ) -> Option<&crate::graph_document::GraphDocument> {
        self.function_graphs.get(path)
    }

    fn variable(
        &self,
        id: &crate::variable::VariableId,
    ) -> Option<&crate::variable::VariableInstance> {
        self.variables.get(id)
    }

    fn database_name(&self, id: &str) -> Option<&str> {
        let resource = crate::node_system::plan::ResourceId::new(format!("databases/{id}")).ok()?;
        self.database_names.get(&resource).map(AsRef::as_ref)
    }

    fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        let resource = crate::node_system::plan::ResourceId::new(format!("databases/{id}")).ok()?;
        self.database_schemas.get(&resource).map(Vec::as_slice)
    }
}

pub(in crate::project) struct ProductionPlotSink;

impl crate::node_system::runtime::PlotSink for ProductionPlotSink {
    fn publish(
        &self,
        _kind: crate::node_system::runtime::PlotKind,
        payload: &str,
    ) -> Result<Box<str>, crate::node_system::runtime::PlotPublishError> {
        Ok(payload.into())
    }
}

pub(in crate::project) struct ProductionResourceSnapshots {
    pub(in crate::project::project_state) compile: CompileResourceSnapshot,
    pub(in crate::project) runtime: crate::node_system::runtime::ProjectResourceSnapshot,
}

pub(in crate::project) fn compile_resources_from_projection_snapshot(
    source: &ProjectionSourceSnapshot,
) -> Result<CompileResourceSnapshot, String> {
    let mut resources =
        compile_resources_from_data(&source.data, source.environment.database_schemas.clone())?;
    apply_compile_resource_authority(
        &mut resources,
        &source.data,
        source.graph_revisions.clone(),
        source.variable_revisions.clone(),
        source.database_revisions.clone(),
    );
    Ok(resources)
}

pub(in crate::project::project_state) fn apply_compile_resource_authority(
    resources: &mut CompileResourceSnapshot,
    data: &ProjectData,
    graph_revisions: std::collections::HashMap<
        GraphResourcePath,
        crate::graph_document::GraphRevision,
    >,
    variable_revisions: std::collections::HashMap<
        crate::variable::VariableId,
        VariableRevisionEntry,
    >,
    database_revisions: std::collections::HashMap<String, u64>,
) {
    use crate::node_system::analysis::{
        ResourceKey as AnalysisResourceKey, ResourceObservedState, ResourceVersion,
    };

    resources.versions.clear();
    resources.resource_states.clear();
    for (path, revision) in graph_revisions {
        if !path.as_str().starts_with("functions/") {
            continue;
        }
        let key = AnalysisResourceKey::new(path.as_str());
        let version = ResourceVersion::new(format!("revision:{}", revision.get()));
        if data
            .graphs
            .get(&path)
            .is_some_and(|resource| resource.function.is_some())
        {
            resources.versions.insert(key.clone(), version.clone());
            resources
                .resource_states
                .insert(key, ResourceObservedState::Present(version));
        } else {
            resources
                .resource_states
                .insert(key, ResourceObservedState::Absent(Some(version)));
        }
    }
    for (id, entry) in variable_revisions {
        let key = AnalysisResourceKey::new(format!("variables/{id}"));
        let version = ResourceVersion::new(format!("revision:{}", entry.revision.get()));
        if entry.is_present() && data.variables.contains_key(&id) {
            resources.versions.insert(key.clone(), version.clone());
            resources
                .resource_states
                .insert(key, ResourceObservedState::Present(version));
        } else {
            resources
                .resource_states
                .insert(key, ResourceObservedState::Absent(Some(version)));
        }
    }
    for (id, revision) in database_revisions {
        let key = AnalysisResourceKey::new(format!("databases/{id}"));
        let version = ResourceVersion::new(format!("revision:{revision}"));
        if data.databases.contains_key(&id) {
            resources.versions.insert(key.clone(), version.clone());
            resources
                .resource_states
                .insert(key, ResourceObservedState::Present(version));
        } else {
            resources
                .resource_states
                .insert(key, ResourceObservedState::Absent(Some(version)));
        }
    }
}

pub(in crate::project) fn compile_resources_from_data(
    data: &ProjectData,
    database_schemas: BTreeMap<
        crate::node_system::plan::ResourceId,
        Vec<crate::schema::ColumnInfoDTO>,
    >,
) -> Result<CompileResourceSnapshot, String> {
    use crate::node_system::analysis::{ResourceKey as AnalysisResourceKey, ResourceVersion};

    let function_names = data
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph
                .function
                .as_ref()
                .map(|_| (path.clone(), graph.name.clone().into_boxed_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let functions = data
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph
                .function
                .clone()
                .map(|function| (path.clone(), function))
        })
        .collect::<BTreeMap<_, _>>();
    let function_graphs = data
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph
                .function
                .as_ref()
                .map(|_| (path.clone(), graph.document.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut versions = crate::node_system::analysis::ResourceVersionSet::new();
    for (path, function) in &functions {
        let graph_path =
            GraphResourcePath::new(path.as_str()).map_err(|error| error.to_string())?;
        let graph = data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("function '{}' graph is not loaded", graph_path))?;
        let version = serde_json::to_string(&(graph.name.as_str(), function, &graph.document))
            .map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(path.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, variable) in &data.variables {
        let key = format!("variables/{id}");
        let version = serde_json::to_string(variable).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, declaration) in &data.databases {
        let key = format!("databases/{id}");
        let resource = crate::node_system::plan::ResourceId::new(key.as_str())
            .map_err(|error| error.to_string())?;
        let schema = database_schemas
            .get(&resource)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let version =
            serde_json::to_string(&(declaration, schema)).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }

    let database_names = data
        .databases
        .iter()
        .filter_map(|(id, declaration)| {
            crate::node_system::plan::ResourceId::new(format!("databases/{id}"))
                .ok()
                .map(|resource| (resource, declaration.name.clone().into()))
        })
        .collect();
    let resource_states = versions
        .iter()
        .map(|(key, version)| {
            (
                key.clone(),
                crate::node_system::analysis::ResourceObservedState::Present(version.clone()),
            )
        })
        .collect();
    Ok(CompileResourceSnapshot {
        versions,
        resource_states,
        function_names,
        functions,
        function_graphs,
        variables: data.variables.clone(),
        database_names,
        database_schemas,
    })
}

#[cfg(test)]
pub(in crate::project) fn snapshot_project_resources(
    state: &ProjectState,
    variables: std::collections::HashMap<
        crate::variable::VariableId,
        crate::variable::VariableInstance,
    >,
    databases: std::collections::HashMap<String, crate::database_contract::DatabaseDecl>,
) -> Result<ProductionResourceSnapshots, String> {
    use crate::node_system::analysis::{ResourceKey as AnalysisResourceKey, ResourceVersion};
    use crate::node_system::plan::ResourceId;

    let (session_id, loaded_databases) = {
        let store = state.project_store.read().unwrap();
        let loaded = store
            .databases
            .iter()
            .filter_map(|(id, database)| match &database.state {
                DatabaseState::Loaded { dataframe, .. } => Some((
                    id.clone(),
                    Arc::clone(dataframe),
                    crate::schema::column_info_from_schema(dataframe.schema().as_ref()),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        (store.project_session_id.clone(), loaded)
    };

    let (function_names, function_resources, function_graphs) = {
        let data = state.project_data.read().unwrap();
        let names = data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph
                    .function
                    .as_ref()
                    .map(|_| (path.clone(), graph.name.clone().into_boxed_str()))
            })
            .collect::<BTreeMap<_, _>>();
        let resources = data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph
                    .function
                    .clone()
                    .map(|function| (path.clone(), function))
            })
            .collect::<BTreeMap<_, _>>();
        let graphs = data
            .graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph
                    .function
                    .as_ref()
                    .map(|_| (path.clone(), graph.document.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        (names, resources, graphs)
    };
    let mut versions = crate::node_system::analysis::ResourceVersionSet::new();
    for (path, function) in &function_resources {
        let graph = function_graphs
            .get(path)
            .ok_or_else(|| format!("function '{}' graph is not loaded", path.as_str()))?;
        let name = function_names
            .get(path)
            .ok_or_else(|| format!("function '{}' name is missing", path.as_str()))?;
        let version =
            serde_json::to_string(&(name, function, graph)).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(path.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, variable) in &variables {
        let key = format!("variables/{id}");
        let version = serde_json::to_string(variable).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, declaration) in &databases {
        let key = format!("databases/{id}");
        let version = serde_json::to_string(declaration).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }

    let project_root = state
        .get_path()
        .map(|path| crate::project::project_root_from_path(&path));
    let database_names = databases
        .iter()
        .filter_map(|(id, declaration)| {
            ResourceId::new(format!("databases/{id}"))
                .ok()
                .map(|resource| (resource, declaration.name.clone().into()))
        })
        .collect();
    let mut database_schemas = BTreeMap::new();
    let variable_revisions = state.variable_revisions.read().unwrap().clone();
    let compile_variables = variables.clone();
    let mut runtime =
        crate::node_system::runtime::ProjectResourceSnapshot::new(session_id, versions.clone())
            .with_plot_sink(Arc::new(ProductionPlotSink));
    for (id, variable) in variables {
        runtime = runtime.with_variable_revision(
            ResourceId::new(format!("variables/{id}")).map_err(|error| error.to_string())?,
            Arc::new(variable),
            variable_revisions
                .get(&id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::project::ResourceRevision::INITIAL),
        );
    }
    for (id, dataframe, columns) in loaded_databases {
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(resource.clone(), columns);
        runtime = runtime.with_database(resource, dataframe);
    }
    for (id, declaration) in databases {
        let crate::database_contract::DatabaseEngine::DuckDb { path, table } = declaration.engine
        else {
            continue;
        };
        let root = project_root
            .as_ref()
            .ok_or_else(|| format!("database '{id}' requires an active project path"))?;
        let absolute = root.join(path);
        let metadata = crate::database::read_table_meta(&absolute, &table)?;
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(
            resource.clone(),
            crate::schema::column_info_from_duckdb(&metadata.columns),
        );
        runtime =
            runtime.with_duckdb_database(resource, absolute.to_string_lossy().into_owned(), table);
    }

    Ok(ProductionResourceSnapshots {
        compile: CompileResourceSnapshot {
            resource_states: versions
                .iter()
                .map(|(key, version)| {
                    (
                        key.clone(),
                        crate::node_system::analysis::ResourceObservedState::Present(
                            version.clone(),
                        ),
                    )
                })
                .collect(),
            versions,
            function_names,
            functions: function_resources,
            function_graphs,
            variables: compile_variables,
            database_names,
            database_schemas,
        },
        runtime,
    })
}
