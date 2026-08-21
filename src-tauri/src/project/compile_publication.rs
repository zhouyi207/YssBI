use super::project_state::{ProjectionSourceSnapshot, compile_resources_from_projection_snapshot};
use super::{GraphResourcePath, ProjectState};
#[cfg(test)]
use crate::node_system::analysis::ResourceKey;
use crate::node_system::analysis::{
    CompilationBasis, CompileProjection, ResourceObservationSet, ResourceObservedState,
    ResourceVersion, TraceSink,
};
use crate::node_system::compiler::{
    CompilationOutcome, CompilationTask, CompileProducts, GraphCompiler, ProjectCompileCoordinator,
    PublishOutcome, PublishedCompileAnalysis, PublishedExecutionPlan, ScheduleOutcome,
    compilation_basis,
};
use crate::node_system::document::GraphRevision;
use crate::node_system::plan::ExecutionPlan;
use std::sync::Arc;

fn publication_blocks_plan(
    outcome: &CompilationOutcome,
    analysis_has_blocking_diagnostics: bool,
) -> Result<bool, String> {
    match outcome {
        CompilationOutcome::Succeeded if analysis_has_blocking_diagnostics => Err(
            "internal_compilation_state: successful compilation has blocking diagnostics".into(),
        ),
        CompilationOutcome::Succeeded => Ok(false),
        CompilationOutcome::AnalysisBlocked | CompilationOutcome::InternalFailure(_) => Ok(true),
    }
}

type PublishedProducts = (
    CompileProjection<PublishedCompileAnalysis>,
    Option<CompileProjection<ExecutionPlan>>,
);
pub(super) struct ExecutionAuthorityToken {
    pub(super) project_instance_id: String,
    pub(super) project_session_id: crate::node_system::ProjectSessionId,
    pub(super) graph_path: crate::node_system::document::GraphResourcePath,
    pub(super) basis: CompilationBasis<GraphRevision>,
    coordinator: Arc<ProjectCompileCoordinator>,
}

pub(super) struct CurrentCompilation {
    pub(super) analysis: CompileProjection<PublishedCompileAnalysis>,
    pub(super) plan: Option<CompileProjection<Arc<PublishedExecutionPlan>>>,
    pub(super) source: ProjectionSourceSnapshot,
    pub(super) authority: ExecutionAuthorityToken,
}

struct CompileInput {
    source: ProjectionSourceSnapshot,
    document_path: crate::node_system::document::GraphResourcePath,
    document: crate::node_system::document::GraphDocument,
    basis: CompilationBasis<GraphRevision>,
    project_instance_id: String,
    project_session_id: crate::node_system::ProjectSessionId,
}

struct CurrentBasis {
    basis: CompilationBasis<GraphRevision>,
    resource_states: ResourceObservationSet,
    project_instance_id: String,
    project_session_id: crate::node_system::ProjectSessionId,
}

impl CurrentCompilation {
    fn into_products(self) -> PublishedProducts {
        let plan = self.plan.and_then(|projection| {
            projection
                .payload
                .full_plan()
                .cloned()
                .map(|payload| CompileProjection {
                    graph_path: projection.graph_path,
                    basis: projection.basis,
                    compile_id: projection.compile_id,
                    payload,
                })
        });
        (self.analysis, plan)
    }
}

impl ProjectState {
    pub(super) fn get_or_compile_current_from_source(
        &self,
        graph_path: &GraphResourcePath,
        source: &ProjectionSourceSnapshot,
    ) -> Result<PublishedProducts, String> {
        let input = CompileInput::from_source(graph_path, source.clone())?;
        let trace_sink = Arc::clone(&input.source.environment.trace_sink);
        self.get_or_compile_input(input, trace_sink.as_ref())
            .map(CurrentCompilation::into_products)
    }

