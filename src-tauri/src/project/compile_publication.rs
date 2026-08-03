use super::project_state::{ProjectionSourceSnapshot, compile_resources_from_projection_snapshot};
use super::{GraphResourcePath, ProjectState};
use crate::node_system::analysis::{CompilationBasis, CompileProjection, TraceSink};
use crate::node_system::compiler::{
    CompilationTask, CompileProducts, GraphCompiler, ProjectCompileCoordinator,
    PublishedCompileAnalysis, PublishedExecutionPlan, ScheduleOutcome, compilation_basis,
};
use crate::node_system::document::GraphRevision;
use crate::node_system::plan::ExecutionPlan;
use std::sync::Arc;

type PublishedProducts = (
    CompileProjection<PublishedCompileAnalysis>,
    Option<CompileProjection<ExecutionPlan>>,
);
type PublishedBasisProducts = (
    CompileProjection<PublishedCompileAnalysis>,
    Option<CompileProjection<Arc<PublishedExecutionPlan>>>,
);

pub(super) struct ExecutionAuthorityToken {
    pub(super) project_instance_id: String,
    pub(super) authority_generation: u64,
    pub(super) project_session_id: crate::node_system::analysis::ProjectSessionId,
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
    authority_generation: u64,
    project_session_id: crate::node_system::analysis::ProjectSessionId,
}

struct CurrentBasis {
    basis: CompilationBasis<GraphRevision>,
    project_instance_id: String,
    authority_generation: u64,
    project_session_id: crate::node_system::analysis::ProjectSessionId,
}

