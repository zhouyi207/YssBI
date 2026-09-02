use std::collections::{BTreeMap, BTreeSet};

use yss_automation_contract::{
    AutomationCapabilityRequest, HarnessSessionId, HarnessTurnId, InspectDatasetProfileRequest,
    InspectDatasetSchemaRequest, ProjectSessionBinding, UnixMillis, WorkflowDefinition, WorkflowId,
    WorkflowRunId, WorkflowRunRecord, WorkflowRunState, WorkflowStep, WorkflowStepId,
    WorkflowStepKind, WorkflowStepRecord, WorkflowStepState, WorkflowVersion,
};

#[derive(Clone, Debug)]
pub struct CompiledWorkflow {
    definition: WorkflowDefinition,
}

impl CompiledWorkflow {
    pub fn compile(definition: WorkflowDefinition) -> Result<Self, WorkflowCompileError> {
        if definition.steps.is_empty() {
            return Err(WorkflowCompileError::Empty);
        }
        let step_ids = definition
            .steps
            .iter()
            .map(|step| step.id.clone())
            .collect::<BTreeSet<_>>();
        if step_ids.len() != definition.steps.len() {
            return Err(WorkflowCompileError::DuplicateStep);
        }
        for step in &definition.steps {
            if step
                .depends_on
                .iter()
                .any(|dependency| dependency == &step.id)
            {
                return Err(WorkflowCompileError::SelfDependency);
            }
            if step
                .depends_on
                .iter()
                .any(|dependency| !step_ids.contains(dependency))
            {
                return Err(WorkflowCompileError::UnknownDependency);
            }
            if let WorkflowStepKind::Capability(request) = &step.kind {
                request
                    .validate()
                    .map_err(|_| WorkflowCompileError::InvalidCapabilityRequest)?;
            }
        }
        ensure_acyclic(&definition.steps)?;
        Ok(Self { definition })
    }

    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }
}

pub fn dataset_quality_review_workflow(
    database_id: impl Into<String>,
) -> Result<CompiledWorkflow, WorkflowCompileError> {
    let database_id = database_id.into();
    let schema_step = WorkflowStepId::try_new("inspect_dataset_schema")
        .map_err(|_| WorkflowCompileError::InvalidIdentity)?;
    let definition = WorkflowDefinition {
        id: WorkflowId::try_new("dataset_quality_review")
            .map_err(|_| WorkflowCompileError::InvalidIdentity)?,
        version: WorkflowVersion::try_new("1.0.0")
            .map_err(|_| WorkflowCompileError::InvalidIdentity)?,
        steps: vec![
            WorkflowStep {
                id: schema_step.clone(),
                depends_on: Vec::new(),
                kind: WorkflowStepKind::Capability(
                    AutomationCapabilityRequest::InspectDatasetSchema(
                        InspectDatasetSchemaRequest {
                            database_id: database_id.clone(),
                        },
                    ),
                ),
            },
            WorkflowStep {
                id: WorkflowStepId::try_new("inspect_dataset_profile")
                    .map_err(|_| WorkflowCompileError::InvalidIdentity)?,
                depends_on: vec![schema_step],
                kind: WorkflowStepKind::Capability(
                    AutomationCapabilityRequest::InspectDatasetProfile(
                        InspectDatasetProfileRequest { database_id },
                    ),
                ),
            },
        ],
    };
    CompiledWorkflow::compile(definition)
}

