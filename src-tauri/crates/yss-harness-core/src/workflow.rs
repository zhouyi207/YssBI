use std::collections::{BTreeMap, BTreeSet};

use yss_harness_contract::{
    HarnessSessionId, HarnessTurnId, ProjectSessionBinding, ToolEffect, UnixMillis,
    WorkflowDefinition, WorkflowRunId, WorkflowRunRecord, WorkflowRunState, WorkflowStep,
    WorkflowStepId, WorkflowStepRecord, WorkflowStepState,
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
            step.request
                .validate()
                .map_err(|_| WorkflowCompileError::InvalidCapabilityRequest)?;
        }
        ensure_acyclic(&definition.steps)?;
        Ok(Self { definition })
    }

    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }
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
            revision: 0,
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
            WorkflowRunState::Planned | WorkflowRunState::Ready => {
                for step in run.steps.values_mut() {
                    if step.state == WorkflowStepState::RetriableFailure {
                        step.state = WorkflowStepState::Pending;
                    }
                }
                run.state = if run
                    .steps
                    .values()
                    .any(|step| step.state == WorkflowStepState::TerminalFailure)
                {
                    WorkflowRunState::Failed
                } else if run
                    .steps
                    .values()
                    .all(|step| step.state == WorkflowStepState::Succeeded)
                {
                    WorkflowRunState::Completed
                } else {
                    WorkflowRunState::Running
                };
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
                                dependency_record.state == WorkflowStepState::Succeeded
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

    pub fn succeed_step(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        step_id: &WorkflowStepId,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        transition_running_step(run, step_id, WorkflowStepState::Succeeded)?;
        run.updated_at = now;
        // Pausing stops dispatch, not settlement of the already admitted step.
        // Resume/advance decides whether the workflow can proceed or finish.
        if run.state != WorkflowRunState::Running {
            return Ok(());
        }
        if run
            .steps
            .values()
            .all(|step| step.state == WorkflowStepState::Succeeded)
        {
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
        if run.state == WorkflowRunState::Running {
            run.state = if retriable {
                WorkflowRunState::Paused
            } else {
                WorkflowRunState::Failed
            };
        }
        run.updated_at = now;
        Ok(())
    }

    pub fn recover_interrupted(
        compiled: &CompiledWorkflow,
        run: &mut WorkflowRunRecord,
        now: UnixMillis,
    ) -> Result<(), WorkflowRuntimeError> {
        ensure_run_matches(compiled, run)?;
        for step in &compiled.definition.steps {
            let Some(record) = run.steps.get_mut(&step.id) else {
                return Err(WorkflowRuntimeError::UnknownStep);
            };
            if record.state != WorkflowStepState::Running {
                continue;
            }
            record.state =
                if step.request.capability_id().descriptor().effect == ToolEffect::Inspect {
                    WorkflowStepState::Pending
                } else {
                    // The operation may already have committed; only its ledger can
                    // establish the outcome. A new attempt must not repeat the effect.
                    WorkflowStepState::TerminalFailure
                };
        }
        run.state = if run
            .steps
            .values()
            .any(|step| step.state == WorkflowStepState::TerminalFailure)
        {
            WorkflowRunState::Failed
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
    if !matches!(
        run.state,
        WorkflowRunState::Running | WorkflowRunState::Paused | WorkflowRunState::Ready
    ) {
        return Err(WorkflowRuntimeError::InvalidRunTransition);
    }
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
    use yss_harness_contract::{
        AutomationCapabilityRequest, InspectGraphRequest, WorkflowId, WorkflowVersion,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    fn step(id: &str, dependencies: &[&str]) -> WorkflowStep {
        WorkflowStep {
            id: WorkflowStepId::try_new(id).unwrap(),
            depends_on: dependencies
                .iter()
                .map(|dependency| WorkflowStepId::try_new(*dependency).unwrap())
                .collect(),
            request: AutomationCapabilityRequest::InspectGraph(InspectGraphRequest {
                graph_path: "events/Main.yssbi-event".to_owned(),
            }),
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
