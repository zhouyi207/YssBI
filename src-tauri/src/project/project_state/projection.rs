//! Projection environment capture and editor projection assembly.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::project) struct ProjectionEnvironmentExpectation {
    pub(in crate::project) project_instance_id: ProjectInstanceId,
    pub(in crate::project) project_root: Option<NormalizedProjectRoot>,
    pub(in crate::project) project_session_id: crate::node_system::ProjectSessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::project) struct ProjectionEnvironmentAuthorityBasis {
    pub(in crate::project) project_instance_id: String,
    pub(in crate::project) authority_generation: u64,
}

#[derive(Clone)]
pub(in crate::project) struct ProjectionEnvironmentSnapshot {
    pub(in crate::project) authority: ProjectionEnvironmentAuthorityBasis,
    pub(in crate::project) registry: Arc<crate::node_system::registry::NodeRegistry>,
    pub(in crate::project) catalog: Arc<crate::node_system::catalog::BuiltinCatalog>,
    pub(in crate::project) project_session_id: crate::node_system::ProjectSessionId,
    pub(in crate::project) database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
    #[cfg(test)]
    pub(in crate::project) projection_test_hook: Option<ProjectionTestHook>,
}

impl ProjectionEnvironmentSnapshot {
    pub(in crate::project) fn matches_publication(
        &self,
        publication: &MutationPublication,
    ) -> bool {
        self.authority.project_instance_id == publication.project_instance_id
            && self.authority.authority_generation == publication.authority_generation()
    }
}

#[derive(Clone)]
pub(in crate::project) struct ProjectionSourceSnapshot {
    pub(in crate::project) state: ProjectState,
    pub(in crate::project) data: ProjectData,
    pub(in crate::project) environment: ProjectionEnvironmentSnapshot,
    pub(in crate::project) project_instance_id: String,
    pub(in crate::project) authority_generation: u64,
    pub(in crate::project) graph_revisions: std::collections::HashMap<
        GraphResourcePath,
        crate::node_system::document::ResourceRevision,
    >,
    pub(in crate::project) variable_revisions:
        std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    pub(in crate::project) database_revisions: std::collections::HashMap<String, u64>,
}

impl ProjectionSourceSnapshot {
    pub(super) fn replacements(
        &self,
        graph_paths: &[String],
        locale: &str,
    ) -> Result<Vec<crate::event::GraphProjectionReplacementDto>, String> {
        graph_paths
            .iter()
            .map(|path| {
                let graph_path = GraphResourcePath::new(path).map_err(|error| error.to_string())?;
                Ok(crate::event::GraphProjectionReplacementDto {
                    graph_path: graph_path.as_str().to_string(),
                    projection: self.graph_projection(&graph_path, locale)?,
                    function_editor_projection: self.function_editor_projection(&graph_path)?,
                })
            })
            .collect()
    }

    fn function_editor_projection(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<Option<crate::node_system::analysis::FunctionEditorProjectionDto>, String> {
        self.data
            .graphs
            .get(graph_path)
            .and_then(|resource| resource.function.as_ref())
            .map(crate::node_system::analysis::build_function_editor_projection)
            .transpose()
    }

    pub(in crate::project) fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String> {
        #[cfg(test)]
        if let Some(hook) = self.environment.projection_test_hook.as_ref() {
            hook()?;
        }
        let document = self
            .data
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.clone())
            .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
        let (analysis, _) = self
            .state
            .get_or_compile_current_from_source(graph_path, self)?;
        EditorGraphProjectionDto::from_compilation_sources(
            graph_path.as_str(),
            &analysis.payload.analysis,
            &analysis.payload.outcome,
            &document,
            self.environment.registry.as_ref(),
            &self.environment.catalog.localization(locale),
            &self.data.computation_settings,
        )
        .map_err(|error| error.to_string())
    }
}

impl ProjectState {
    pub(in crate::project) fn current_projection_environment_expectation(
        &self,
    ) -> ProjectionEnvironmentExpectation {
        self.activation_identity.read().unwrap().clone()
    }