fn ensure_acyclic(steps: &[WorkflowStep]) -> Result<(), WorkflowCompileError> {
    let mut remaining_dependencies = steps
        .iter()
        .map(|step| (step.id.clone(), step.depends_on.iter().cloned().collect()))
        .collect::<BTreeMap<WorkflowStepId, BTreeSet<WorkflowStepId>>>();
    let mut ready = remaining_dependencies
        .iter()
        .filter(|(_, dependencies)| dependencies.is_empty())
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;

    while let Some(step_id) = ready.pop_first() {
        visited += 1;
        for (candidate_id, dependencies) in &mut remaining_dependencies {
            if dependencies.remove(&step_id) && dependencies.is_empty() {
                ready.insert(candidate_id.clone());
            }
        }
    }
    if visited == steps.len() {
        Ok(())
    } else {
        Err(WorkflowCompileError::Cycle)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkflowCompileError {
    #[error("workflow identity is invalid")]
    InvalidIdentity,
    #[error("workflow has no steps")]
    Empty,
    #[error("workflow contains duplicate step ids")]
    DuplicateStep,
    #[error("workflow step depends on itself")]
    SelfDependency,
    #[error("workflow step has an unknown dependency")]
    UnknownDependency,
    #[error("workflow contains a dependency cycle")]
    Cycle,
    #[error("workflow contains an invalid capability request")]
    InvalidCapabilityRequest,
}

pub struct WorkflowRuntime;

impl WorkflowRuntime {
    pub fn plan(
        compiled: &CompiledWorkflow,
        run_id: WorkflowRunId,
        session_id: HarnessSessionId,
        turn_id: Option<HarnessTurnId>,
        project: ProjectSessionBinding,
        now: UnixMillis,
    ) -> WorkflowRunRecord {
        WorkflowRunRecord {
            id: run_id,
            session_id,
            turn_id,
            definition_id: compiled.definition.id.clone(),
            definition_version: compiled.definition.version.clone(),
            project,
            state: WorkflowRunState::Planned,
            steps: compiled
                .definition
                .steps
                .iter()
                .map(|step| {
                    (
                        step.id.clone(),
                        WorkflowStepRecord {
                            state: WorkflowStepState::Pending,
                            attempt: 0,
                        },
                    )
                })
                .collect(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn start(run: &mut WorkflowRunRecord, now: UnixMillis) -> Result<(), WorkflowRuntimeError> {
        match run.state {
            WorkflowRunState::Planned | WorkflowRunState::Ready | WorkflowRunState::Paused => {
                run.state = WorkflowRunState::Running;
                run.updated_at = now;
                Ok(())
            }
            _ => Err(WorkflowRuntimeError::InvalidRunTransition),
        }
    }

    pub fn pause(run: &mut WorkflowRunRecord, now: UnixMillis) -> Result<(), WorkflowRuntimeError> {
        if !matches!(
            run.state,
            WorkflowRunState::Planned | WorkflowRunState::Ready | WorkflowRunState::Running
        ) {
            return Err(WorkflowRuntimeError::InvalidRunTransition);
        }
        run.state = WorkflowRunState::Paused;
        run.updated_at = now;
        Ok(())
    }

    pub fn resume(
        run: &mut WorkflowRunRecord,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        if run.state != WorkflowRunState::Paused {
            return Err(WorkflowRuntimeError::InvalidRunTransition);
        }
        run.state = WorkflowRunState::Ready;
        run.updated_at = now;
        Ok(())
    }

    pub fn cancel(
        run: &mut WorkflowRunRecord,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        if matches!(
            run.state,
            WorkflowRunState::Completed | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            return Err(WorkflowRuntimeError::InvalidRunTransition);
        }
        run.state = WorkflowRunState::Cancelled;
        run.updated_at = now;
        Ok(())
    }

    pub fn ready_steps(
        compiled: &CompiledWorkflow,
        run: &WorkflowRunRecord,
    ) -> Result<Vec<WorkflowStepId>, WorkflowRuntimeError> {
        ensure_run_matches(compiled, run)?;
        if run.state != WorkflowRunState::Running {
            return Ok(Vec::new());
        }
        Ok(compiled
            .definition
            .steps
            .iter()
            .filter(|step| {
                run.steps.get(&step.id).is_some_and(|record| {
                    record.state == WorkflowStepState::Pending
                        && step.depends_on.iter().all(|dependency| {
                            run.steps.get(dependency).is_some_and(|dependency_record| {
                                matches!(
                                    dependency_record.state,
                                    WorkflowStepState::Succeeded | WorkflowStepState::Skipped
                                )
                            })
                        })
                })
            })
            .map(|step| step.id.clone())
            .collect())
    }

    pub fn start_step(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        if !Self::ready_steps(compiled, run)?.contains(step_id) {
            return Err(WorkflowRuntimeError::StepNotReady);
        }
        let record = run
            .steps
            .get_mut(step_id)
            .ok_or(WorkflowRuntimeError::UnknownStep)?;
        record.attempt = record
            .attempt
            .checked_add(1)
            .ok_or(WorkflowRuntimeError::AttemptExhausted)?;
        record.state = WorkflowStepState::Running;
        run.updated_at = now;
        Ok(())
    }

    pub fn wait_for_approval(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        if !Self::ready_steps(compiled, run)?.contains(step_id)
            || !compiled.definition.steps.iter().any(|step| {
                &step.id == step_id && matches!(step.kind, WorkflowStepKind::Approval { .. })
            })
        {
            return Err(WorkflowRuntimeError::StepNotReady);
        }
        run.state = WorkflowRunState::WaitingForApproval;
        run.updated_at = now;
        Ok(())
    }

    pub fn wait_for_external_input(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        if !Self::ready_steps(compiled, run)?.contains(step_id)
            || !compiled.definition.steps.iter().any(|step| {
                &step.id == step_id && matches!(step.kind, WorkflowStepKind::Decision { .. })
            })
        {
            return Err(WorkflowRuntimeError::StepNotReady);
        }
        run.state = WorkflowRunState::WaitingForExternalInput;
        run.updated_at = now;
        Ok(())
    }

    pub fn succeed_step(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        transition_running_step(run, step_id, WorkflowStepState::Succeeded)?;
        run.updated_at = now;
        if run.steps.values().all(|step| {
            matches!(
                step.state,
                WorkflowStepState::Succeeded | WorkflowStepState::Skipped
            )
        }) {
            run.state = WorkflowRunState::Completed;
        } else if Self::ready_steps(compiled, run)?.is_empty()
            && run
                .steps
                .values()
                .all(|step| step.state != WorkflowStepState::Running)
        {
            return Err(WorkflowRuntimeError::NoProgressPossible);
        }
        Ok(())
    }

    pub fn fail_step(
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        retriable: bool,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        transition_running_step(
            run,
            step_id,
            if retriable {
                WorkflowStepState::RetriableFailure
            } else {
                WorkflowStepState::TerminalFailure
            },
        )?;
        run.state = if retriable {
            WorkflowRunState::Paused
        } else {
            WorkflowRunState::Failed
        };
        run.updated_at = now;
        Ok(())
    }

    pub fn recover_interrupted(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        ensure_run_matches(compiled, run)?;
        let mut requires_reconciliation = false;
        for step in &compiled.definition.steps {
            let Some(record) = run.steps.get_mut(&step.id) else {
                return Err(WorkflowRuntimeError::UnknownStep);
            };
            if record.state != WorkflowStepState::Running {
                continue;
            }
            if matches!(step.kind, WorkflowStepKind::Capability(_)) {
                record.state = WorkflowStepState::Pending;
            } else {
                requires_reconciliation = true;
            }
        }
        run.state = if requires_reconciliation {
            WorkflowRunState::Paused
        } else {
            WorkflowRunState::Ready
        };
        run.updated_at = now;
        Ok(())
    }
}

fn ensure_run_matches(
    compiled: &CompiledWorkflow,
    run: &WorkflowRunRecord,
) -> Result<(), WorkflowRuntimeError> {
    if run.definition_id != compiled.definition.id
        || run.definition_version != compiled.definition.version
        || run.steps.len() != compiled.definition.steps.len()
    {
        return Err(WorkflowRuntimeError::DefinitionMismatch);
    }
    Ok(())
}

fn transition_running_step(
    run: &mut WorkflowRunRecord,
    step_id: &WorkflowStepId,
    next: WorkflowStepState,
) -> Result<(), WorkflowRuntimeError> {
    let record = run
        .steps
        .get_mut(step_id)
        .ok_or(WorkflowRuntimeError::UnknownStep)?;
    if record.state != WorkflowStepState::Running {
        return Err(WorkflowRuntimeError::InvalidStepTransition);
    }
    record.state = next;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkflowRuntimeError {
    #[error("workflow run does not match its immutable definition")]
    DefinitionMismatch,
    #[error("workflow run transition is invalid")]
    InvalidRunTransition,
    #[error("workflow step transition is invalid")]
    InvalidStepTransition,
    #[error("workflow step is unknown")]
    UnknownStep,
    #[error("workflow step is not ready")]
    StepNotReady,
    #[error("workflow step attempt count is exhausted")]
    AttemptExhausted,
    #[error("workflow cannot make progress")]
    NoProgressPossible,
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_automation_contract::{
        AutomationCapabilityRequest, InspectGraphRequest, WorkflowId, WorkflowStepKind,
        WorkflowVersion,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    fn step(id: &str, dependencies: &[&str]) -> WorkflowStep {
        WorkflowStep {
            id: WorkflowStepId::try_new(id).unwrap(),
            depends_on: dependencies
                .iter()
                .map(|dependency| WorkflowStepId::try_new(*dependency).unwrap())
                .collect(),
            kind: WorkflowStepKind::Capability(AutomationCapabilityRequest::InspectGraph(
                InspectGraphRequest {
                    graph_path: "events/Main.yssbi-event".to_owned(),
                },
            )),
        }
    }

    fn definition(steps: Vec<WorkflowStep>) -> WorkflowDefinition {
        WorkflowDefinition {
            id: WorkflowId::try_new("dataset_quality_review").unwrap(),
            version: WorkflowVersion::try_new("1.0.0").unwrap(),
            steps,
        }
    }

    #[test]
    fn compiler_rejects_cycles() {
        let error = CompiledWorkflow::compile(definition(vec![
            step("inspect", &["review"]),
            step("review", &["inspect"]),
        ]))
        .unwrap_err();

        assert_eq!(error, WorkflowCompileError::Cycle);
    }

    #[test]
    fn runtime_orders_dependencies_and_recovers_read_only_interruption() {
        let compiled = CompiledWorkflow::compile(definition(vec![
            step("inspect", &[]),
            step("review", &["inspect"]),
        ]))
        .unwrap();
        let mut run = WorkflowRuntime::plan(
            &compiled,
            WorkflowRunId::try_new("run-1").unwrap(),
            HarnessSessionId::try_new("session-1").unwrap(),
            None,
            ProjectSessionBinding::new(
                ProjectInstanceId::from_existing("project-1".into()),
                ProjectSessionId::new("project-session-1"),
            ),
            UnixMillis::from_existing(10),
        );
        WorkflowRuntime::start(&mut run, UnixMillis::from_existing(11)).unwrap();
        let inspect = WorkflowStepId::try_new("inspect").unwrap();
        let review = WorkflowStepId::try_new("review").unwrap();
        assert_eq!(
            WorkflowRuntime::ready_steps(&compiled, &run).unwrap(),
            std::slice::from_ref(&inspect)
        );

        WorkflowRuntime::start_step(&compiled, &mut run, &inspect, UnixMillis::from_existing(12))
            .unwrap();
        WorkflowRuntime::succeed_step(&compiled, &mut run, &inspect, UnixMillis::from_existing(13))
            .unwrap();
        WorkflowRuntime::start_step(&compiled, &mut run, &review, UnixMillis::from_existing(14))
            .unwrap();
        WorkflowRuntime::recover_interrupted(&compiled, &mut run, UnixMillis::from_existing(15))
            .unwrap();

        assert_eq!(run.state, WorkflowRunState::Ready);
        assert_eq!(run.steps[&review].state, WorkflowStepState::Pending);
        assert_eq!(run.steps[&review].attempt, 1);
    }
}
