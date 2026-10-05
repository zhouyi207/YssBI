use crate::tools::HarnessToolExecutor;
use crate::{CompiledWorkflow, HarnessError, HarnessHost, ToolRegistry, WorkflowRuntime};
use std::sync::{Arc, Mutex, Weak};
use yss_harness_contract::{
    AutomationIdKind, CancellationReason, CancellationToken, CapabilityFailureCode, HarnessEvent,
    HarnessSessionId, HarnessSessionState, HarnessTurnId, WorkflowRunId, WorkflowRunRecord,
    WorkflowRunState,
};

impl HarnessHost {
    pub async fn plan_workflow(
        &self,
        session_id: &HarnessSessionId,
        turn_id: Option<&HarnessTurnId>,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        if let Some(turn_id) = turn_id {
            let turn = self
                .ports
                .sessions
                .load_turn(turn_id)
                .await?
                .ok_or(HarnessError::WorkflowTurnNotFound)?;
            if turn.session_id != session.id {
                return Err(HarnessError::WorkflowTurnMismatch);
            }
        }
        self.ports
            .workflows
            .save_definition(compiled.definition())
            .await?;
        let run_id =
            WorkflowRunId::try_new(self.ports.ids.next_id(AutomationIdKind::WorkflowRun)?)?;
        let run = WorkflowRuntime::plan(
            compiled,
            run_id.clone(),
            session.id.clone(),
            turn_id.cloned(),
            session.project,
            self.ports.clock.now(),
        );
        let run = self.ports.workflows.save_run(&run, None).await?;
        self.event_writer()
            .append(
                session_id,
                turn_id,
                HarnessEvent::WorkflowPlanned { run_id },
            )
            .await?;
        Ok(run)
    }

