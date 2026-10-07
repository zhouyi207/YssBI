use std::{collections::BTreeMap, sync::Mutex};
use yss_harness_contract::*;

#[derive(Clone, Default)]
pub(super) struct Evidence {
    pub(super) invocations: Vec<ToolInvocationId>,
    pub(super) artifacts: Vec<ResourceChange>,
    pub(super) results: Vec<GraphResultReference>,
    pub(super) plan: Option<StatisticalPlan>,
    pub(super) blocked_reason: Option<String>,
    requires_saved_artifact: bool,
    artifact_saves: BTreeMap<ProjectResourceRef, bool>,
}

impl Evidence {
    pub(super) fn delivery_pending(&self) -> bool {
        self.requires_saved_artifact
            && (self.artifact_saves.is_empty() || self.artifact_saves.values().any(|saved| !saved))
    }

    pub(super) fn delivery_feedback(&self) -> Option<String> {
        self.delivery_pending()
            .then(|| serde_json::json!({
                "reason": "report_resource_not_saved",
                "artifactCount": self.artifacts.len(),
                "nextStep": "Continue from committed receipts. Create or edit the authorized Doc or Mind and save after the final edit. Do not repeat successful writes. If blocked, state the concrete missing requirement."
            }).to_string())
    }
    pub(super) fn for_task(task: &AgentTask) -> Self {
        Self {
            requires_saved_artifact: task.worker == AgentRole::Report
                && (task.scope.creations.iter().any(|grant| {
                    matches!(
                        grant.specification,
                        ResourceCreation::Doc { .. } | ResourceCreation::Mind { .. }
                    )
                }) || task.scope.resources.iter().any(|access| {
                    matches!(
                        access.resource.kind,
                        ProjectResourceKind::Doc | ProjectResourceKind::Mind
                    ) && access.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            AgentResourceOperation::Edit | AgentResourceOperation::Save
                        )
                    })
                })),
            ..Self::default()
        }
    }
}

pub(super) fn record_receipt(
    request: &AutomationCapabilityRequest,
    result: &AutomationCapabilityResult,
    evidence: &mut Evidence,
    scope: &mut AgentInvocationScope,
) {
    let Some(task) = &mut scope.task else {
        return;
    };
    match result {
        AutomationCapabilityResult::ChartInspection(inspection) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource == inspection.chart.resource())
                && access.version.is_none()
            {
                access.version = Some(inspection.version.clone());
            }
        }
        AutomationCapabilityResult::DocumentRead(inspection) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource == inspection.document.resource())
                && access.version.is_none()
            {
                access.version = Some(inspection.version.clone());
            }
        }
        AutomationCapabilityResult::MindRead(inspection) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource == inspection.mind.resource())
                && access.version.is_none()
            {
                access.version = Some(inspection.version.clone());
            }
        }
        AutomationCapabilityResult::DatabaseRead(inspection) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource == inspection.database.resource())
                && access.version.is_none()
            {
                access.version = Some(inspection.version.clone());
            }
        }
        AutomationCapabilityResult::ResourceInspection(inspection) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource == inspection.resource)
                && access.version.is_none()
            {
                access.version = Some(inspection.version.clone());
            }
        }
        AutomationCapabilityResult::ResourceManaged(receipt)
        | AutomationCapabilityResult::ResourceEdited(receipt) => {
            evidence.artifacts.extend(receipt.changes.clone());
            for change in &receipt.changes {
                if !matches!(
                    change.resource.kind,
                    ProjectResourceKind::Doc | ProjectResourceKind::Mind
                ) {
                    continue;
                }
                if change.deleted {
                    evidence.artifact_saves.remove(&change.resource);
                } else {
                    // Only the matching successful Save receipt proves delivery.
                    // A later edit or rename requires another save before completion.
                    let saved = matches!(
                        (request, result),
                        (
                            AutomationCapabilityRequest::ManageResource(
                                ManageResourceRequest::Save { resource, .. }
                            ),
                            AutomationCapabilityResult::ResourceManaged(_)
                        ) if resource == &change.resource
                            && change.revision_kind == ResourceRevisionKind::Resource
                    );
                    evidence
                        .artifact_saves
                        .insert(change.resource.clone(), saved);
                }
            }
            for moved in &receipt.moves {
                if let Some(access) = task
                    .resources
                    .iter_mut()
                    .find(|access| access.resource == moved.from)
                {
                    access.resource = moved.to.clone();
                    access.version = None;
                }
            }
            for change in &receipt.changes {
                if change.deleted {
                    task.resources
                        .retain(|access| access.resource != change.resource);
                    continue;
                }
                if let Some(access) = task
                    .resources
                    .iter_mut()
                    .find(|access| access.resource == change.resource)
                {
                    if change.revision_kind == ResourceRevisionKind::Resource
                        && let Some(version) = &mut access.version
                    {
                        version.revision = change.revision;
                    }
                } else if matches!(
                    request,
                    AutomationCapabilityRequest::ManageResource(
                        ManageResourceRequest::Create { .. }
                            | ManageResourceRequest::Duplicate { .. }
                    )
                ) {
                    let operations = match request {
                        AutomationCapabilityRequest::ManageResource(
                            ManageResourceRequest::Create { specification },
                        ) => task
                            .creations
                            .iter()
                            .find(|grant| &grant.specification == specification)
                            .map(|grant| grant.operations.clone()),
                        AutomationCapabilityRequest::ManageResource(
                            ManageResourceRequest::Duplicate { resource, .. },
                        ) => task
                            .resources
                            .iter()
                            .find(|grant| &grant.resource == resource)
                            .map(|grant| grant.operations.clone()),
                        _ => None,
                    }
                    .unwrap_or_default();
                    task.resources.push(AgentResourceAccess {
                        resource: change.resource.clone(),
                        version: None,
                        operations,
                    });
                }
            }
            for state in &receipt.resources {
                if let Some(access) = task
                    .resources
                    .iter_mut()
                    .find(|access| access.resource == state.resource)
                {
                    access.version = Some(state.version.clone());
                }
            }
        }
        AutomationCapabilityResult::GraphExecution(value) => {
            add_results(task, evidence, &value.results)
        }
        AutomationCapabilityResult::GraphResults(value) => {
            add_results(task, evidence, &value.results)
        }
        AutomationCapabilityResult::GraphEditReceipt(value) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource.id == value.graph_path)
            {
                evidence.artifacts.push(ResourceChange {
                    resource: access.resource.clone(),
                    revision: value.to_revision,
                    revision_kind: ResourceRevisionKind::Resource,
                    deleted: false,
                });
                if let Some(version) = &mut access.version {
                    version.revision = value.to_revision;
                }
            }
        }
        AutomationCapabilityResult::GraphSaved(value) => {
            if let Some(access) = task
                .resources
                .iter_mut()
                .find(|access| access.resource.id == value.graph_path)
            {
                evidence.artifacts.push(ResourceChange {
                    resource: access.resource.clone(),
                    revision: value.resource_revision,
                    revision_kind: ResourceRevisionKind::Resource,
                    deleted: false,
                });
                if let Some(version) = &mut access.version {
                    version.revision = value.resource_revision;
                }
            }
        }
        _ => {}
    }
}