impl CurrentCompilation {
    fn into_products(self) -> PublishedProducts {
        let plan = self.plan.map(|projection| CompileProjection {
            graph_path: projection.graph_path,
            basis: projection.basis,
            compile_id: projection.compile_id,
            payload: projection.payload.full_plan().clone(),
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
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
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
        input: CompileInput,
        trace_sink: &dyn TraceSink,
    ) -> Result<CurrentCompilation, String> {
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        loop {
            if let Some(products) = self.current_products_at_authority_gate(&coordinator, &input)? {
                return Ok(products);
            }
            match coordinator.request(input.document_path.clone(), input.basis.clone()) {
                ScheduleOutcome::Start(task) => {
                    self.compile_and_publish(&coordinator, &task, &input, trace_sink);
                    self.finish_and_drive_pending(
                        &coordinator,
                        task,
                        &input.project_session_id,
                        trace_sink,
                    );
                }
                ScheduleOutcome::Coalesced { compile_id } => {
                    self.run_compile_coalesced_before_wait_test_hook();
                    if coordinator
                        .wait_for_current(&input.document_path, &input.basis, compile_id)
                        .is_some()
                    {
                        if let Some(products) =
                            self.current_products_at_authority_gate(&coordinator, &input)?
                        {
                            return Ok(products);
                        }
                    }
                }
            }
        }
    }

    fn current_products_at_authority_gate(
        &self,
        coordinator: &Arc<ProjectCompileCoordinator>,
        input: &CompileInput,
    ) -> Result<Option<CurrentCompilation>, String> {
        let product_hint = current_products(coordinator, input).is_some();
        if product_hint {
            self.run_compile_before_authority_gate_test_hook();
        }
        let publication = self.mutation_publication.lock().unwrap();
        if !self.authority_matches(&publication, coordinator, input) {
            return Err(
                "stale_project_lifecycle: compile input changed before product return".into(),
            );
        }
        Ok(
            current_products(coordinator, input).map(|(analysis, plan)| CurrentCompilation {
                analysis,
                plan,
                source: input.source.clone(),
                authority: ExecutionAuthorityToken {
                    project_instance_id: input.project_instance_id.clone(),
                    authority_generation: input.authority_generation,
                    project_session_id: input.project_session_id.clone(),
                    basis: input.basis.clone(),
                    coordinator: Arc::clone(coordinator),
                },
            }),
        )
    }

    pub(super) fn execution_authority_matches(
        &self,
        publication: &super::project_state::MutationPublication,
        authority: &ExecutionAuthorityToken,
    ) -> bool {
        if publication.project_instance_id != authority.project_instance_id
            || publication.authority_generation() != authority.authority_generation
        {
            return false;
        }
        let identity = self.current_projection_environment_expectation();
        identity.project_instance_id.as_str() == authority.project_instance_id
            && identity.project_session_id == authority.project_session_id
            && Arc::ptr_eq(
                &authority.coordinator,
                &self.compile_coordinator.read().unwrap().clone(),
            )
    }

    fn authority_matches(
        &self,
        publication: &super::project_state::MutationPublication,
        coordinator: &Arc<ProjectCompileCoordinator>,
        input: &CompileInput,
    ) -> bool {
        if publication.project_instance_id != input.project_instance_id
            || publication.authority_generation() != input.authority_generation
        {
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
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
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
            self.compile_and_publish(coordinator, &next, &input, trace_sink);
            finished = next;
        }
    }

    fn compile_and_publish(
        &self,
        coordinator: &Arc<ProjectCompileCoordinator>,
        task: &CompilationTask,
        input: &CompileInput,
        trace_sink: &dyn TraceSink,
    ) {
        let Ok(resources) = compile_resources_from_projection_snapshot(&input.source) else {
            task.cancellation.cancel();
            return;
        };
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
            return;
        };
        let has_blocking_diagnostics = result.analysis.has_blocking_errors();
        let plan = result
            .plan
            .zip(result.execution_basis)
            .map(|(plan, execution_basis)| {
                Arc::new(PublishedExecutionPlan::new(plan, execution_basis))
            });
        let products = CompileProducts {
            analysis: PublishedCompileAnalysis {
                analysis: result.analysis,
                semantic: result.semantic,
            },
            has_blocking_diagnostics,
            plan,
        };

        let Ok(current) = self.capture_current_basis(
            &GraphResourcePath::new(input.document_path.0.as_ref())
                .expect("captured graph path is normalized"),
        ) else {
            task.cancellation.cancel();
            return;
        };
        if current.basis != input.basis
            || current.project_instance_id != input.project_instance_id
            || current.authority_generation != input.authority_generation
            || current.project_session_id != input.project_session_id
        {
            task.cancellation.cancel();
            return;
        }
        self.run_compile_before_authority_gate_test_hook();
        let publication = self.mutation_publication.lock().unwrap();
        if !self.authority_matches(&publication, coordinator, input) {
            task.cancellation.cancel();
            return;
        }
        coordinator.publish(task, &current.basis, products);
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

    fn capture_current_basis(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<CurrentBasis, String> {
        let input = self.capture_compile_input(graph_path)?;
        Ok(CurrentBasis {
            basis: input.basis,
            project_instance_id: input.project_instance_id,
            authority_generation: input.authority_generation,
            project_session_id: input.project_session_id,
        })
    }

    #[cfg(test)]
    pub(super) fn published_variant_cache_state_for_test(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Option<(crate::node_system::analysis::CompileId, usize)> {
        let input = self.capture_compile_input(graph_path).ok()?;
        let coordinator = self.compile_coordinator.read().unwrap().clone();
        let (_, plan) = coordinator.get_current(&input.document_path, &input.basis)?;
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
        let (analysis, plan) = coordinator.get_current(&input.document_path, &input.basis)?;
        Some((analysis.compile_id, plan.map(|plan| plan.compile_id)))
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
        let resources = compile_resources_from_projection_snapshot(&source)?;
        let basis = compilation_basis(
            document.revision,
            source.environment.registry.fingerprint().clone(),
            resources.versions,
        );
        let project_instance_id = source.project_instance_id.clone();
        let authority_generation = source.authority_generation;
        let project_session_id = source.environment.project_session_id.clone();
        Ok(Self {
            source,
            document_path: crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ),
            document,
            basis,
            project_instance_id,
            authority_generation,
            project_session_id,
        })
    }
}

fn current_products(
    coordinator: &ProjectCompileCoordinator,
    input: &CompileInput,
) -> Option<PublishedBasisProducts> {
    let products = coordinator.get_current(&input.document_path, &input.basis)?;
    if products.1.as_ref().is_some_and(|plan| {
        plan.payload.full_plan().provenance.project_session_id
            != input.source.environment.project_session_id
    }) {
        return None;
    }
    Some(products)
}