    pub(super) fn get_or_compile_current(
        &self,
        graph_path: &GraphResourcePath,
        expected_session_id: &crate::node_system::ProjectSessionId,
        trace_sink: &dyn TraceSink,
    ) -> Result<CurrentCompilation, String> {
        let input = self.capture_compile_input(graph_path)?;
        if &input.project_session_id != expected_session_id {
            return Err(
                "stale_project_lifecycle: project changed before execution compilation".into(),
            );
        }
        self.get_or_compile_input(input, trace_sink)
    }

    fn get_or_compile_input(
        &self,
        mut input: CompileInput,
        trace_sink: &dyn TraceSink,
    ) -> Result<CurrentCompilation, String> {
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        loop {
            if let Some(products) = self.current_products_at_authority_gate(&coordinator, &input)? {
                return Ok(products);
            }
            if coordinator
                .get_candidate(&input.document_path, &input.basis)
                .is_some()
            {
                let graph_path = GraphResourcePath::new(input.document_path.0.as_ref())
                    .expect("captured graph path is normalized");
                let refreshed = self.capture_compile_input(&graph_path)?;
                if refreshed.project_session_id != input.project_session_id {
                    return Err(
                        "stale_project_lifecycle: project changed while refreshing compile input"
                            .into(),
                    );
                }
                if refreshed.source.authority_generation != input.source.authority_generation {
                    input = refreshed;
                    continue;
                }
            }
            match coordinator.request(input.document_path.clone(), input.basis.clone()) {
                ScheduleOutcome::Start(task) => {
                    let outcome =
                        self.compile_and_publish(&coordinator, &task, &input, trace_sink)?;
                    self.finish_and_drive_pending(
                        &coordinator,
                        task,
                        &input.project_session_id,
                        trace_sink,
                    );
                    if outcome != PublishOutcome::Current {
                        let graph_path = GraphResourcePath::new(input.document_path.0.as_ref())
                            .expect("captured graph path is normalized");
                        let refreshed = self.capture_compile_input(&graph_path)?;
                        if refreshed.project_session_id != input.project_session_id {
                            return Err(
                                "stale_project_lifecycle: project changed while refreshing rejected compile input"
                                    .into(),
                            );
                        }
                        input = refreshed;
                    }
                }
                ScheduleOutcome::Coalesced { compile_id } => {
                    self.run_compile_coalesced_before_wait_test_hook();
                    if coordinator
                        .wait_for_candidate(&input.document_path, &input.basis, compile_id)
                        .is_some()
                    {
                        if let Some(products) =
                            self.current_products_at_authority_gate(&coordinator, &input)?
                        {
                            return Ok(products);
                        }
                    }
                }
                ScheduleOutcome::Exhausted => {
                    return Err("compiler identity space is exhausted".into());
                }
            }
        }
    }

    fn current_products_at_authority_gate(
        &self,
        coordinator: &Arc<ProjectCompileCoordinator>,
        input: &CompileInput,
    ) -> Result<Option<CurrentCompilation>, String> {
        if coordinator
            .get_candidate(&input.document_path, &input.basis)
            .is_none()
        {
            return Ok(None);
        }
        self.run_compile_before_authority_gate_test_hook();
        let publication = self.mutation_publication.lock().unwrap();
        if !self.authority_matches(&publication, coordinator, input) {
            return Err(
                "stale_project_lifecycle: compile input changed before product return".into(),
            );
        }
        let Some(candidate) = coordinator.get_candidate(&input.document_path, &input.basis) else {
            return Ok(None);
        };
        let current = self.capture_current_basis_at_gate(input, &candidate.0.basis)?;
        let products = coordinator.get_current_with_observations(
            &input.document_path,
            &current.basis,
            &current.resource_states,
        );
        Ok(products
            .filter(|products| products.0.compile_id == candidate.0.compile_id)
            .map(|(analysis, plan)| {
                let basis = analysis.basis.clone();
                CurrentCompilation {
                    analysis,
                    plan,
                    source: input.source.clone(),
                    authority: ExecutionAuthorityToken {
                        project_instance_id: input.project_instance_id.clone(),
                        project_session_id: input.project_session_id.clone(),
                        graph_path: input.document_path.clone(),
                        basis,
                        coordinator: Arc::clone(coordinator),
                    },
                }
            }))
    }

