use super::*;

impl<'a> RunExecutor<'a> {
    pub(super) fn create_pending_operation_group(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        run_id: RunId,
        job: &mut PreparedOperation,
    ) -> Result<(), RunError> {
        if job.output_group.is_some() {
            return Ok(());
        }
        let operation = &plan.operations[job.operation.index()];
        if operation.outputs.is_empty() {
            return Ok(());
        }
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
        let group = self
            .result_store()
            .create_pending_group(
                self.activation_provenance(run_id, job.activation, plan, operation.source_node_id),
                &descriptors,
            )
            .map_err(result_store_error)?;
        for (output, result_id) in operation
            .outputs
            .iter()
            .zip(group.output_result_ids.iter().copied())
        {
            frame.bind_result(output.value, result_id)?;
        }
        job.output_group = Some(group);
        Ok(())
    }

    pub(super) fn bind_reused_operation(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        run_id: RunId,
        job: &mut PreparedOperation,
        result_ids: &[ResultId],
    ) -> Result<(), RunError> {
        let operation = &plan.operations[job.operation.index()];
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
        if descriptors.is_empty() {
            job.reused_memo = true;
            return Ok(());
        }
        let original_activation_id = result_ids
            .first()
            .and_then(|result_id| self.result_store().result(*result_id))
            .map(|result| result.provenance.activation_id)
            .ok_or_else(|| RunError::InvalidPlan("memoized result is unavailable".into()))?;
        let mut provenance =
            self.activation_provenance(run_id, job.activation, plan, operation.source_node_id);
        provenance.usage = ResultUsage::Reused {
            original_activation_id,
        };
        let group = self
            .result_store()
            .record_reused_group(provenance, &descriptors, result_ids)
            .map_err(result_store_error)?;
        self.record_result_group_changed(plan, &group, super::super::ResultStateKind::Ready);
        for (output, result_id) in operation
            .outputs
            .iter()
            .zip(group.output_result_ids.iter().copied())
        {
            frame.bind_result(output.value, result_id)?;
        }
        job.output_group = Some(group);
        job.reused_memo = true;
        Ok(())
    }

