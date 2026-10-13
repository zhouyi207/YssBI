use crate::contract::*;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(crate) struct Evidence {
    pub(crate) invocations: Vec<ToolInvocationId>,
    pub(super) artifacts: Vec<ResourceChange>,
    pub(super) results: Vec<GraphResultReference>,
    pub(super) plan: Option<StatisticalPlan>,
    pub(crate) blocked_reason: Option<String>,
    requires_saved_artifact: bool,
    artifact_saves: BTreeMap<ProjectResourceRef, bool>,
}

impl Evidence {
    pub(crate) fn delivery_pending(&self) -> bool {
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
    pub(super) fn for_task(role: AgentRole, scope: &AgentTaskScope) -> Self {
        Self {
            requires_saved_artifact: role == AgentRole::Report
                && (scope.creations.iter().any(|grant| {
                    matches!(
                        grant.specification,
                        ResourceCreation::Doc { .. } | ResourceCreation::Mind { .. }
                    )
                }) || scope.resources.iter().any(|access| {
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
