use super::*;

pub(super) struct ReadyOperationContext<'context, 'memo> {
    pub(super) run: &'context GraphRunIdentity,
    pub(super) run_output: &'context dyn RunOutputSink,
    pub(super) plan: &'context ExecutionPlan,
    pub(super) operations: &'context [OperationIndex],
    pub(super) activation_id: ActivationId,
    pub(super) activated: &'context mut BTreeSet<OperationIndex>,
    pub(super) frame: &'context mut Frame,
    pub(super) resources: &'context RunResourceSet,
    pub(super) resource_owner: &'context RunResourceOwner,
    pub(super) relational_backends: &'context RunRelationalBackends,
    pub(super) memoization: &'context MemoTables<'memo>,
    pub(super) demand: &'context DemandFingerprint,
    pub(super) cancellation: &'context CancellationToken,
    pub(super) run_parent_span_id: Option<SpanId>,
    pub(super) parent_call: Option<ParentCallId>,
}

impl<'a> RunExecutor<'a> {
    pub(super) fn execute_ready_operations(
        &self,
        context: ReadyOperationContext<'_, '_>,
    ) -> Result<(), RunError> {
        let ReadyOperationContext {
            run,
            run_output,
            plan,
            operations,
            activation_id,
            activated,
            frame,
            resources,
            resource_owner,
            relational_backends,
            memoization,
            demand,
            cancellation,
            run_parent_span_id,
            parent_call,
        } = context;
        let run_id = run.run_id;
        self.propagate_value_dependencies(plan, frame)?;
        let mut pending = BTreeSet::new();
        for operation in operations {
            if !activated.insert(*operation) || !pending.insert(*operation) {
                return Err(RunError::OperationAlreadyExecuted {
                    operation: *operation,
                    activation: activation_id,
                });
            }
        }

        let mut prepared = BTreeMap::new();
        let mut queued = BTreeSet::new();
        let mut running = BTreeMap::new();
        let mut delayed_retries: BinaryHeap<Reverse<DelayedRetry>> = BinaryHeap::new();
        let mut delayed_operations = BTreeSet::new();
        let mut next_retry_tie = 0_u64;
        let mut memo_inflight = BTreeSet::new();
        let mut admission = ClassScheduler::new(self.options.scheduling);
        let worker_count = self.options.scheduling.worker_count();
        let job_queue: WorkerQueue<PreparedOperation> =
            WorkerQueue::new(worker_count, cancellation.clone(), self.options.deadline);
        let (completion_sender, completion_receiver) = mpsc::sync_channel(worker_count);
        let scheduler_signal = SchedulerSignal::new(cancellation);
        let mut worker_panic = None;
        let worker_context = OperationWorkerContext {
            run_id,
            run_output,
            frame_id: frame.id,
            computation_settings: self.computation_settings,
            plan,
            kernels: self.kernels,
            compiled_parameters: self.compiled_parameters,
            resources,
            resource_owner,
            relational_backends,
            results: self.result_store(),
            cancellation,
            deadline: self.options.deadline,
            run_parent_span_id,
            parent_call,
            #[cfg(test)]
            checkpoint: self.checkpoint.as_ref(),
        };

        let result = std::thread::scope(|scope| {
            let _queue_close = WorkerQueueCloseGuard(&job_queue);
            for _ in 0..worker_count {
                let sender = completion_sender.clone();
                let queue = &job_queue;
                let context = &worker_context;
                let signal = &scheduler_signal;
                scope.spawn(move || {
                    while let Some(job) = queue.pop() {
                        let operation = job.operation;
                        let activation = job.activation;
                        let attempt = job.attempt;
                        let output_group = job.output_group.clone();
                        let trace = WorkerTrace::default();
                        let (outputs, panic, completed_at) =
                            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let outputs = execute_operation_worker(context, job, &trace);
                                (outputs, Instant::now())
                            })) {
                                Ok((outputs, completed_at)) => (outputs, None, completed_at),
                                Err(payload) => (
                                    Err(RunError::InvalidPlan("operation worker panicked".into())),
                                    Some(payload),
                                    Instant::now(),
                                ),
                            };
                        #[cfg(test)]
                        if let Some(checkpoint) = context.checkpoint {
                            checkpoint(
                                SchedulerCheckpoint::WorkerOutcomeProduced,
                                context.cancellation,
                            );
                        }
                        if sender
                            .send(WorkerCompletion {
                                completed_at,
                                completion: OperationCompletion {
                                    operation,
                                    activation,
                                    attempt,
                                    output_group,
                                    outputs,
                                },
                                trace_spans: trace.into_spans(),
                                panic,
                            })
                            .is_err()
                        {
                            break;
                        }
                        signal.notify();
                    }
                });
            }