    pub(super) fn execution_authority_matches(
        &self,
        publication: &super::project_state::MutationPublication,
        authority: &ExecutionAuthorityToken,
    ) -> bool {
        if publication.project_instance_id != authority.project_instance_id {
            return false;
        }
        let identity = self.current_projection_environment_expectation();
        identity.project_instance_id.as_str() == authority.project_instance_id
            && identity.project_session_id == authority.project_session_id
            && Arc::ptr_eq(
                &authority.coordinator,
                &self.compile_coordinator.read().unwrap().clone(),
            )
            && self
                .capture_current_basis_at_gate_for(
                    &authority.graph_path,
                    &authority.basis,
                    &authority.project_instance_id,
                    &authority.project_session_id,
                )
                .is_ok_and(|current| {
                    current.basis.graph_revision == authority.basis.graph_revision
                        && authority
                            .coordinator
                            .get_current_with_observations(
                                &authority.graph_path,
                                &current.basis,
                                &current.resource_states,
                            )
                            .is_some_and(|products| products.0.basis == authority.basis)
                })
    }

    fn authority_matches(
        &self,
        publication: &super::project_state::MutationPublication,
        coordinator: &Arc<ProjectCompileCoordinator>,
        input: &CompileInput,
    ) -> bool {
        if publication.project_instance_id != input.project_instance_id {
            return false;
        }
        let identity = self.current_projection_environment_expectation();
        identity.project_instance_id.as_str() == input.project_instance_id
            && identity.project_session_id == input.project_session_id
            && Arc::ptr_eq(
                coordinator,
                &self.compile_coordinator.read().unwrap().clone(),
            )
    }

    fn finish_and_drive_pending(
        &self,
        coordinator: &Arc<ProjectCompileCoordinator>,
        mut finished: CompilationTask,
        expected_session_id: &crate::node_system::ProjectSessionId,
        trace_sink: &dyn TraceSink,
    ) {
        while let Some(next) = coordinator.finish(&finished.graph_path, finished.compile_id) {
            let graph_path = match GraphResourcePath::new(next.graph_path.0.as_ref()) {
                Ok(path) => path,
                Err(_) => {
                    next.cancellation.cancel();
                    finished = next;
                    continue;
                }
            };
            let Ok(input) = self.capture_compile_input(&graph_path) else {
                next.cancellation.cancel();
                finished = next;
                continue;
            };
            if input.basis != next.basis || &input.project_session_id != expected_session_id {
                next.cancellation.cancel();
                finished = next;
                continue;
            }
            let _ = self.compile_and_publish(coordinator, &next, &input, trace_sink);
            finished = next;
        }
    }

