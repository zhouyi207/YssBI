use super::*;

struct ExecutionSnapshot {
    document: crate::node_system::document::GraphDocument,
    data: ProjectData,
    database_instances: std::collections::HashMap<String, crate::database::DatabaseInstance>,
    graph_revisions: std::collections::HashMap<
        GraphResourcePath,
        crate::node_system::document::ResourceRevision,
    >,
    variable_revisions:
        std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    database_revisions: std::collections::HashMap<String, u64>,
    project_root: Option<NormalizedProjectRoot>,
    database_schemas:
        BTreeMap<crate::node_system::plan::ResourceId, Vec<crate::schema::ColumnInfoDTO>>,
    registry: Arc<crate::node_system::registry::NodeRegistry>,
    kernels: Arc<crate::node_system::runtime::KernelRegistry>,
    functions: Arc<crate::node_system::runtime::FunctionPlanStore>,
    results: crate::node_system::runtime::ResultStore,
    memoization: Arc<crate::node_system::runtime::SessionMemoization>,
    runs: Arc<crate::node_system::runtime::ProjectRunRegistry>,
    session_id: crate::node_system::ProjectSessionId,
}

impl ProjectState {
    pub fn cancel_graph_run(&self, run_id: crate::node_system::runtime::RunId) -> bool {
        let (runs, project_session_id) = self.current_run_registry();
        runs.cancel_run(&project_session_id, run_id)
    }

    pub(in crate::project) fn current_run_registry(
        &self,
    ) -> (
        Arc<crate::node_system::runtime::ProjectRunRegistry>,
        crate::node_system::ProjectSessionId,
    ) {
        let store = self
            .project_store
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (Arc::clone(&store.runs), store.project_session_id.clone())
    }

