//! Resume the same graph run after publishing a required data-dependent schema boundary.
use super::*;
use yss_graph_analysis::GraphAnalysis;
use yss_graph_execution::state::{ActiveExecutionRun, ExecutionRuntimeState};
use yss_project::execution_authority::PreparedProjectExecution;

pub(super) struct PreparedGraphRun {
    pub context: GraphResolutionContext,
    pub analysis: GraphAnalysis,
    pub scope: GraphExecutionScope,
    pub demand: PlanExecutionDemand,
    pub project: PreparedProjectExecution,
}

pub(super) fn select_stage(
    graph: &GraphResourcePath,
    analysis: &GraphAnalysis,
    scope: &GraphExecutionScope,
    demand: &PlanExecutionDemand,
    execution: &ExecutionRuntimeState,
    active: Option<&ActiveExecutionRun<'_>>,
) -> Result<(GraphExecutionScope, PlanExecutionDemand, bool), ExecutionApplicationError> {
    let current_only = matches!(
        demand,
        PlanExecutionDemand::Node {
            mode: yss_graph_execution::plan::NodeExecutionMode::CurrentInputs,
            ..
        }
    );
    let reuse = matches!(
        demand,
        PlanExecutionDemand::Node { .. }
            | PlanExecutionDemand::Outputs {
                reuse_inputs: true,
                ..
            }
    );
    if !current_only
        && let Some((scope, mut demand)) = scope
            .schema_frontier(graph, analysis.semantic_snapshot(), |output| {
                active.is_some_and(|run| run.completed_output(output))
                    || (reuse && execution.query_pin_result(output).is_some())
            })
            .map_err(ExecutionApplicationError::GraphPlan)?
    {
        if let PlanExecutionDemand::Outputs { reuse_inputs, .. } = &mut demand {
            *reuse_inputs = reuse;
        }
        return Ok((scope, demand, false));
    }
    if !analysis.semantic_snapshot().nodes_ready(scope.nodes()) {
        return Err(ExecutionApplicationError::GraphNotReady);
    }
    Ok((scope.clone(), demand.clone(), true))
}