            let scheduler_result = (|| {
                let mut terminal_error = None;
                loop {
                    if terminal_error.is_none() {
                        let phase =
                            if admission.has_queued() || !prepared.is_empty() || !queued.is_empty()
                            {
                                RunPhase::QueueWait
                            } else {
                                RunPhase::Kernel
                            };
                        if let Err(error) =
                            check_terminal(cancellation, self.options.deadline, phase)
                        {
                            terminal_error = Some(error);
                        }
                    }
                    if terminal_error.is_none() && cancellation.is_cancelled() && running.is_empty()
                    {
                        terminal_error = Some(RunError::Cancelled);
                    }

                    if terminal_error.is_none() && !cancellation.is_cancelled() {
                        while delayed_retries
                            .peek()
                            .is_some_and(|Reverse(retry)| retry.eligible_at <= Instant::now())
                        {
                            let Reverse(retry) =
                                delayed_retries.pop().expect("peeked delayed retry exists");
                            delayed_operations.remove(&retry.operation);
                            if let Err(error) = check_terminal(
                                cancellation,
                                self.options.deadline,
                                RunPhase::QueueWait,
                            ) {
                                if let Some(key) = &retry.memo_key {
                                    memo_inflight.remove(key);
                                }
                                memoization.abort_delayed(&retry, error.clone());
                                self.transition_group_terminal(retry.output_group.as_ref(), &error);
                                terminal_error = Some(error);
                                break;
                            }
                            let prepared_retry = PreparedOperation {
                                operation: retry.operation,
                                owner_activation: retry.owner_activation,
                                activation: retry.activation,
                                attempt: retry.attempt,
                                input_result_ids: retry.input_result_ids,
                                output_group: retry.output_group,
                                memo_key: retry.memo_key,
                                memo_policy: retry.memo_policy,
                                owns_memo_flight: true,
                                awaits_memo_flight: false,
                                reused_memo: false,
                                class: retry.class,
                            };
                            #[cfg(test)]
                            self.run_test_checkpoint(
                                SchedulerCheckpoint::AttemptPrepared {
                                    operation: prepared_retry.operation,
                                    activation: prepared_retry.activation,
                                    attempt: prepared_retry.attempt,
                                },
                                cancellation,
                            );
                            prepared.insert(prepared_retry.operation, prepared_retry);
                        }

                        for operation in pending.iter().copied().collect::<Vec<_>>() {
                            if prepared.contains_key(&operation)
                                || queued.contains(&operation)
                                || running.contains_key(&operation)
                                || delayed_operations.contains(&operation)
                                || !self.operation_is_ready(
                                    plan,
                                    operation,
                                    activation_id,
                                    activated,
                                    frame,
                                )
                            {
                                continue;
                            }
                            match self.prepare_operation(
                                plan,
                                operation,
                                activation_id,
                                frame,
                                memoization,
                                demand,
                                run_id,
                                cancellation,
                            ) {
                                Ok(operation) => {
                                    #[cfg(test)]
                                    self.run_test_checkpoint(
                                        SchedulerCheckpoint::AttemptPrepared {
                                            operation: operation.operation,
                                            activation: operation.activation,
                                            attempt: operation.attempt,
                                        },
                                        cancellation,
                                    );
                                    prepared.insert(operation.operation, operation);
                                }
                                Err(error) => terminal_error = Some(error),
                            }
                        }

                        for (operation, job) in &prepared {
                            if queued.contains(operation)
                                || job.memo_key.as_ref().is_some_and(|key| {
                                    memo_inflight.contains(key) && !job.owns_memo_flight
                                })
                            {
                                continue;
                            }
                            admission.enqueue(*operation, job.class);
                            queued.insert(*operation);
                        }

                        while let Some((operation, class)) = admission.admit() {
                            queued.remove(&operation);
                            let mut job = prepared
                                .remove(&operation)
                                .expect("admitted operations are prepared");

                            let memo_table = memoization.for_policy(job.memo_policy);
                            let cached_result_ids = if job.awaits_memo_flight {
                                let key = job.memo_key.as_ref().expect("memo waiter has a key");
                                loop {
                                    match memo_table
                                        .expect("memoized job has a memo table")
                                        .wait_completed(key, cancellation)
                                    {
                                        Ok(result_ids) => break Some(result_ids),
                                        Err(RunError::MemoizationRetry)
                                            if job.memo_policy == CachePolicy::PerSession =>
                                        {
                                            match memo_table
                                                .expect("memoized job has a memo table")
                                                .reserve(key, self.result_store())
                                            {
                                                Ok(crate::node_system::runtime::MemoReservation::Complete(
                                                    result_ids,
                                                )) => {
                                                    break Some(result_ids);
                                                }
                                                Ok(crate::node_system::runtime::MemoReservation::Producer) => {
                                                    job.owns_memo_flight = true;
                                                    job.awaits_memo_flight = false;
                                                    break None;
                                                }
                                                Ok(crate::node_system::runtime::MemoReservation::Running) => continue,
                                                Err(error) => {
                                                    terminal_error = Some(error);
                                                    break None;
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            terminal_error = Some(error);
                                            break None;
                                        }
                                    }
                                }
                            } else {
                                job.memo_key.as_ref().and_then(|key| {
                                    memo_table
                                        .expect("memoized job has a memo table")
                                        .completed(key, self.result_store())
                                })
                            };
                            if terminal_error.is_some() {
                                break;
                            }
                            if let Some(result_ids) = cached_result_ids {
                                if job.output_group.is_none()
                                    && let Err(error) = self.bind_reused_operation(
                                        plan,
                                        frame,
                                        run_id,
                                        &mut job,
                                        &result_ids,
                                    )
                                {
                                    let key = job.memo_key.as_ref().expect("cache hit has a key");
                                    memo_table
                                        .expect("memoized job has a memo table")
                                        .invalidate(key);
                                    match memo_table
                                        .expect("memoized job has a memo table")
                                        .reserve(key, self.result_store())
                                    {
                                        Ok(
                                            crate::node_system::runtime::MemoReservation::Producer,
                                        ) => {
                                            job.owns_memo_flight = true;
                                            job.awaits_memo_flight = false;
                                            job.reused_memo = false;
                                            if let Err(group_error) = self
                                                .create_pending_operation_group(
                                                    plan, frame, run_id, &mut job,
                                                )
                                            {
                                                memoization
                                                    .abort_prepared(&job, group_error.clone());
                                                terminal_error = Some(group_error);
                                                break;
                                            }
                                        }
                                        Ok(_) => {
                                            terminal_error = Some(error);
                                            break;
                                        }
                                        Err(reserve_error) => {
                                            terminal_error = Some(reserve_error);
                                            break;
                                        }
                                    }
                                }
                                if !job.reused_memo {
                                    if let Err(error) = self.submit_admitted_operation(
                                        plan,
                                        frame,
                                        &mut admission,
                                        &mut running,
                                        &mut memo_inflight,
                                        &job_queue,
                                        memoization,
                                        job,
                                        class,
                                        cancellation,
                                    ) {
                                        terminal_error = Some(error);
                                    }
                                    continue;
                                }
                                self.bookkeep_admission(
                                    frame,
                                    &mut admission,
                                    &mut running,
                                    &mut memo_inflight,
                                    &job,
                                    class,
                                    false,
                                );
                                self.finish_operation_completion(
                                    plan,
                                    frame,
                                    memoization,
                                    &mut admission,
                                    &mut running,
                                    &mut prepared,
                                    &mut delayed_retries,
                                    &mut delayed_operations,
                                    &mut next_retry_tie,
                                    &mut memo_inflight,
                                    &mut pending,
                                    &mut terminal_error,
                                    cancellation,
                                    run,
                                    &mut worker_panic,
                                    WorkerCompletion {
                                        completed_at: Instant::now(),
                                        completion: OperationCompletion {
                                            operation,
                                            activation: job.activation,
                                            attempt: job.attempt,
                                            output_group: job.output_group,
                                            outputs: Ok(Box::new([])),
                                        },
                                        trace_spans: Box::new([]),
                                        panic: None,
                                    },
                                );
                                continue;
                            }
                            if job.owns_memo_flight
                                && job.output_group.is_none()
                                && let Err(error) = self
                                    .create_pending_operation_group(plan, frame, run_id, &mut job)
                            {
                                memoization.abort_prepared(&job, error.clone());
                                terminal_error.get_or_insert(error);
                                break;
                            }
                            if let Err(error) = self.submit_admitted_operation(
                                plan,
                                frame,
                                &mut admission,
                                &mut running,
                                &mut memo_inflight,
                                &job_queue,
                                memoization,
                                job,
                                class,
                                cancellation,
                            ) {
                                terminal_error.get_or_insert(error);
                                break;
                            }
                        }
                        #[cfg(test)]
                        if let Some(class) = admission.blocked_class() {
                            self.run_test_checkpoint(
                                SchedulerCheckpoint::AdmissionBlocked(class),
                                cancellation,
                            );
                        }
                    }

                    let mut drained_completion = false;
                    while let Ok(completion) = completion_receiver.try_recv() {
                        drained_completion = true;
                        self.finish_operation_completion(
                            plan,
                            frame,
                            memoization,
                            &mut admission,
                            &mut running,
                            &mut prepared,
                            &mut delayed_retries,
                            &mut delayed_operations,
                            &mut next_retry_tie,
                            &mut memo_inflight,
                            &mut pending,
                            &mut terminal_error,
                            cancellation,
                            run,
                            &mut worker_panic,
                            completion,
                        );
                    }
                    if drained_completion && terminal_error.is_none() {
                        continue;
                    }

                    if running.is_empty() {
                        if let Some(error) = terminal_error {
                            return Err(error);
                        }
                        if pending.is_empty() {
                            return Ok(());
                        }
                        if !admission.has_queued()
                            && prepared.is_empty()
                            && delayed_retries.is_empty()
                        {
                            return Err(self.blocked_operation_error(
                                plan,
                                *pending.first().expect("pending is not empty"),
                                activation_id,
                                activated,
                                frame,
                            ));
                        }
                    }

                    if terminal_error.is_some() && !running.is_empty() {
                        match completion_receiver.recv() {
                            Ok(completion) => self.finish_operation_completion(
                                plan,
                                frame,
                                memoization,
                                &mut admission,
                                &mut running,
                                &mut prepared,
                                &mut delayed_retries,
                                &mut delayed_operations,
                                &mut next_retry_tie,
                                &mut memo_inflight,
                                &mut pending,
                                &mut terminal_error,
                                cancellation,
                                run,
                                &mut worker_panic,
                                completion,
                            ),
                            Err(_) => {
                                terminal_error.get_or_insert_with(|| {
                                    RunError::InvalidPlan(
                                        "operation completion channel closed".into(),
                                    )
                                });
                            }
                        }
                        continue;
                    }

                    if terminal_error.is_none()
                        && (!running.is_empty() || !delayed_retries.is_empty())
                    {
                        let phase = if admission.has_queued() || !delayed_retries.is_empty() {
                            RunPhase::QueueWait
                        } else {
                            RunPhase::Kernel
                        };
                        let retry_wait = delayed_retries.peek().map(|Reverse(retry)| {
                            retry.eligible_at.saturating_duration_since(Instant::now())
                        });
                        let deadline_wait = match self.options.deadline {
                            Some(deadline) => match deadline.remaining(cancellation, phase) {
                                Ok(remaining) => Some(remaining),
                                Err(error) => {
                                    terminal_error = Some(error);
                                    continue;
                                }
                            },
                            None => None,
                        };
                        let timeout = match (retry_wait, deadline_wait) {
                            (Some(retry), Some(deadline)) => Some(retry.min(deadline)),
                            (Some(retry), None) => Some(retry),
                            (None, Some(deadline)) => Some(deadline),
                            (None, None) => None,
                        };
                        if timeout.is_some_and(|timeout| timeout.is_zero()) {
                            continue;
                        }

                        let mut notified = scheduler_signal
                            .notified
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        if let Ok(completion) = completion_receiver.try_recv() {
                            *notified = false;
                            drop(notified);
                            self.finish_operation_completion(
                                plan,
                                frame,
                                memoization,
                                &mut admission,
                                &mut running,
                                &mut prepared,
                                &mut delayed_retries,
                                &mut delayed_operations,
                                &mut next_retry_tie,
                                &mut memo_inflight,
                                &mut pending,
                                &mut terminal_error,
                                cancellation,
                                run,
                                &mut worker_panic,
                                completion,
                            );
                            continue;
                        }
                        if cancellation.is_cancelled() {
                            drop(notified);
                            continue;
                        }
                        if let Some(timeout) = timeout {
                            let (mut notified, _) = scheduler_signal
                                .ready
                                .wait_timeout_while(notified, timeout, |notified| {
                                    !*notified && !cancellation.is_cancelled()
                                })
                                .unwrap_or_else(|error| error.into_inner());
                            *notified = false;
                        } else {
                            let mut notified = scheduler_signal
                                .ready
                                .wait_while(notified, |notified| {
                                    !*notified && !cancellation.is_cancelled()
                                })
                                .unwrap_or_else(|error| error.into_inner());
                            *notified = false;
                        }
                        continue;
                    }
                }
            })();
            if let Err(error) = &scheduler_result {
                for job in prepared.values() {
                    memoization.abort_prepared(job, error.clone());
                    self.transition_group_terminal(job.output_group.as_ref(), error);
                }
                for operation in running.values() {
                    memoization.abort_owned(operation, error.clone());
                    self.transition_group_terminal(operation.output_group.as_ref(), error);
                }
                for retry in delayed_retries.iter() {
                    memoization.abort_delayed(&retry.0, error.clone());
                    self.transition_group_terminal(retry.0.output_group.as_ref(), error);
                }
            }
            scheduler_result
        });
        if let Some(payload) = worker_panic {
            std::panic::resume_unwind(payload);
        }
        result
    }
}