    fn capture_execution_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        compilation: &crate::project::compile_publication::CurrentCompilation,
    ) -> Result<ExecutionSnapshot, ProjectExecutionError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str()
            || publication.project_instance_id != compilation.authority.project_instance_id
        {
            return Err(ProjectExecutionError::stale_project_lifecycle(
                "execution authority changed before snapshot",
            ));
        }
        let data = self.project_data.read().unwrap().clone();
        let graph_revisions = self.graph_revisions.read().unwrap().clone();
        let variable_revisions = self.variable_revisions.read().unwrap().clone();
        let database_revisions = self.database_authority_revisions.read().unwrap().clone();
        let store = self.project_store.read().unwrap();
        let database_instances = store.databases.clone();
        let registry = Arc::clone(&store.node_registry);
        let kernels = Arc::clone(&store.kernels);
        let functions = Arc::clone(&store.function_plans);
        let results = store.results.clone();
        let memoization = Arc::clone(&store.memoization);
        let runs = Arc::clone(&store.runs);
        let session_id = store.project_session_id.clone();
        drop(store);
        let identity = self.current_projection_environment_expectation();
        if !self.execution_authority_matches(&publication, &compilation.authority)
            || session_id != compilation.authority.project_session_id
        {
            return Err(ProjectExecutionError::stale_project_lifecycle(
                "execution authority changed before snapshot",
            ));
        }
        let document = data
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.clone())
            .ok_or_else(|| {
                ProjectExecutionError::internal(format!("graph '{}' not loaded", graph_path))
            })?;
        Ok(ExecutionSnapshot {
            document,
            data,
            database_instances,
            graph_revisions,
            variable_revisions,
            database_revisions,
            project_root: identity.project_root,
            database_schemas: compilation.source.environment.database_schemas.clone(),
            registry,
            kernels,
            functions,
            results,
            memoization,
            runs,
            session_id,
        })
    }

    fn validate_execution_authority(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        authority: &crate::project::compile_publication::ExecutionAuthorityToken,
    ) -> Result<(), ProjectExecutionError> {
        let publication = self.mutation_publication.lock().unwrap();
        (publication.project_instance_id == expected_project_instance_id.as_str()
            && self.execution_authority_matches(&publication, authority))
        .then_some(())
        .ok_or_else(|| {
            ProjectExecutionError::stale_project_lifecycle("execution authority changed before run")
        })
    }

    fn load_function_resources(
        &self,
        cancellation: &crate::node_system::runtime::CancellationToken,
    ) -> Result<(), String> {
        cancellation.check().map_err(|error| error.to_string())?;
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let loaded_paths = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let function_paths = self
            .read_project_index(&session.instance_id)
            .map_err(|error| error.to_string())?
            .graphs
            .into_iter()
            .filter(|entry| entry.graph_type == crate::project::GraphDocumentKind::Function)
            .map(|entry| GraphResourcePath::new(entry.path).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        for path in function_paths {
            if loaded_paths.contains(&path) {
                continue;
            }
            cancellation.check().map_err(|error| error.to_string())?;
            let guard = self
                .resource_lifecycle
                .allocate_and_register(&session, &path, ResourceLifecycleIntent::Load)
                .map_err(|error| error.to_string())?;
            let operation = ResourceLifecycleOperation::from_guard(session.clone(), &guard);
            let cached = self.project_data.read().unwrap().graphs.contains_key(&path);
            let before_commit = || {
                cancellation.check().map_err(|error| {
                    ProjectFilesystemError::StaleProjectLifecycle {
                        message: error.to_string(),
                    }
                })?;
                #[cfg(test)]
                if let Some(checkpoint) = self
                    .test_hooks
                    .function_load_checkpoint
                    .read()
                    .unwrap()
                    .clone()
                {
                    checkpoint(cancellation);
                }
                cancellation.check().map_err(|error| {
                    ProjectFilesystemError::StaleProjectLifecycle {
                        message: error.to_string(),
                    }
                })
            };
            self.load_graph_for_registered_lifecycle_commit(
                operation,
                guard,
                cached,
                false,
                Some(&before_commit),
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn execute_graph_for_current_project_for_test(
        &self,
        graph_path: &GraphResourcePath,
        demand: &crate::node_system::plan::ExecutionDemand,
        events: &dyn crate::node_system::runtime::RunEventSink,
    ) -> Result<crate::node_system::runtime::RunResult, ProjectExecutionError> {
        let project_instance_id = self
            .capture_project_session()
            .map_err(ProjectExecutionError::from)?
            .instance_id;
        self.execute_graph(&project_instance_id, graph_path, demand, events)
    }

    pub fn execute_graph(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        demand: &crate::node_system::plan::ExecutionDemand,
        events: &dyn crate::node_system::runtime::RunEventSink,
    ) -> Result<crate::node_system::runtime::RunResult, ProjectExecutionError> {
        self.ensure_project_operational()
            .map_err(ProjectExecutionError::from)?;
        let session = self
            .capture_project_session()
            .map_err(ProjectExecutionError::from)?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectExecutionError::stale_project_lifecycle(
                "execution caller project is stale",
            ));
        }
        let cancellation = crate::node_system::runtime::CancellationToken::new();
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str() {
            return Err(ProjectExecutionError::stale_project_lifecycle(
                "execution authority changed before preparation",
            ));
        }
        let store = self.project_store.read().unwrap();
        let session_id = store.project_session_id.clone();
        let runs = Arc::clone(&store.runs);
        let preparation = runs
            .track_pre_run(session_id.clone(), cancellation.clone())
            .map_err(crate::node_system::runtime::RunError::from)?;
        drop(store);
        drop(publication);

        self.load_function_resources(&cancellation)?;
        let compilation = self.get_or_compile_current(graph_path, &session_id)?;
        let product = match &compilation.analysis.payload.outcome {
            crate::node_system::compiler::CompilationOutcome::Succeeded => compilation
                .plan
                .as_ref()
                .map(|projection| Arc::clone(&projection.payload))
                .ok_or_else(|| {
                    ProjectExecutionError::internal_compilation(
                        crate::node_system::compiler::InternalCompilationFailure {
                            stage: crate::node_system::compiler::CompilationStage::Lowering,
                            code: "project.execution.compilation_plan_missing".into(),
                            node_id: None,
                        },
                    )
                })?,
            crate::node_system::compiler::CompilationOutcome::AnalysisBlocked => {
                let codes = compilation
                    .analysis
                    .payload
                    .analysis
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "execution refused because graph has blocking diagnostics: {codes}"
                )
                .into());
            }
            crate::node_system::compiler::CompilationOutcome::InternalFailure(failure) => {
                return Err(ProjectExecutionError::internal_compilation(failure.clone()));
            }
        };
        let selected = product
            .select(demand)
            .map_err(|error| ProjectExecutionError::invalid_demand(error.to_string()))?;
        let plan = selected.plan;
        cancellation.check().map_err(|error| error.to_string())?;
        let execution = self.capture_execution_snapshot(
            expected_project_instance_id,
            graph_path,
            &compilation,
        )?;
        let mut compile_resources =
            compile_resources_from_data(&execution.data, execution.database_schemas.clone())?;
        super::compile_resources::apply_compile_resource_authority(
            &mut compile_resources,
            &execution.data,
            execution.graph_revisions.clone(),
            execution.variable_revisions.clone(),
            execution.database_revisions.clone(),
        );
        let resource_basis_matches = compilation
            .authority
            .basis
            .resource_versions
            .iter()
            .all(|(key, expected)| compile_resources.versions.get(key) == Some(expected));
        if !resource_basis_matches {
            return Err(ProjectExecutionError::stale_project_lifecycle(
                "execution resource basis changed",
            ));
        }
        let resource_snapshot = snapshot_execution_resources(&execution, compile_resources)?;
        let compile_cancellation =
            crate::node_system::compiler::CompileCancellationToken::from_shared(
                cancellation.shared_flag(),
            );
        let mut compiled_parameters = crate::node_system::runtime::CompiledParameterStore::new();
        let function_generation = publish_function_plans(
            execution.registry.as_ref(),
            execution.functions.as_ref(),
            &resource_snapshot.compile,
            Some(plan.as_ref()),
            execution.session_id.clone(),
            &compile_cancellation,
            &execution.data.computation_settings,
            &mut compiled_parameters,
        )?;
        #[cfg(test)]
        let production_relational_observer = self
            .test_hooks
            .production_relational_observer
            .read()
            .unwrap()
            .clone();
        #[cfg(test)]
        if let Some(observer) = &production_relational_observer {
            observer.observe_plan(plan.as_ref());
        }
        #[cfg(test)]
        let mut resources =
            crate::node_system::runtime::ProjectResourceProvider::new(resource_snapshot.runtime);
        #[cfg(not(test))]
        let resources =
            crate::node_system::runtime::ProjectResourceProvider::new(resource_snapshot.runtime);
        #[cfg(test)]
        if let Some(observer) = self
            .test_hooks
            .project_resource_lease_observer
            .read()
            .unwrap()
            .clone()
        {
            resources.set_lease_observer(observer);
        }
        build_run_parameters(
            &mut compiled_parameters,
            &execution.document,
            plan.as_ref(),
            &execution.data.computation_settings,
        )?;
        let mut relational_backends = crate::node_system::runtime::RelationalBackendRegistry::new();
        #[cfg(test)]
        let production_relational_backend = self
            .test_hooks
            .production_relational_backend_factory
            .read()
            .unwrap()
            .clone()
            .map(|factory| factory())
            .unwrap_or_else(|| {
                Arc::new(
                    production_relational_observer
                        .map(
                            crate::node_system::runtime::ProductionRelationalBackend::with_observer,
                        )
                        .unwrap_or_default(),
                )
            });
        #[cfg(test)]
        relational_backends
            .register_shared_for_test(
                crate::node_system::plan::RelationalBackendId::new("relational.default")
                    .map_err(|error| error.to_string())?,
                production_relational_backend,
            )
            .map_err(|error| error.to_string())?;
        #[cfg(not(test))]
        relational_backends
            .register(
                crate::node_system::plan::RelationalBackendId::new("relational.default")
                    .map_err(|error| error.to_string())?,
                crate::node_system::runtime::ProductionRelationalBackend::default(),
            )
            .map_err(|error| error.to_string())?;
        self.run_execution_before_final_gate_test_hook();
        self.validate_execution_authority(expected_project_instance_id, &compilation.authority)?;
        drop(preparation);
        self.run_execution_before_run_test_hook();
        let pre_run = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != expected_project_instance_id.as_str()
                || !self.execution_authority_matches(&publication, &compilation.authority)
            {
                return Err(ProjectExecutionError::stale_project_lifecycle(
                    "execution authority changed before run registration",
                ));
            }
            let store = self.project_store.read().unwrap();
            if store.project_session_id != execution.session_id
                || !Arc::ptr_eq(&runs, &execution.runs)
            {
                return Err(ProjectExecutionError::stale_project_lifecycle(
                    "execution session changed before run registration",
                ));
            }
            execution
                .runs
                .track_pre_run(execution.session_id.clone(), cancellation.clone())
                .map_err(crate::node_system::runtime::RunError::from)?
        };
        let prepared_authority = std::cell::RefCell::new(None);
        let prepare =
            |_: &mut crate::node_system::runtime::RunResult,
             cancellation: &crate::node_system::runtime::CancellationToken,
             deadline: Option<crate::node_system::runtime::RunDeadline>| {
                self.run_execution_before_commit_gate_test_hook();
                if let Some(deadline) = deadline {
                    deadline.check(
                        cancellation,
                        crate::node_system::runtime::RunPhase::ResultPublication,
                    )?;
                }
                let finalization = pre_run
                    .begin_finalization(cancellation)
                    .map_err(crate::node_system::runtime::RunError::from)?;
                let terminal = Some((cancellation, deadline));
                let effects = resources.snapshot().variable_effects();
                let authority = self
                    .prepare_variable_effects_receipt(&execution.session_id, effects, terminal)
                    .map_err(variable_effect_run_error)?;
                prepared_authority.replace(Some((finalization, authority)));
                Ok(())
            };
        let finalize =
            |result: &mut crate::node_system::runtime::RunResult,
             cancellation: &crate::node_system::runtime::CancellationToken,
             deadline: Option<crate::node_system::runtime::RunDeadline>| {
                let mut prepared = prepared_authority.borrow_mut();
                let (_finalization, authority) = prepared
                    .as_mut()
                    .expect("project success authority was prepared before finalization");
                let committed =
                    authority(Some((cancellation, deadline))).map_err(variable_effect_run_error)?;
                result.committed_variable_ids = committed.variable_ids;
                result.resource_mutation = committed.resource_mutation;
                Ok(())
            };
        crate::node_system::runtime::RunExecutor::new(
            execution.kernels.as_ref(),
            &resources,
            &function_generation,
            execution.results.clone(),
            Arc::clone(&execution.memoization),
        )
        .with_relational_backends(&relational_backends)
        .with_compiled_parameters(&compiled_parameters)
        .with_run_registry(execution.runs.as_ref())
        .with_computation_settings_snapshot(&execution.data.computation_settings)
        .with_selection_digest(selected.selection_digest)
        .with_event_sink(events)
        .with_result_store(&execution.results)
        .with_atomic_success_transaction(&prepare, &finalize)
        .run(plan.as_ref(), cancellation)
        .map_err(ProjectExecutionError::from)
    }
}