    fn compile_and_publish(
        &self,
        coordinator: &Arc<ProjectCompileCoordinator>,
        task: &CompilationTask,
        input: &CompileInput,
        trace_sink: &dyn TraceSink,
    ) -> Result<PublishOutcome, String> {
        self.run_compile_after_source_capture_test_hook();
        {
            let publication = self.mutation_publication.lock().unwrap();
            if !self.authority_matches(&publication, coordinator, input) {
                task.cancellation.cancel();
                return Err(
                    "stale_project_lifecycle: project changed after compile source capture".into(),
                );
            }
        }
        let resources = compile_resources_from_projection_snapshot(&input.source)?;
        let compiler = GraphCompiler::with_resolvers(
            input.source.environment.registry.as_ref(),
            &resources,
            resources.schema_resolvers(),
            crate::node_system::compiler::build_builtin_interface_resolvers(),
        )
        .with_observability(
            input.source.environment.project_session_id.clone(),
            trace_sink,
        );
        let snapshot = compiler.snapshot_with_compile_id(
            task.compile_id,
            input.document_path.clone(),
            &input.document,
        );
        let Ok(result) = compiler.compile_snapshot(&snapshot, &task.cancellation) else {
            return Ok(PublishOutcome::Cancelled);
        };
        let has_blocking_diagnostics =
            publication_blocks_plan(&result.outcome, result.analysis.has_blocking_errors())?;
        let plan = match &result.outcome {
            CompilationOutcome::Succeeded => result.execution_basis.map(|execution_basis| {
                Arc::new(PublishedExecutionPlan::new(result.plan, execution_basis))
            }),
            CompilationOutcome::AnalysisBlocked | CompilationOutcome::InternalFailure(_) => None,
        };
        let products = CompileProducts {
            analysis: PublishedCompileAnalysis {
                analysis: result.analysis,
                interface_projection: result.interface_projection,
                semantic: result.semantic,
                outcome: result.outcome,
            },
            has_blocking_diagnostics,
            plan,
        };

        self.run_compile_before_authority_gate_test_hook();
        let publication = self.mutation_publication.lock().unwrap();
        if !self.authority_matches(&publication, coordinator, input) {
            task.cancellation.cancel();
            return Err(
                "stale_project_lifecycle: project changed before compile publication".into(),
            );
        }
        let final_basis = products.analysis.analysis.basis.clone();
        let current = self
            .capture_current_basis_at_gate(input, &final_basis)
            .map_err(|error| {
                task.cancellation.cancel();
                error
            })?;
        if current.basis != input.basis
            || current.project_instance_id != input.project_instance_id
            || current.project_session_id != input.project_session_id
        {
            task.cancellation.cancel();
            return Ok(PublishOutcome::Stale);
        }
        let report = coordinator.publish_with_observations(
            task,
            &current.basis,
            &current.resource_states,
            &final_basis,
            products,
        );
        Ok(report.analysis)
    }

