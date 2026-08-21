use super::*;

pub(super) fn operation_correlation(
    plan: &ExecutionPlan,
    run_id: RunId,
    parent_call: Option<ParentCallId>,
    operation: OperationIndex,
) -> CorrelationContext {
    let planned = &plan.operations[operation.index()];
    CorrelationContext::compile(&plan.provenance)
        .for_run(run_id, parent_call)
        .for_node(planned.source_node_id, planned.source_node_type_id.clone())
}

pub(super) fn execute_operation_worker(
    context: &OperationWorkerContext<'_>,
    job: PreparedOperation,
    trace: &dyn TraceSink,
) -> Result<Box<[StoredValue]>, RunError> {
    let operation = job.operation;
    let activation = job.activation;
    let attempt = job.attempt;
    let correlation =
        operation_correlation(context.plan, context.run_id, context.parent_call, operation);
    let planned = &context.plan.operations[operation.index()];
    let mut span = start_span_safely(
        trace,
        SpanSpec {
            parent_span_id: context.run_parent_span_id,
            run_id: Some(context.run_id),
            operation_id: Some(planned.stable_id.clone()),
            activation_id: Some(activation),
            attempt_id: Some(attempt),
            kind: SpanKind::OperationAttempt,
            correlation,
        },
    );
    let operation_span_id = span.span_id();
    let result = execute_operation_worker_inner(context, job, trace, operation_span_id);
    span.finish(operation_span_outcome(
        context.plan,
        operation,
        attempt,
        &result,
    ));
    result
}