fn snapshot_execution_resources(
    snapshot: &ExecutionSnapshot,
    compile: CompileResourceSnapshot,
) -> Result<ProductionResourceSnapshots, String> {
    use crate::node_system::plan::ResourceId;

    let mut runtime = crate::node_system::runtime::ProjectResourceSnapshot::new(
        snapshot.session_id.clone(),
        compile.versions.clone(),
    )
    .with_plot_sink(Arc::new(ProductionPlotSink));
    for (id, variable) in &snapshot.data.variables {
        runtime = runtime.with_variable_revision(
            ResourceId::new(format!("variables/{id}")).map_err(|error| error.to_string())?,
            Arc::new(variable.clone()),
            snapshot
                .variable_revisions
                .get(id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
        );
    }
    for (id, instance) in &snapshot.database_instances {
        if !snapshot.data.databases.contains_key(id) {
            continue;
        }
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        match &instance.state {
            DatabaseState::Loaded { dataframe, .. } => {
                runtime = runtime.with_database(resource, Arc::clone(dataframe));
            }
            DatabaseState::DuckDb { .. } => {
                let crate::database::DatabaseEngine::DuckDb { path, table } = &instance.decl.engine
                else {
                    return Err(format!("database '{id}' runtime/declaration mismatch"));
                };
                let root = snapshot
                    .project_root
                    .as_ref()
                    .ok_or_else(|| format!("database '{id}' requires an active project path"))?;
                runtime = runtime.with_duckdb_database(
                    resource,
                    root.as_path().join(path).to_string_lossy().into_owned(),
                    table.clone(),
                );
            }
            DatabaseState::Failed { .. } => {}
        }
    }
    Ok(ProductionResourceSnapshots { compile, runtime })
}