pub(super) fn execute_stages<D>(
    state: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: &RunGraphRequest,
    prepared: PreparedGraphRun,
    deliver: &mut D,
) -> Result<RunGraphReceipt, ExecutionApplicationError>
where
    D: FnMut(RunApplicationEvent) -> bool + Send,
{
    let PreparedGraphRun {
        context,
        mut analysis,
        scope,
        demand,
        project,
    } = prepared;
    let control =
        RunExecutionControl::with_cancellation(Arc::clone(&request.cancellation), request.deadline);
    // Plan/resource admission precedes public run identity, as for a single-stage run.
    context.revalidate(captured)?;
    revalidate_final_session(state, captured)?;
    check_control(request)?;
    let mut active = captured
        .execution()
        .start_run(&control)
        .map_err(ExecutionApplicationError::PreparedExecution)?;
    let identity = RunIdentity::new(
        captured.execution().session_id(),
        request.graph_path.clone(),
        active.run_id(),
        request.semantic_input_hash,
    );
    let mut started = false;
    let mut announced = std::collections::BTreeSet::new();
    let mut results = BTreeMap::new();
    let outcome = (|| {
        loop {
            check_control(request)?;
            context.revalidate(captured)?;
            revalidate_final_session(state, captured)?;
            let (stage_scope, stage_demand, last) = select_stage(
                &request.graph_path,
                &analysis,
                &scope,
                &demand,
                captured.execution(),
                Some(&active),
            )?;
            let result_basis = captured
                .execution()
                .capture_result_run_basis(
                    request.graph_path.as_str(),
                    crate::graph::inputs::graph_result_inputs(
                        &request.graph_path,
                        &analysis,
                        &context.database,
                        context.registry_fingerprint,
                    ),
                )
                .ok_or(ExecutionApplicationError::DraftChanged)?;
            let package = captured
                .execution()
                .prepare_graph_package(
                    &request.graph_path,
                    &analysis,
                    plan_basis(captured, project.resources().grants())?,
                    &stage_scope,
                )
                .and_then(|package| {
                    package.with_functions(
                        &analysis,
                        captured.graph().registry(),
                        &context.graph_catalog,
                    )
                })
                .map_err(ExecutionApplicationError::GraphPlan)?;
            let plan = captured
                .execution()
                .prepare_package(package, captured.runtime_generation())
                .map_err(ExecutionApplicationError::PackagePreparation)?;
            let bindings = map_project_resource_facts(captured, project.resources().grants())
                .map_err(ExecutionApplicationError::ResourceBindings)?;
            let handoff = active
                .execute_stage(
                    &plan,
                    bindings,
                    captured.resource_provider_factory(),
                    yss_graph_execution::state::ExecutionResultRequest {
                        demand: &stage_demand,
                        basis: Some(&result_basis),
                    },
                    |event| match event {
                        PreparedExecutionEvent::RunStarted { outputs, .. } => {
                            started = true;
                            announced.extend(outputs);
                            let _ = deliver(RunApplicationEvent::new(
                                captured.execution(),
                                identity.clone(),
                                RunApplicationEventKind::RunStarted {
                                    outputs: announced.iter().cloned().collect(),
                                },
                            ));
                        }
                    },
                )
                .map_err(ExecutionApplicationError::PreparedExecution)?;
            let effects = captured
                .project()
                .prepare_execution_effects(
                    project.authority(),
                    CandidateProjectEffects::new(project.resources().grants().iter().cloned()),
                )
                .map_err(ExecutionApplicationError::ProjectEffectPreparation)?;
            let _committed = captured
                .project()
                .finalize_execution_effects(
                    effects,
                    &ProjectEffectCommitControl::new(
                        Arc::clone(&request.cancellation),
                        request.deadline,
                    ),
                )
                .map_err(ExecutionApplicationError::ProjectEffectFinalization)?;
            revalidate_final_session(state, captured)?;
            let outcome = finalize_successful_run(handoff)
                .map_err(ExecutionApplicationError::Finalization)?;
            if !captured
                .execution()
                .publish_committed_results(outcome.handoff())
            {
                return Err(ExecutionApplicationError::Cancelled);
            }
            active
                .record_published_stage(outcome.handoff())
                .map_err(ExecutionApplicationError::PreparedExecution)?;
            for result in outcome
                .handoff()
                .results()
                .iter()
                .filter(|result| result.value().is_evaluated())
            {
                results.insert(
                    result.output().clone(),
                    RunResultReference {
                        result_id: result.result_id(),
                        output: result.output().clone(),
                        category: result.category(),
                    },
                );
            }
            for inspection in outcome.inspection_requests() {
                let _ = deliver(RunApplicationEvent::new(
                    captured.execution(),
                    identity.clone(),
                    RunApplicationEventKind::ResultInspectionRequested {
                        result_id: inspection.result_id(),
                        source: inspection.requester().clone(),
                    },
                ));
            }
            if last {
                break;
            }
            analysis = context.resolve(captured, &request.graph_path, &request.document, "en-US");
        }
        active
            .begin_finalization()
            .map_err(ExecutionApplicationError::RunFinalization)?;
        captured
            .execution()
            .finalize_run_success(active.run_id())
            .map_err(ExecutionApplicationError::RunFinalization)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        if started {
            publish_run_failure(
                captured.execution(),
                active.run_id(),
                &identity,
                deliver,
                terminal_kind(&error),
            );
        }
        return Err(error);
    }
    let _ = deliver(RunApplicationEvent::new(
        captured.execution(),
        identity.clone(),
        RunApplicationEventKind::RunCompleted,
    ));
    Ok(RunGraphReceipt {
        identity,
        results: results.into_values().collect(),
    })
}

fn terminal_kind(error: &ExecutionApplicationError) -> RunApplicationEventKind {
    use yss_graph_execution::error::{RunFailure, RunFailureCode};
    match error {
        ExecutionApplicationError::Cancelled
        | ExecutionApplicationError::DraftChanged
        | ExecutionApplicationError::StaleSession(_)
        | ExecutionApplicationError::PreparedExecution(ExecutePreparedError::Cancelled {
            ..
        }) => RunApplicationEventKind::RunCancelled,
        ExecutionApplicationError::PreparedExecution(error) => {
            RunApplicationEventKind::RunErrored {
                failure: error.failure(),
            }
        }
        ExecutionApplicationError::ProjectEffectPreparation(error)
        | ExecutionApplicationError::ProjectEffectFinalization(error) => {
            terminal_kind_for_effect_error(error)
        }
        ExecutionApplicationError::DeadlineExceeded => RunApplicationEventKind::RunErrored {
            failure: RunFailure {
                code: RunFailureCode::DeadlineExceeded,
                phase: RunPhase::Admission,
                source: None,
                groups: Box::new([]),
            },
        },
        ExecutionApplicationError::GraphNotReady
        | ExecutionApplicationError::GraphPlan(_)
        | ExecutionApplicationError::GraphResolutionFailed { .. } => {
            RunApplicationEventKind::RunErrored {
                failure: RunFailure {
                    code: RunFailureCode::InvalidParameter,
                    phase: RunPhase::PlanValidation,
                    source: None,
                    groups: Box::new([]),
                },
            }
        }
        _ => RunApplicationEventKind::RunErrored {
            failure: finalization_failure(),
        },
    }
}