    pub async fn advance_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let control = self.workflow_control(run_id);
        let admission = control.admit()?;
        let transition = control.transitions.lock().await;
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        if matches!(
            run.state,
            WorkflowRunState::Completed | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            return Ok(run);
        }
        if run.state == WorkflowRunState::Paused {
            return Err(HarnessError::WorkflowWaiting);
        }
        if run
            .steps
            .values()
            .any(|step| step.state == yss_harness_contract::WorkflowStepState::Running)
        {
            return Err(HarnessError::ConcurrentWorkflow);
        }
        let session = self
            .ports
            .sessions
            .load_session(&run.session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active || session.project != run.project {
            run.state = WorkflowRunState::Paused;
            run.updated_at = self.ports.clock.now();
            self.persist_workflow(&mut run).await?;
            return Err(HarnessError::WorkflowProjectChanged);
        }
        let definition = self
            .ports
            .workflows
            .load_definition(&run.definition_id, &run.definition_version)
            .await?
            .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
        let compiled = CompiledWorkflow::compile(definition)?;
        let was_planned = run.state == WorkflowRunState::Planned;
        if matches!(
            run.state,
            WorkflowRunState::Planned | WorkflowRunState::Ready
        ) {
            WorkflowRuntime::start(&mut run, self.ports.clock.now())?;
            self.persist_workflow(&mut run).await?;
            if was_planned {
                self.event_writer()
                    .append(
                        &run.session_id,
                        run.turn_id.as_ref(),
                        HarnessEvent::WorkflowStarted {
                            run_id: run.id.clone(),
                        },
                    )
                    .await?;
            }
            if run.state == WorkflowRunState::Completed {
                self.event_writer()
                    .append(
                        &run.session_id,
                        run.turn_id.as_ref(),
                        HarnessEvent::WorkflowCompleted {
                            run_id: run.id.clone(),
                        },
                    )
                    .await?;
                return Ok(run);
            }
        }
        let Some(step_id) = WorkflowRuntime::ready_steps(&compiled, &run)?
            .into_iter()
            .next()
        else {
            return Ok(run);
        };
        let step = compiled
            .definition()
            .steps
            .iter()
            .find(|step| step.id == step_id)
            .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
        let request = &step.request;
        let turn_id = run
            .turn_id
            .clone()
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        let turn = self
            .ports
            .sessions
            .load_turn(&turn_id)
            .await?
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        if turn.session_id != run.session_id {
            return Err(HarnessError::WorkflowTurnMismatch);
        }
        WorkflowRuntime::start_step(&compiled, &mut run, &step_id, self.ports.clock.now())?;
        self.persist_workflow(&mut run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                Some(&turn_id),
                HarnessEvent::WorkflowStepStarted {
                    run_id: run.id.clone(),
                    step_id: step_id.clone(),
                },
            )
            .await?;
        let executor = HarnessToolExecutor::new_for_workflow(
            ToolRegistry::for_capability(request.capability_id()),
            Arc::clone(&self.ports.capability_gateway),
            Arc::clone(&self.knowledge),
            Arc::clone(&self.ports.tool_ledger),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
            session.principal_id,
            run.session_id.clone(),
            turn_id.clone(),
            run.project.clone(),
            admission.cancellation.clone(),
            run.id.clone(),
            step_id.clone(),
        );
        // Pause/cancel can acquire the transition gate while the owner
        // waits on the capability. A second advance still cannot enter.
        drop(transition);
        let outcome = executor.execute(request.clone(), None).await;
        let _completion = control.transitions.lock().await;
        let current = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        let same_attempt = current.steps.get(&step_id) == run.steps.get(&step_id);
        if admission.cancellation.is_cancelled()
            || !same_attempt
            || current.project != run.project
            || !matches!(
                current.state,
                WorkflowRunState::Running | WorkflowRunState::Paused | WorkflowRunState::Ready
            )
            || (current.revision != run.revision && current.state == WorkflowRunState::Running)
        {
            return Ok(current);
        }
        run = current;
        let current_session = self.ports.sessions.load_session(&run.session_id).await?;
        if current_session.is_none_or(|session| {
            session.state != HarnessSessionState::Active || session.project != run.project
        }) {
            run.state = WorkflowRunState::Paused;
            run.updated_at = self.ports.clock.now();
            self.persist_workflow(&mut run).await?;
            return Err(HarnessError::WorkflowProjectChanged);
        }
        match outcome {
            Ok(_) => {
                WorkflowRuntime::succeed_step(
                    &compiled,
                    &mut run,
                    &step_id,
                    self.ports.clock.now(),
                )?;
                self.persist_workflow(&mut run).await?;
                self.event_writer()
                    .append(
                        &run.session_id,
                        Some(&turn_id),
                        HarnessEvent::WorkflowStepCompleted {
                            run_id: run.id.clone(),
                            step_id,
                        },
                    )
                    .await?;
                if run.state == WorkflowRunState::Completed {
                    self.event_writer()
                        .append(
                            &run.session_id,
                            Some(&turn_id),
                            HarnessEvent::WorkflowCompleted {
                                run_id: run.id.clone(),
                            },
                        )
                        .await?;
                }
            }
            Err(failure) => {
                let retriable = workflow_failure_is_retriable(failure.code);
                WorkflowRuntime::fail_step(&mut run, &step_id, retriable, self.ports.clock.now())?;
                self.persist_workflow(&mut run).await?;
                self.event_writer()
                    .append(
                        &run.session_id,
                        Some(&turn_id),
                        HarnessEvent::WorkflowStepFailed {
                            run_id: run.id.clone(),
                            step_id,
                            retriable,
                        },
                    )
                    .await?;
            }
        }
        Ok(run)
    }

