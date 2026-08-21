use super::*;

impl<'a> RunExecutor<'a> {
    pub(super) fn finish_operation_completion(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        _memoization: &MemoTables,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        _prepared: &mut BTreeMap<OperationIndex, PreparedOperation>,
        delayed_retries: &mut BinaryHeap<Reverse<DelayedRetry>>,
        delayed_operations: &mut BTreeSet<OperationIndex>,
        next_retry_tie: &mut u64,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        pending: &mut BTreeSet<OperationIndex>,
        terminal_error: &mut Option<RunError>,
        cancellation: &CancellationToken,
        run: &GraphRunIdentity,
        worker_panic: &mut Option<Box<dyn std::any::Any + Send>>,
        mut envelope: WorkerCompletion,
    ) {
        let completed_at = envelope.completed_at;
        let completion = envelope.completion;
        let Some(active) = running.get(&completion.operation) else {
            return;
        };
        if active.activation != completion.activation || active.attempt != completion.attempt {
            return;
        }
        let completed_before_deadline = self
            .options
            .deadline
            .is_none_or(|deadline| !deadline.exceeded_at(completed_at));
        let produced_ordinary_error = completion.outputs.as_ref().is_err_and(|error| {
            !matches!(
                error,
                RunError::Cancelled | RunError::DeadlineExceeded { .. }
            )
        });
        let active = running
            .remove(&completion.operation)
            .expect("validated running operation exists");
        admission.release(active.class);
        let mut suppress_completion = false;
        if matches!(terminal_error, Some(RunError::DeadlineExceeded { .. })) {
            if cancellation.is_cancelled() {
                *terminal_error = Some(RunError::Cancelled);
            }
            if completed_before_deadline
                && ordinary_precedes_cancellation(
                    produced_ordinary_error,
                    completed_at,
                    cancellation,
                )
            {
                *terminal_error = completion.outputs.clone().err();
            } else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                suppress_completion = true;
            }
        } else if !completed_before_deadline {
            *terminal_error = Some(if cancellation.is_cancelled() {
                RunError::Cancelled
            } else {
                RunError::DeadlineExceeded {
                    phase: RunPhase::Kernel,
                }
            });
            if let Some(key) = &active.memo_key {
                memo_inflight.remove(key);
            }
            suppress_completion = true;
        } else if terminal_error.is_none()
            && !produced_ordinary_error
            && let Err(error) =
                check_terminal(cancellation, self.options.deadline, RunPhase::Kernel)
        {
            *terminal_error = Some(error);
        }
        if terminal_error.is_none()
            && cancellation.is_cancelled()
            && !ordinary_precedes_cancellation(produced_ordinary_error, completed_at, cancellation)
        {
            *terminal_error = Some(RunError::Cancelled);
        }
        apply_authoritative_attempt_outcome(
            &mut envelope.trace_spans,
            &plan.operations[completion.operation.index()].stable_id,
            &completion,
            completed_at,
            cancellation.cancelled_at(),
            self.options.deadline,
        );
        for span in envelope.trace_spans {
            complete_span_safely(self.trace, span);
        }
        if worker_panic.is_none() {
            *worker_panic = envelope.panic;
        }
        if suppress_completion {
            if active.owns_memo_flight {
                _memoization.abort_owned(
                    &active,
                    terminal_error.clone().unwrap_or(RunError::Cancelled),
                );
            }
            self.transition_group_terminal(
                completion.output_group.as_ref(),
                terminal_error.as_ref().unwrap_or(&RunError::Cancelled),
            );
            return;
        }

        if terminal_error.is_some() {
            if matches!(terminal_error, Some(RunError::Cancelled))
                && completion
                    .outputs
                    .as_ref()
                    .is_err_and(|error| !matches!(error, RunError::Cancelled))
                && ordinary_precedes_cancellation(
                    produced_ordinary_error,
                    completed_at,
                    cancellation,
                )
            {
                *terminal_error = completion.outputs.clone().err();
            }
            if active.owns_memo_flight {
                _memoization.abort_owned(
                    &active,
                    terminal_error
                        .as_ref()
                        .expect("terminal error is present")
                        .clone(),
                );
            }
            self.transition_group_terminal(
                completion.output_group.as_ref(),
                terminal_error.as_ref().expect("terminal error is present"),
            );
            if let Some(key) = &active.memo_key {
                memo_inflight.remove(key);
            }
            return;
        }