fn add_results(
    task: &mut AgentTaskScope,
    evidence: &mut Evidence,
    results: &[GraphResultReference],
) {
    for result in results {
        let access = AgentResultAccess {
            result_ref: result.result_ref.clone(),
        };
        if !task.results.contains(&access) {
            task.results.push(access);
        }
        if !evidence.results.contains(result) {
            evidence.results.push(result.clone());
        }
    }
}

pub(super) fn finish_outcome(
    run_id: AgentRunId,
    role: AgentRole,
    result: &Result<AgentTurnResult, AgentDriverFailure>,
    evidence: &Mutex<Evidence>,
) -> AgentTaskOutcome {
    let evidence = evidence.lock().unwrap_or_else(|e| e.into_inner());
    // The public reply is already owned by the turn transcript; only Workers produce reports.
    let mut report = result.as_ref().ok().and_then(|result| {
        (role != AgentRole::Manager).then(|| WorkerReport {
            summary: result.final_text.clone(),
            warnings: vec![],
            blocked_reason: evidence.blocked_reason.clone(),
            next_steps: vec![],
        })
    });
    if evidence.delivery_pending()
        && let Some(report) = &mut report
        && report.blocked_reason.is_none()
    {
        report.blocked_reason = Some("report_resource_not_saved".into());
        report.next_steps.push("Use followup_task to continue this worker from committed receipts. Use the authorized document or topic editing tools and finish with save_resource. A Mind-only task does not require a Doc.".into());
    }
    let failure_code = result.as_ref().err().map(|error| error.code);
    let state = match failure_code {
        Some(AgentDriverFailureCode::Cancelled) => AgentRunState::Cancelled,
        Some(_) => AgentRunState::Failed,
        None if report
            .as_ref()
            .is_some_and(|report| report.blocked_reason.is_some()) =>
        {
            AgentRunState::Blocked
        }
        None => AgentRunState::Completed,
    };
    AgentTaskOutcome {
        run_id,
        role,
        state,
        report,
        failure_code,
        artifacts: evidence.artifacts.clone(),
        results: evidence.results.clone(),
        evidence: evidence.invocations.clone(),
        plan: evidence.plan.clone(),
        invalidated_runs: vec![],
    }
}