    pub(super) fn capture_projection_source(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<ProjectionSourceSnapshot, String> {
        const MAX_CAPTURE_ATTEMPTS: usize = 3;
        for _ in 0..MAX_CAPTURE_ATTEMPTS {
            let expectation = self.current_projection_environment_expectation();
            let environment = self.capture_projection_environment(&expectation)?;
            self.run_compile_capture_after_environment_test_hook();
            let source = {
                let publication = self.mutation_publication.lock().unwrap();
                if !environment.matches_publication(&publication) {
                    None
                } else {
                    let data = self.project_data.read().unwrap();
                    if !data.graphs.contains_key(graph_path) {
                        return Err(format!("graph '{}' not loaded", graph_path));
                    }
                    let project_instance_id = environment.authority.project_instance_id.clone();
                    let authority_generation = environment.authority.authority_generation;
                    Some(self.projection_source_snapshot(
                        &data,
                        environment,
                        project_instance_id,
                        authority_generation,
                        self.graph_revisions.read().unwrap().clone(),
                        self.variable_revisions.read().unwrap().clone(),
                        self.database_authority_revisions.read().unwrap().clone(),
                    ))
                }
            };
            if let Some(source) = source {
                return Ok(source);
            }
        }
        Err(
            "stale_project_lifecycle: authority changed repeatedly during projection capture"
                .into(),
        )
    }

    fn capture_compile_input(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<CompileInput, String> {
        CompileInput::from_source(graph_path, self.capture_projection_source(graph_path)?)
    }

    fn capture_current_basis_at_gate(
        &self,
        input: &CompileInput,
        candidate_basis: &CompilationBasis<GraphRevision>,
    ) -> Result<CurrentBasis, String> {
        self.capture_current_basis_at_gate_for(
            &input.document_path,
            candidate_basis,
            &input.project_instance_id,
            &input.project_session_id,
        )
    }

    fn capture_current_basis_at_gate_for(
        &self,
        graph_path: &crate::node_system::document::GraphResourcePath,
        candidate_basis: &CompilationBasis<GraphRevision>,
        project_instance_id: &str,
        project_session_id: &crate::node_system::ProjectSessionId,
    ) -> Result<CurrentBasis, String> {
        let data = self.project_data.read().unwrap();
        let graph_path = GraphResourcePath::new(graph_path.0.as_ref())
            .expect("captured graph path is normalized");
        let document = data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
        let resource_states = self.authoritative_resource_states(&data, candidate_basis);
        Ok(CurrentBasis {
            basis: compilation_basis(
                document.document.revision,
                self.project_store
                    .read()
                    .unwrap()
                    .node_registry
                    .fingerprint()
                    .clone(),
                Default::default(),
            ),
            resource_states,
            project_instance_id: project_instance_id.to_string(),
            project_session_id: project_session_id.clone(),
        })
    }

    fn authoritative_resource_states(
        &self,
        data: &super::ProjectData,
        candidate_basis: &CompilationBasis<GraphRevision>,
    ) -> ResourceObservationSet {
        let graph_revisions = self.graph_revisions.read().unwrap();
        let variable_revisions = self.variable_revisions.read().unwrap();
        let database_revisions = self.database_authority_revisions.read().unwrap();
        candidate_basis
            .resource_versions
            .keys()
            .chain(candidate_basis.resource_observations.keys())
            .map(|key| {
                let state = if key.as_str().starts_with("functions/") {
                    let path = GraphResourcePath::new(key.as_str())
                        .expect("compiled function resource key is normalized");
                    let revision = graph_revisions.get(&path).copied();
                    if data
                        .graphs
                        .get(&path)
                        .is_some_and(|resource| resource.function.is_some())
                    {
                        revision
                            .map(resource_revision_version)
                            .map(ResourceObservedState::Present)
                            .unwrap_or(ResourceObservedState::Absent(None))
                    } else {
                        ResourceObservedState::Absent(revision.map(resource_revision_version))
                    }
                } else if let Some(id) = key.as_str().strip_prefix("variables/") {
                    uuid::Uuid::parse_str(id)
                        .ok()
                        .and_then(|id| {
                            let id = crate::variable::VariableId::from(id);
                            variable_revisions.get(&id).copied().map(|entry| {
                                let version = resource_revision_version(entry.revision);
                                if entry.is_present() && data.variables.contains_key(&id) {
                                    ResourceObservedState::Present(version)
                                } else {
                                    ResourceObservedState::Absent(Some(version))
                                }
                            })
                        })
                        .unwrap_or(ResourceObservedState::Absent(None))
                } else if let Some(id) = key.as_str().strip_prefix("databases/") {
                    database_revisions
                        .get(id)
                        .copied()
                        .map(|revision| {
                            let version = ResourceVersion::new(format!("revision:{revision}"));
                            if data.databases.contains_key(id) {
                                ResourceObservedState::Present(version)
                            } else {
                                ResourceObservedState::Absent(Some(version))
                            }
                        })
                        .unwrap_or(ResourceObservedState::Absent(None))
                } else {
                    ResourceObservedState::Absent(None)
                };
                (key.clone(), state)
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn authoritative_resource_states_for_test(
        &self,
        keys: impl IntoIterator<Item = ResourceKey>,
    ) -> ResourceObservationSet {
        let mut basis = compilation_basis(
            GraphRevision::INITIAL,
            self.project_store
                .read()
                .unwrap()
                .node_registry
                .fingerprint()
                .clone(),
            Default::default(),
        );
        for key in keys {
            basis
                .resource_observations
                .insert(key, ResourceObservedState::Absent(None));
        }
        let _publication = self.mutation_publication.lock().unwrap();
        let data = self.project_data.read().unwrap();
        self.authoritative_resource_states(&data, &basis)
    }

    #[cfg(test)]
    pub(super) fn published_variant_cache_state_for_test(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Option<(crate::node_system::analysis::CompileId, usize)> {
        let input = self.capture_compile_input(graph_path).ok()?;
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        let candidate = coordinator.get_candidate(&input.document_path, &input.basis)?;
        let _publication = self.mutation_publication.lock().unwrap();
        let current = self
            .capture_current_basis_at_gate(&input, &candidate.0.basis)
            .ok()?;
        let (_, plan) = coordinator.get_current_with_observations(
            &input.document_path,
            &current.basis,
            &current.resource_states,
        )?;
        let plan = plan?;
        Some((plan.compile_id, plan.payload.cached_variant_count()))
    }

    #[cfg(test)]
    pub(super) fn published_compile_ids_for_test(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Option<(
        crate::node_system::analysis::CompileId,
        Option<crate::node_system::analysis::CompileId>,
    )> {
        let input = self.capture_compile_input(graph_path).ok()?;
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        let candidate = coordinator.get_candidate(&input.document_path, &input.basis)?;
        let _publication = self.mutation_publication.lock().unwrap();
        let current = self
            .capture_current_basis_at_gate(&input, &candidate.0.basis)
            .ok()?;
        let (analysis, plan) = coordinator.get_current_with_observations(
            &input.document_path,
            &current.basis,
            &current.resource_states,
        )?;
        Some((analysis.compile_id, plan.map(|plan| plan.compile_id)))
    }
}

fn resource_revision_version(
    revision: crate::node_system::document::ResourceRevision,
) -> ResourceVersion {
    ResourceVersion::new(format!("revision:{}", revision.get()))
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    use crate::node_system::compiler::{
        CompilationOutcome, CompilationStage, InternalCompilationFailure,
    };

    #[test]
    fn internal_outcome_blocks_plan_publication_without_diagnostics() {
        let outcome = CompilationOutcome::InternalFailure(InternalCompilationFailure {
            stage: CompilationStage::Lowering,
            code: "compiler.lowering.internal_invariant".into(),
            node_id: None,
        });

        assert!(publication_blocks_plan(&outcome, false).unwrap());
    }
}

impl CompileInput {
    fn from_source(
        graph_path: &GraphResourcePath,
        source: ProjectionSourceSnapshot,
    ) -> Result<Self, String> {
        let document = source
            .data
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.clone())
            .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
        {
            let publication = source.state.mutation_publication.lock().unwrap();
            if publication.project_instance_id != source.project_instance_id {
                return Err("stale_project_lifecycle: compile source project changed".into());
            }
            let identity = source.state.current_projection_environment_expectation();
            if identity.project_instance_id.as_str() != source.project_instance_id
                || identity.project_session_id != source.environment.project_session_id
            {
                return Err("stale_project_lifecycle: compile source session changed".into());
            }
            let current_revision = source
                .state
                .project_data
                .read()
                .unwrap()
                .graphs
                .get(graph_path)
                .map(|resource| resource.document.revision);
            if current_revision != Some(document.revision) {
                return Err("stale_project_lifecycle: compile source graph changed".into());
            }
            let current_registry = source
                .state
                .project_store
                .read()
                .unwrap()
                .node_registry
                .clone();
            if current_registry.fingerprint() != source.environment.registry.fingerprint() {
                return Err("stale_project_lifecycle: compile source registry changed".into());
            }
        }
        let basis = compilation_basis(
            document.revision,
            source.environment.registry.fingerprint().clone(),
            Default::default(),
        );
        let project_instance_id = source.project_instance_id.clone();
        let project_session_id = source.environment.project_session_id.clone();
        Ok(Self {
            source,
            document_path: crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ),
            document,
            basis,
            project_instance_id,
            project_session_id,
        })
    }
}