    fn projection_environment_expectation_for_session(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentExpectation, String> {
        self.projection_environment_expectation_for_identity(
            session.instance_id.as_str(),
            &session.root,
        )
    }

    pub(in crate::project) fn projection_environment_expectation_for_identity(
        &self,
        project_instance_id: &str,
        project_root: &NormalizedProjectRoot,
    ) -> Result<ProjectionEnvironmentExpectation, String> {
        let expected = self.current_projection_environment_expectation();
        if expected.project_instance_id.as_str() != project_instance_id
            || expected.project_root.as_ref() != Some(project_root)
        {
            return Err(
                "stale_project_lifecycle: project changed before projection environment capture"
                    .into(),
            );
        }
        Ok(expected)
    }

    pub(in crate::project) fn capture_projection_environment_for_session(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        self.capture_projection_environment(&expected)
    }

    pub(super) fn capture_projection_environment_for_execution_session(
        &self,
        session: &ProjectSession,
        expected_session_id: &crate::node_system::ProjectSessionId,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        if &expected.project_session_id != expected_session_id {
            return Err(
                "stale_project_lifecycle: execution session changed before projection environment capture"
                    .into(),
            );
        }
        self.capture_projection_environment(&expected)
    }

    pub(in crate::project) fn capture_projection_environment(
        &self,
        expected: &ProjectionEnvironmentExpectation,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        use crate::node_system::plan::ResourceId;
        use std::sync::atomic::Ordering;

        let mut capture_attempts = 0;
        loop {
            let generation_before = self.activation_generation.load(Ordering::Acquire);
            if generation_before % 2 != 0 {
                std::thread::yield_now();
                continue;
            }
            if capture_attempts == 3 {
                return Err(
                    "stale_project_lifecycle: authority changed repeatedly during projection environment capture"
                        .into(),
                );
            }
            capture_attempts += 1;

            let path = self.project_path.read().unwrap();
            #[cfg(test)]
            if let Some(hook) = self
                .test_hooks
                .projection_environment_capture_test_hook
                .read()
                .unwrap()
                .clone()
            {
                hook();
            }
            let project_path = path.clone();
            drop(path);
            let (authority, databases) = {
                let publication = self.mutation_publication.lock().unwrap();
                if publication.project_instance_id != expected.project_instance_id.as_str() {
                    return Err(
                        "stale_project_lifecycle: project changed before projection environment authority capture"
                            .into(),
                    );
                }
                let data = self.project_data.read().unwrap();
                (
                    ProjectionEnvironmentAuthorityBasis {
                        project_instance_id: publication.project_instance_id.clone(),
                        authority_generation: publication.authority_generation(),
                    },
                    data.databases.clone(),
                )
            };
            self.run_projection_environment_after_path_data_test_hook();

            let project_root = project_path
                .as_deref()
                .map(NormalizedProjectRoot::from_project_path)
                .transpose()
                .map_err(|error| error.to_string())?;
            let (registry, catalog, project_session_id, mut database_schemas) = {
                let store = self.project_store.read().unwrap();
                let schemas = store
                    .databases
                    .iter()
                    .filter_map(|(id, database)| {
                        if !databases.contains_key(id) {
                            return None;
                        }
                        let columns = match &database.state {
                            DatabaseState::DuckDb { columns, .. } => {
                                crate::schema::column_info_from_duckdb(columns)
                            }
                            DatabaseState::Loaded { dataframe, .. } => {
                                crate::schema::column_info_from_schema(dataframe.schema().as_ref())
                            }
                            DatabaseState::Failed { .. } => return None,
                        };
                        Some((id.clone(), columns))
                    })
                    .collect::<BTreeMap<_, _>>();
                (
                    Arc::clone(&store.node_registry),
                    Arc::clone(&store.catalog),
                    store.project_session_id.clone(),
                    schemas,
                )
            };
            let identity_after = self.activation_identity.read().unwrap().clone();
            let generation_after = self.activation_generation.load(Ordering::Acquire);
            if generation_before != generation_after || generation_after % 2 != 0 {
                if &identity_after != expected {
                    return Err("stale_project_lifecycle: project changed during projection environment capture"
                        .into());
                }
                continue;
            }
            if &identity_after != expected
                || project_root != expected.project_root
                || project_session_id != expected.project_session_id
            {
                return Err(
                    "stale_project_lifecycle: projection environment identity mismatch".into(),
                );
            }

            let mut metadata_error = None;
            for (id, declaration) in &databases {
                if database_schemas.contains_key(id) {
                    continue;
                }
                let crate::database_contract::DatabaseEngine::DuckDb { path, table } =
                    &declaration.engine
                else {
                    continue;
                };
                let Some(root) = project_root.as_ref() else {
                    metadata_error =
                        Some(format!("database '{id}' requires an active project path"));
                    break;
                };
                match crate::database::read_table_meta(&root.as_path().join(path), table) {
                    Ok(metadata) => {
                        database_schemas.insert(
                            id.clone(),
                            crate::schema::column_info_from_duckdb(&metadata.columns),
                        );
                    }
                    Err(error) => {
                        metadata_error = Some(error);
                        break;
                    }
                }
            }

            let final_generation = self.activation_generation.load(Ordering::Acquire);
            let final_identity = self.activation_identity.read().unwrap().clone();
            if final_generation != generation_after || final_generation % 2 != 0 {
                if &final_identity != expected {
                    return Err("stale_project_lifecycle: project changed during projection metadata capture"
                        .into());
                }
                continue;
            }
            if &final_identity != expected {
                return Err(
                    "stale_project_lifecycle: projection metadata identity mismatch".into(),
                );
            }
            let authority_is_current = {
                let publication = self.mutation_publication.lock().unwrap();
                authority.project_instance_id == publication.project_instance_id
                    && authority.authority_generation == publication.authority_generation()
            };
            if !authority_is_current {
                continue;
            }
            if let Some(error) = metadata_error {
                return Err(error);
            }

            let database_schemas = database_schemas
                .into_iter()
                .map(|(id, columns)| {
                    ResourceId::new(format!("databases/{id}"))
                        .map(|resource| (resource, columns))
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<_, _>>()?;
            return Ok(ProjectionEnvironmentSnapshot {
                authority,
                registry,
                catalog,
                project_session_id,
                database_schemas,
                #[cfg(test)]
                projection_test_hook: self.test_hooks.projection_test_hook.read().unwrap().clone(),
            });
        }
    }

    pub(in crate::project) fn projection_source_snapshot(
        &self,
        data: &ProjectData,
        environment: ProjectionEnvironmentSnapshot,
        project_instance_id: String,
        authority_generation: u64,
        graph_revisions: std::collections::HashMap<
            GraphResourcePath,
            crate::node_system::document::ResourceRevision,
        >,
        variable_revisions: std::collections::HashMap<
            crate::variable::VariableId,
            VariableRevisionEntry,
        >,
        database_revisions: std::collections::HashMap<String, u64>,
    ) -> ProjectionSourceSnapshot {
        ProjectionSourceSnapshot {
            state: self.clone(),
            data: data.clone(),
            environment,
            project_instance_id,
            authority_generation,
            graph_revisions,
            variable_revisions,
            database_revisions,
        }
    }

    #[cfg(test)]
    fn run_projection_environment_after_path_data_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .projection_environment_after_path_data_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_projection_environment_after_path_data_test_hook(&self) {}

    #[cfg(test)]
    pub(in crate::project) fn capture_projection_environment_for_test(
        &self,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.current_projection_environment_expectation();
        self.capture_projection_environment(&expected)
    }

    #[cfg(test)]
    pub(in crate::project) fn capture_projection_environment_for_session_for_test(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectionEnvironmentSnapshot, String> {
        let expected = self.projection_environment_expectation_for_session(session)?;
        self.capture_projection_environment(&expected)
    }

    pub fn graph_projection_for_project(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "graph hydrate project instance is stale".into(),
            });
        }
        let projection = self
            .capture_projection_source(graph_path)
            .and_then(|source| source.graph_projection(graph_path, locale));
        match projection {
            Ok(projection) => {
                self.validate_project_session(&session)?;
                Ok(projection)
            }
            Err(message) => {
                self.validate_project_session(&session)?;
                Err(ProjectFilesystemError::TransactionPrepareFailed { message })
            }
        }
    }

    pub fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String> {
        self.ensure_project_operational()
            .map_err(|error| format!("{}: {error}", error.code()))?;
        self.capture_projection_source(graph_path)?
            .graph_projection(graph_path, locale)
    }
}

pub(in crate::project) fn candidate_projection_replacement(
    source: &ProjectionSourceSnapshot,
    graph_path: &GraphResourcePath,
    locale: &str,
) -> Result<crate::event::GraphProjectionReplacementDto, MutationConflict> {
    #[cfg(test)]
    if let Some(hook) = source.environment.projection_test_hook.as_ref() {
        hook().map_err(|error| MutationConflict::Projection(error.into()))?;
    }
    let document = &source
        .data
        .graphs
        .get(graph_path)
        .ok_or_else(|| MutationConflict::Projection("candidate graph is not loaded".into()))?
        .document;
    let resources = compile_resources_from_projection_snapshot(source)
        .map_err(|error| MutationConflict::Projection(error.into()))?;
    let compiler = GraphCompiler::with_resolvers(
        source.environment.registry.as_ref(),
        &resources,
        resources.schema_resolvers(),
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    )
    .with_project_session_id(source.environment.project_session_id.clone());
    let snapshot = compiler.snapshot_with_compile_id(
        crate::node_system::analysis::CompileId::new(source.authority_generation),
        crate::node_system::document::GraphResourcePath(graph_path.as_str().into()),
        document,
    );
    let compiled = compiler
        .compile_snapshot(
            &snapshot,
            &crate::node_system::compiler::CompileCancellationToken::new(),
        )
        .map_err(|error| MutationConflict::Projection(error.to_string().into()))?;
    let projection = EditorGraphProjectionDto::from_compilation_sources(
        graph_path.as_str(),
        &compiled.analysis,
        &compiled.outcome,
        document,
        source.environment.registry.as_ref(),
        &source.environment.catalog.localization(locale),
        &source.data.computation_settings,
    )
    .map_err(|error| MutationConflict::Projection(error.to_string().into()))?;
    let function_editor_projection = source
        .data
        .graphs
        .get(graph_path)
        .and_then(|resource| resource.function.as_ref())
        .map(crate::node_system::analysis::build_function_editor_projection)
        .transpose()
        .map_err(|error| MutationConflict::Projection(error.into()))?;
    Ok(crate::event::GraphProjectionReplacementDto {
        graph_path: graph_path.as_str().to_owned(),
        projection,
        function_editor_projection,
    })
}