fn execute_operation_worker_inner(
    context: &OperationWorkerContext<'_>,
    job: PreparedOperation,
    trace: &dyn TraceSink,
    operation_span_id: Option<SpanId>,
) -> Result<Box<[StoredValue]>, RunError> {
    check_terminal(context.cancellation, context.deadline, RunPhase::Kernel)?;
    let operation = &context.plan.operations[job.operation.index()];
    let inputs = if operation.source_node_type_id.as_str() == "yssbi.debug.view" {
        for result_id in &job.input_result_ids {
            let result = context
                .results
                .wait_terminal(*result_id, context.cancellation, context.deadline)
                .map_err(result_store_error)?;
            match &result.state {
                ResultState::Ready(_) => {}
                ResultState::Failed(failure) => {
                    return Err(RunError::UpstreamResultFailed {
                        source_result_id: *result_id,
                        message: failure.message.clone(),
                    });
                }
                ResultState::Cancelled => {
                    return Err(RunError::UpstreamResultCancelled {
                        source_result_id: *result_id,
                    });
                }
                ResultState::Pending(_) => unreachable!("wait_terminal returned pending result"),
            }
        }
        Box::new([])
    } else {
        job.input_result_ids
            .iter()
            .zip(&operation.inputs)
            .map(|(result_id, input)| {
                let result = context
                    .results
                    .wait_terminal(*result_id, context.cancellation, context.deadline)
                    .map_err(result_store_error)?;
                let value = match &result.state {
                    ResultState::Ready(value) => value.to_runtime_value(),
                    ResultState::Failed(failure) => {
                        return Err(RunError::UpstreamResultFailed {
                            source_result_id: *result_id,
                            message: failure.message.clone(),
                        });
                    }
                    ResultState::Cancelled => {
                        return Err(RunError::UpstreamResultCancelled {
                            source_result_id: *result_id,
                        });
                    }
                    ResultState::Pending(_) => {
                        unreachable!("wait_terminal returned pending result")
                    }
                };
                if input.contract.kind == PlannedValueKind::DataSeries {
                    let RuntimeValue::Artifact(artifact) = &value else {
                        return Err(RunError::InvalidPlan(
                            format!(
                                "DataSeries input value {} did not receive a DataSeries Artifact",
                                input.value.index()
                            )
                            .into(),
                        ));
                    };
                    let metadata = artifact.data_series_metadata().ok_or_else(|| {
                        RunError::InvalidPlan(
                            format!(
                                "DataSeries input value {} did not receive a DataSeries Artifact",
                                input.value.index()
                            )
                            .into(),
                        )
                    })?;
                    validate_data_series_type_expr(metadata, &input.contract.type_expr)
                        .map_err(|error| RunError::InvalidPlan(error.to_string().into()))?;
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, RunError>>()?
            .into_boxed_slice()
    };
    let outputs = if operation.source_node_type_id.as_str() == "yssbi.debug.view" {
        debug_assert_eq!(job.input_result_ids.len(), 1);
        Vec::new()
    } else {
        match &operation.kernel {
            PlannedKernel::Native(handle) => {
                let kernel = context
                    .kernels
                    .get(handle)
                    .ok_or_else(|| RunError::KernelNotFound(handle.as_str().into()))?;
                let kernel_context = KernelContext {
                    run_id: context.run_id,
                    frame_id: context.frame_id,
                    activation_id: job.activation,
                    source_graph_path: &context.plan.provenance.graph_path,
                    source_node_id: operation.source_node_id,
                    run_output: context.run_output,
                    computation_settings: context.computation_settings,
                    params: &operation.params,
                    compiled_parameters: context.compiled_parameters,
                    resources: context.resources,
                    resource_owner: context.resource_owner,
                    cancellation: context.cancellation,
                    deadline: context.deadline,
                };
                match kernel.execute(&kernel_context, &inputs) {
                    Ok(outputs) => outputs,
                    Err(error) if error.kind() == KernelErrorKind::Cancelled => {
                        return Err(RunError::Cancelled);
                    }
                    Err(error) if error.kind() == KernelErrorKind::DeadlineExceeded => {
                        return Err(RunError::DeadlineExceeded {
                            phase: RunPhase::Kernel,
                        });
                    }
                    Err(error) => {
                        return Err(RunError::KernelFailed {
                            operation: job.operation,
                            kind: error.kind(),
                            message: error.message().into(),
                        });
                    }
                }
            }
            PlannedKernel::Adapter(adapter) => {
                let input = inputs.into_vec().into_iter().next().ok_or_else(|| {
                    RunError::InvalidPlan("adapter operation has no input".into())
                })?;
                let correlation = operation_correlation(
                    context.plan,
                    context.run_id,
                    context.parent_call,
                    job.operation,
                );
                let mut adapter_span = start_span_safely(
                    trace,
                    SpanSpec {
                        parent_span_id: operation_span_id,
                        run_id: Some(context.run_id),
                        operation_id: Some(operation.stable_id.clone()),
                        activation_id: Some(job.activation),
                        attempt_id: Some(job.attempt),
                        kind: SpanKind::AdapterIo,
                        correlation,
                    },
                );
                let result = execute_planned_adapter(
                    adapter,
                    input,
                    context.resource_owner,
                    context.cancellation,
                );
                adapter_span.finish(span_outcome(&result));
                vec![result?]
            }
            PlannedKernel::Relational(index) => {
                let subplan = &context.plan.relational_subplans[index.index()];
                let backend = context
                    .relational_backends
                    .get(&subplan.backend)
                    .ok_or_else(|| RunError::RelationalBackendNotFound(subplan.backend.clone()))?;
                let relational_context = RelationalContext {
                    run_id: context.run_id,
                    resources: context.resources,
                    resource_owner: context.resource_owner,
                    cancellation: context.cancellation,
                    deadline: context.deadline,
                };
                let correlation = operation_correlation(
                    context.plan,
                    context.run_id,
                    context.parent_call,
                    job.operation,
                );
                let mut adapter_span = start_span_safely(
                    trace,
                    SpanSpec {
                        parent_span_id: operation_span_id,
                        run_id: Some(context.run_id),
                        operation_id: Some(operation.stable_id.clone()),
                        activation_id: Some(job.activation),
                        attempt_id: Some(job.attempt),
                        kind: SpanKind::AdapterIo,
                        correlation,
                    },
                );
                let backend_result =
                    backend.execute(&relational_context, &subplan.compiled_plan, &inputs);
                let backend_outcome = match &backend_result {
                    Err(error)
                        if error.code()
                            == crate::node_system::runtime::RelationalErrorCode::Cancelled =>
                    {
                        SpanOutcome::Cancellation
                    }
                    Err(_) => SpanOutcome::Error,
                    Ok(_) if context.cancellation.is_cancelled() => SpanOutcome::Cancellation,
                    Ok(_) => SpanOutcome::Success,
                };
                adapter_span.finish(backend_outcome);
                match backend_result {
                    Ok(execution) => execution.outputs,
                    Err(error) => return Err(RunError::from_relational(job.operation, error)),
                }
            }
        }
    };
    check_terminal(context.cancellation, context.deadline, RunPhase::Kernel)?;
    if outputs.len() != operation.outputs.len() {
        return Err(RunError::OutputCount {
            operation: job.operation,
            expected: operation.outputs.len(),
            actual: outputs.len(),
        });
    }
    outputs
        .into_iter()
        .map(|value| StoredValue::prepare(value, context.resource_owner))
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}