    pub async fn pause_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let control = self.workflow_control(run_id);
        let _transition = control.transitions.lock().await;
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::pause(&mut run, self.ports.clock.now())?;
        self.persist_workflow(&mut run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowPaused {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn resume_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let control = self.workflow_control(run_id);
        let _transition = control.transitions.lock().await;
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::resume(&mut run, self.ports.clock.now())?;
        self.persist_workflow(&mut run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowResumed {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn cancel_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let control = self.workflow_control(run_id);
        let _transition = control.transitions.lock().await;
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::cancel(&mut run, self.ports.clock.now())?;
        self.persist_workflow(&mut run).await?;
        control.cancel();
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowCancelled {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn recover_workflows(&self) -> Result<usize, HarnessError> {
        let mut recovered = 0usize;
        for snapshot in self.ports.workflows.load_recoverable_runs().await? {
            let control = self.workflow_control(&snapshot.id);
            let Ok(_admission) = control.admit() else {
                continue;
            };
            let _transition = control.transitions.lock().await;
            let Some(mut run) = self.ports.workflows.load_run(&snapshot.id).await? else {
                continue;
            };
            let previous = run.state;
            if !matches!(
                previous,
                WorkflowRunState::Running | WorkflowRunState::Paused | WorkflowRunState::Ready
            ) || (previous != WorkflowRunState::Running
                && !run
                    .steps
                    .values()
                    .any(|step| step.state == yss_harness_contract::WorkflowStepState::Running))
            {
                continue;
            }
            let definition = self
                .ports
                .workflows
                .load_definition(&run.definition_id, &run.definition_version)
                .await?
                .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
            let compiled = CompiledWorkflow::compile(definition)?;
            WorkflowRuntime::recover_interrupted(&compiled, &mut run, self.ports.clock.now())?;
            if run.state != WorkflowRunState::Failed {
                let current = self.ports.sessions.load_session(&run.session_id).await?;
                if previous == WorkflowRunState::Paused
                    || current.as_ref().is_none_or(|session| {
                        session.state != HarnessSessionState::Active
                            || session.project != run.project
                    })
                {
                    run.state = WorkflowRunState::Paused;
                }
            }
            self.persist_workflow(&mut run).await?;
            recovered += 1;
        }
        Ok(recovered)
    }

    pub(super) fn workflow_control(&self, id: &WorkflowRunId) -> Arc<WorkflowControl> {
        let mut controls = self
            .workflow_controls
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        controls.retain(|_, control| control.strong_count() > 0);
        if let Some(control) = controls.get(id).and_then(Weak::upgrade) {
            return control;
        }
        let control = Arc::new(WorkflowControl::default());
        controls.insert(id.clone(), Arc::downgrade(&control));
        control
    }

    pub(super) async fn persist_workflow(
        &self,
        run: &mut WorkflowRunRecord,
    ) -> Result<(), HarnessError> {
        *run = self
            .ports
            .workflows
            .save_run(run, Some(run.revision))
            .await?;
        Ok(())
    }
}

fn workflow_failure_is_retriable(code: CapabilityFailureCode) -> bool {
    matches!(
        code,
        CapabilityFailureCode::ProjectSessionUnavailable
            | CapabilityFailureCode::ProjectSessionChanged
            | CapabilityFailureCode::GraphUnavailable
            | CapabilityFailureCode::DatabaseUnavailable
            | CapabilityFailureCode::CatalogUnavailable
            | CapabilityFailureCode::ResultUnavailable
            | CapabilityFailureCode::DeadlineElapsed
            | CapabilityFailureCode::PersistenceUnavailable
            | CapabilityFailureCode::InternalFailure
    )
}

#[derive(Default)]
pub(super) struct WorkflowControl {
    transitions: tokio::sync::Mutex<()>,
    active: Mutex<Option<CancellationToken>>,
}

impl WorkflowControl {
    fn admit(self: &Arc<Self>) -> Result<WorkflowAdmission, HarnessError> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.is_some() {
            return Err(HarnessError::ConcurrentWorkflow);
        }
        let cancellation = CancellationToken::default();
        *active = Some(cancellation.clone());
        Ok(WorkflowAdmission {
            control: Arc::clone(self),
            cancellation,
        })
    }

    fn cancel(&self) {
        let token = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(token) = token {
            token.cancel(CancellationReason::User);
        }
    }
}

struct WorkflowAdmission {
    control: Arc<WorkflowControl>,
    cancellation: CancellationToken,
}

impl Drop for WorkflowAdmission {
    fn drop(&mut self) {
        self.cancellation.cancel(CancellationReason::User);
        self.control
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}