    pub(super) fn prepare_operation(
        &self,
        plan: &ExecutionPlan,
        operation_index: OperationIndex,
        owner_activation: ActivationId,
        frame: &mut Frame,
        memoization: &MemoTables,
        demand: &DemandFingerprint,
        run_id: RunId,
        _cancellation: &CancellationToken,
    ) -> Result<PreparedOperation, RunError> {
        let attempt = AttemptId::initial();
        let activation = self.activation_ids.allocate()?;
        let operation = &plan.operations[operation_index.index()];
        if operation.source_node_type_id.as_str() == "yssbi.debug.view"
            && (operation.inputs.len() != 1 || !operation.outputs.is_empty())
        {
            return Err(RunError::InvalidPlan(
                "View Data operation must have exactly one Data input and no data outputs".into(),
            ));
        }
        for input in &operation.inputs {
            if !frame.has(input.value) {
                let value = input
                    .bound_value
                    .as_ref()
                    .ok_or(RunError::MissingValue(input.value))?;
                let result_id = self.create_internal_ready_result(
                    run_id,
                    plan,
                    operation.source_node_id,
                    input.value,
                    input.contract.clone(),
                    value.clone(),
                )?;
                frame.bind_result(input.value, result_id)?;
            }
        }
        let input_result_ids = operation
            .inputs
            .iter()
            .map(|input| frame.result_id(input.value))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let memo_key = if operation.cache_policy != CachePolicy::Disabled
            && operation_memoization_safe(plan, operation_index)
        {
            operation_resource_versions(plan, operation_index).and_then(|resource_versions| {
                OperationMemoKey::from_inputs(
                    operation.stable_id.clone(),
                    &input_result_ids,
                    self.result_store(),
                    resource_versions,
                    operation.semantics_version,
                    self.computation_settings,
                    demand.clone(),
                )
            })
        } else {
            None
        };
        let memo_reservation = memo_key
            .as_ref()
            .map(|key| {
                memoization
                    .for_policy(operation.cache_policy)
                    .expect("memoized operation has a memo table")
                    .reserve(key, self.result_store())
            })
            .transpose()?;
        let owns_memo_flight = matches!(
            memo_reservation,
            Some(crate::node_system::runtime::MemoReservation::Producer)
        );
        let waiting_for_memo = matches!(
            memo_reservation,
            Some(crate::node_system::runtime::MemoReservation::Running)
        );
        let cached_result_ids = match memo_reservation {
            Some(crate::node_system::runtime::MemoReservation::Complete(result_ids)) => {
                Some(result_ids)
            }
            Some(
                crate::node_system::runtime::MemoReservation::Running
                | crate::node_system::runtime::MemoReservation::Producer,
            )
            | None => None,
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
        let reused_memo = cached_result_ids.is_some();
        let output_group = if descriptors.is_empty() || waiting_for_memo || reused_memo {
            None
        } else {
            let group = match self.result_store().create_pending_group(
                self.activation_provenance(run_id, activation, plan, operation.source_node_id),
                &descriptors,
            ) {
                Ok(group) => group,
                Err(error) => {
                    let error = result_store_error(error);
                    memoization.abort_flight(
                        owns_memo_flight,
                        memo_key.as_ref(),
                        operation.cache_policy,
                        error.clone(),
                    );
                    return Err(error);
                }
            };
            for (output, result_id) in operation
                .outputs
                .iter()
                .zip(group.output_result_ids.iter().copied())
            {
                if let Err(error) = frame.bind_result(output.value, result_id) {
                    memoization.abort_flight(
                        owns_memo_flight,
                        memo_key.as_ref(),
                        operation.cache_policy,
                        error.clone(),
                    );
                    self.transition_group_terminal(plan, Some(&group), &error);
                    return Err(error);
                }
            }
            Some(group)
        };
        Ok(PreparedOperation {
            operation: operation_index,
            owner_activation,
            activation,
            attempt,
            input_result_ids,
            output_group,
            memo_key,
            memo_policy: operation.cache_policy,
            owns_memo_flight,
            awaits_memo_flight: waiting_for_memo,
            reused_memo,
            class: operation.workload,
        })
    }

    pub(super) fn bookkeep_admission(
        &self,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        job: &PreparedOperation,
        class: WorkloadClass,
        track_memo: bool,
    ) -> AdmissionBookkeeping {
        let activation_key = MemoKey {
            frame: frame.id,
            activation: job.owner_activation,
            operation: job.operation,
        };
        let previous_attempt = frame.attempted.insert(activation_key, job.attempt);
        if job.attempt == AttemptId::initial() {
            debug_assert!(previous_attempt.is_none());
        } else {
            debug_assert_eq!(
                previous_attempt.and_then(AttemptId::next_checked),
                Some(job.attempt)
            );
        }
        let previous_running = running.insert(
            job.operation,
            RunningOperation {
                class,
                owner_activation: job.owner_activation,
                activation: job.activation,
                attempt: job.attempt,
                input_result_ids: job.input_result_ids.clone(),
                output_group: job.output_group.clone(),
                memo_key: job.memo_key.clone(),
                memo_policy: job.memo_policy,
                owns_memo_flight: job.owns_memo_flight,
                reused_memo: job.reused_memo,
            },
        );
        debug_assert!(previous_running.is_none());
        if track_memo && let Some(key) = &job.memo_key {
            let inserted = memo_inflight.insert(key.clone());
            debug_assert!(inserted || job.attempt != AttemptId::initial());
        }
        debug_assert_eq!(admission.running_count(), running.len());
        AdmissionBookkeeping {
            operation: job.operation,
            class,
            activation_key,
            previous_attempt,
            memo_key: if track_memo {
                job.memo_key.clone()
            } else {
                None
            },
        }
    }

    pub(super) fn rollback_admission(
        &self,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        bookkeeping: AdmissionBookkeeping,
        _attempt: AttemptId,
        _cancellation: &CancellationToken,
    ) {
        let removed = running.remove(&bookkeeping.operation);
        debug_assert!(removed.is_some());
        admission.release(bookkeeping.class);
        match bookkeeping.previous_attempt {
            Some(previous) => {
                frame.attempted.insert(bookkeeping.activation_key, previous);
            }
            None => {
                frame.attempted.remove(&bookkeeping.activation_key);
            }
        }
        if let Some(key) = &bookkeeping.memo_key {
            memo_inflight.remove(key);
        }
        debug_assert_eq!(admission.running_count(), running.len());
        #[cfg(test)]
        self.run_test_checkpoint(
            SchedulerCheckpoint::AdmissionRolledBack {
                operation: bookkeeping.operation,
                attempt: _attempt,
                running_count: admission.running_count(),
                tracked_running: running.len(),
                memo_owned: bookkeeping
                    .memo_key
                    .as_ref()
                    .is_some_and(|key| memo_inflight.contains(key)),
                frame_attempt: frame.attempted.get(&bookkeeping.activation_key).copied(),
            },
            _cancellation,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn submit_admitted_operation(
        &self,
        plan: &ExecutionPlan,
        frame: &mut Frame,
        admission: &mut ClassScheduler,
        running: &mut BTreeMap<OperationIndex, RunningOperation>,
        memo_inflight: &mut BTreeSet<OperationMemoKey>,
        job_queue: &WorkerQueue<PreparedOperation>,
        memoization: &MemoTables,
        job: PreparedOperation,
        class: WorkloadClass,
        cancellation: &CancellationToken,
        parent_call: Option<ParentCallId>,
        run_id: RunId,
    ) -> Result<(), RunError> {
        let operation = job.operation;
        let activation = job.activation;
        let attempt = job.attempt;
        let output_group = job.output_group.clone();
        let bookkeeping =
            self.bookkeep_admission(frame, admission, running, memo_inflight, &job, class, true);
        #[cfg(test)]
        self.run_test_checkpoint(
            SchedulerCheckpoint::AdmissionBookkept {
                operation,
                activation,
                attempt,
            },
            cancellation,
        );
        let owned_memo_key = job.owns_memo_flight.then(|| job.memo_key.clone()).flatten();
        if job_queue.push(job).is_err() {
            self.rollback_admission(
                frame,
                admission,
                running,
                memo_inflight,
                bookkeeping,
                attempt,
                cancellation,
            );
            let error = check_terminal(cancellation, self.options.deadline, RunPhase::QueueWait)
                .err()
                .unwrap_or_else(|| RunError::InvalidPlan("operation worker queue closed".into()));
            memoization.abort_flight(
                owned_memo_key.is_some(),
                owned_memo_key.as_ref(),
                plan.operations[operation.index()].cache_policy,
                error.clone(),
            );
            self.transition_group_terminal(plan, output_group.as_ref(), &error);
            return Err(error);
        }
        let correlation = operation_correlation(plan, run_id, parent_call, operation);
        self.record_operation_started(plan, correlation, operation, activation, attempt);
        Ok(())
    }
}