        let retry_policy = plan.operations[completion.operation.index()]
            .retry
            .policy
            .filter(|_| {
                completion.outputs.as_ref().is_err_and(|error| {
                    matches!(
                        error,
                        RunError::KernelFailed {
                            kind: KernelErrorKind::Transient,
                            ..
                        }
                    )
                })
            })
            .filter(|policy| completion.attempt.get() < u64::from(policy.max_attempts.get()));
        if let Some(policy) = retry_policy {
            let backoff = retry_backoff(policy, completion.attempt);
            let Some(eligible_at) = Instant::now().checked_add(backoff) else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                let error = RunError::DeadlineExceeded {
                    phase: RunPhase::QueueWait,
                };
                _memoization.abort_owned(&active, error.clone());
                self.transition_group_terminal(active.output_group.as_ref(), &error);
                *terminal_error = Some(error);
                return;
            };
            if cancellation.is_cancelled()
                || self
                    .options
                    .deadline
                    .is_some_and(|deadline| deadline.exceeded_at(eligible_at))
            {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                let error = if cancellation.is_cancelled() {
                    RunError::Cancelled
                } else {
                    RunError::DeadlineExceeded {
                        phase: RunPhase::QueueWait,
                    }
                };
                _memoization.abort_owned(&active, error.clone());
                self.transition_group_terminal(active.output_group.as_ref(), &error);
                *terminal_error = Some(error);
                return;
            }
            let Some(attempt) = completion.attempt.next_checked() else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                let error = RunError::InvalidPlan("retry attempt identity overflowed".into());
                _memoization.abort_owned(&active, error.clone());
                self.transition_group_terminal(active.output_group.as_ref(), &error);
                *terminal_error = Some(error);
                return;
            };
            let tie_break = *next_retry_tie;
            let Some(next_tie) = tie_break.checked_add(1) else {
                if let Some(key) = &active.memo_key {
                    memo_inflight.remove(key);
                }
                let error =
                    RunError::InvalidPlan("delayed retry tie-break identity overflowed".into());
                _memoization.abort_owned(&active, error.clone());
                self.transition_group_terminal(active.output_group.as_ref(), &error);
                *terminal_error = Some(error);
                return;
            };
            *next_retry_tie = next_tie;
            #[cfg(test)]
            self.run_test_checkpoint(
                SchedulerCheckpoint::RetryBackoff {
                    operation: completion.operation,
                    activation: completion.activation,
                    attempt: completion.attempt,
                },
                cancellation,
            );
            delayed_operations.insert(completion.operation);
            delayed_retries.push(Reverse(DelayedRetry {
                eligible_at,
                tie_break,
                operation: completion.operation,
                owner_activation: active.owner_activation,
                activation: active.activation,
                attempt,
                input_result_ids: active.input_result_ids,
                output_group: active.output_group,
                memo_key: active.memo_key,
                memo_policy: active.memo_policy,
                class: active.class,
            }));
            return;
        }

        if let Some(key) = &active.memo_key {
            memo_inflight.remove(key);
        }
        match completion.outputs {
            Ok(outputs) => {
                if cancellation.is_cancelled() {
                    if active.owns_memo_flight {
                        _memoization.abort_owned(&active, RunError::Cancelled);
                    }
                    self.transition_group_terminal(
                        completion.output_group.as_ref(),
                        &RunError::Cancelled,
                    );
                    *terminal_error = Some(RunError::Cancelled);
                    return;
                }
                let activation_key = MemoKey {
                    frame: frame.id,
                    activation: active.owner_activation,
                    operation: completion.operation,
                };
                if frame.attempted.get(&activation_key) != Some(&completion.attempt)
                    || frame.completed.contains(&activation_key)
                {
                    let error = RunError::InvalidPlan(
                        "operation completion no longer matches its active attempt".into(),
                    );
                    _memoization.abort_owned(&active, error.clone());
                    self.transition_group_terminal(completion.output_group.as_ref(), &error);
                    *terminal_error = Some(error);
                    return;
                }
                #[cfg(test)]
                if completion.output_group.is_some() {
                    self.run_test_checkpoint(SchedulerCheckpoint::BeforeGroupCommit, cancellation);
                }
                if !active.reused_memo
                    && let Some(group) = completion.output_group.as_ref()
                    && let Err(error) = self.complete_result_group(group, outputs)
                {
                    if active.owns_memo_flight {
                        _memoization.abort_owned(&active, error.clone());
                    }
                    self.transition_group_terminal(Some(group), &error);
                    *terminal_error = Some(error);
                    cancellation.cancel();
                    return;
                }
                if active.owns_memo_flight
                    && let (Some(key), Some(group)) = (&active.memo_key, &completion.output_group)
                    && !_memoization
                        .for_policy(active.memo_policy)
                        .expect("memoized operation has a memo table")
                        .commit_completed(
                            key.clone(),
                            &group.output_result_ids,
                            self.result_store(),
                        )
                {
                    let error = RunError::Cancelled;
                    self.transition_group_terminal(Some(group), &error);
                    *terminal_error = Some(error);
                    cancellation.cancel();
                    return;
                }
                frame.completed.insert(activation_key);
                *frame
                    .completion_counts
                    .entry(completion.operation)
                    .or_default() += 1;
                pending.remove(&completion.operation);
                let operation = &plan.operations[completion.operation.index()];
                if operation.source_node_type_id.as_str() == "yssbi.debug.view" {
                    let Some(result_id) = active.input_result_ids.first().copied() else {
                        *terminal_error = Some(RunError::InvalidPlan(
                            "View Data operation has no Data input result".into(),
                        ));
                        cancellation.cancel();
                        return;
                    };
                    tracing::info!(
                        target: "yssbi::node_system::runtime::view",
                        diagnostic_domain = "ui",
                        diagnostic_event = "openResultWindow",
                        diagnostic_source = "yssbi.debug.view",
                        result_id = result_id.get(),
                        run_id = run.run_id.get(),
                        activation_id = completion.activation.get(),
                        node_id = %operation.source_node_id,
                        "View Data result is ready"
                    );
                    self.record_event(run, RunEventKind::OpenResultWindow { result_id });
                }
                if let Err(error) = self.propagate_value_dependencies(plan, frame) {
                    *terminal_error = Some(error);
                    cancellation.cancel();
                    return;
                }
            }
            Err(error) => {
                if active.owns_memo_flight {
                    _memoization.abort_owned(&active, error.clone());
                }
                self.transition_group_terminal(completion.output_group.as_ref(), &error);
                if let Err(propagation_error) = self.propagate_value_dependencies(plan, frame) {
                    *terminal_error = Some(propagation_error);
                    cancellation.cancel();
                    return;
                }
                self.terminalize_dependent_operations(plan, frame, pending, run.run_id, &error);
                *terminal_error = Some(error);
                cancellation.cancel();
            }
        }
    }

    pub(super) fn terminalize_dependent_operations(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        pending: &mut BTreeSet<OperationIndex>,
        run_id: RunId,
        upstream_error: &RunError,
    ) {
        loop {
            let affected = pending
                .iter()
                .copied()
                .filter_map(|operation_index| {
                    let operation = &plan.operations[operation_index.index()];
                    let source = operation.inputs.iter().find_map(|input| {
                        let result_id = frame.result_id(input.value).ok()?;
                        let result = self.result_store().result(result_id)?;
                        matches!(
                            result.state,
                            ResultState::Failed(_) | ResultState::Cancelled
                        )
                        .then_some((result_id, result.state.clone()))
                    })?;
                    Some((operation_index, source))
                })
                .collect::<Vec<_>>();
            if affected.is_empty() {
                break;
            }
            for (operation_index, (source_result_id, source_state)) in affected {
                let operation = &plan.operations[operation_index.index()];
                let activation = match self.activation_ids.allocate() {
                    Ok(activation) => activation,
                    Err(_) => continue,
                };
                let descriptors = operation
                    .outputs
                    .iter()
                    .map(|output| PendingOutputDescriptor {
                        value: output.value,
                        output: output.public_output.clone(),
                        presentation: output.presentation,
                        contract: output.contract.clone(),
                    })
                    .collect::<Vec<_>>();
                let group = if descriptors.is_empty() {
                    None
                } else {
                    match self.result_store().create_pending_group(
                        self.activation_provenance(
                            run_id,
                            activation,
                            plan,
                            operation.source_node_id,
                        ),
                        &descriptors,
                    ) {
                        Ok(group) => Some(group),
                        Err(_) => continue,
                    }
                };
                if let Some(group) = &group {
                    for (output, result_id) in operation
                        .outputs
                        .iter()
                        .zip(group.output_result_ids.iter().copied())
                    {
                        let _ = frame.bind_result(output.value, result_id);
                    }
                    let error = match source_state {
                        ResultState::Failed(failure) => RunError::UpstreamResultFailed {
                            source_result_id,
                            message: failure.message.clone(),
                        },
                        ResultState::Cancelled => {
                            RunError::UpstreamResultCancelled { source_result_id }
                        }
                        _ => upstream_error.clone(),
                    };
                    self.transition_group_terminal(Some(group), &error);
                }
                pending.remove(&operation_index);
            }
            let _ = self.propagate_value_dependencies(plan, frame);
        }
    }
}
