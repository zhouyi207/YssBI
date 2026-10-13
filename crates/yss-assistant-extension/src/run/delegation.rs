//! Bind delegated and resumed tasks to the root run's read observations.

use super::*;

impl AssistantRun {
    pub(super) async fn bind_task_scope(
        &self,
        input: AgentTaskInput,
        runner: &dyn ToolRunner,
    ) -> Result<Value, CapabilityFailure> {
        let scope: model::AgentTaskScopeInput =
            serde_json::from_value(input.scope).map_err(|_| rejected("invalid field 'scope'"))?;
        let mut resources = Vec::with_capacity(scope.resources.len());
        for resource in scope.resources {
            let version = self
                .task_resource_version(&resource.resource, true, runner)
                .await?
                .ok_or_else(persistence_failure)?;
            resources.push(AgentResourceAccess {
                resource: resource.resource,
                version: Some(version),
                operations: resource.operations,
            });
        }
        serde_json::to_value(AgentTaskScope {
            resources,
            results: scope.results,
            creations: scope.creations,
            export_paths: scope.export_paths,
        })
        .map_err(|_| internal_failure())
    }

    /// Rebind a resumed worker's granted resources to fresh root observations or its own receipts.
    pub(super) async fn rebind_worker(
        &self,
        worker: &dyn AgentRunHooks,
        runner: &dyn ToolRunner,
    ) -> Result<(Value, Value), CapabilityFailure> {
        let worker = worker
            .as_any()
            .downcast_ref::<AssistantRun>()
            .ok_or_else(internal_failure)?;
        let mut scope = worker
            .scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task
            .clone()
            .ok_or_else(persistence_failure)?;
        let mut resource_versions = Vec::new();
        for access in &mut scope.resources {
            if let Some(version) = self
                .task_resource_version(&access.resource, false, runner)
                .await?
            {
                worker
                    .observations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .refresh_from(
                        &self.observations.lock().unwrap_or_else(|e| e.into_inner()),
                        &access.resource,
                    );
                access.version = Some(version.clone());
                resource_versions.push(AgentResourceVersion {
                    resource: access.resource.clone(),
                    version,
                });
            } else if access.version.is_some()
                && worker
                    .observations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .version(&access.resource, true)?
                    != access.version
            {
                return Err(
                    CapabilityFailure::new(CapabilityFailureCode::REVISION_CONFLICT)
                        .with_detail("reason", "resource_requires_current_read")
                        .with_detail("resourceId", &access.resource.id),
                );
            }
        }
        worker
            .scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task = Some(scope.clone());
        Ok((
            serde_json::to_value(scope).map_err(|_| internal_failure())?,
            serde_json::to_value(AgentFollowupBindings { resource_versions })
                .map_err(|_| internal_failure())?,
        ))
    }

    /// A worker starts only when every granted input still has its delegated version.
    pub(super) async fn check_inputs(
        &self,
        runner: &dyn ToolRunner,
    ) -> Result<Option<String>, AgentDriverFailure> {
        let resources = self
            .scope
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .task
            .as_ref()
            .map(|task| task.resources.clone())
            .unwrap_or_default();
        for access in resources {
            let Some(version) = &access.version else {
                continue;
            };
            let outcome = self
                .execute_internal(
                    runner,
                    AutomationCapabilityRequest::InspectResource(InspectResourceRequest {
                        resource: access.resource.clone(),
                    }),
                )
                .await;
            let result = match outcome {
                Ok((_, result)) => result,
                Err(failure) => {
                    if failure.code == CapabilityFailureCode::CANCELLED {
                        return Err(AgentDriverFailure::new(AgentDriverFailureCode::Cancelled));
                    }
                    self.evidence
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .blocked_reason = Some(
                        if failure.code == CapabilityFailureCode::REVISION_CONFLICT {
                            "input_changed"
                        } else {
                            "input_inspection_failed"
                        }
                        .into(),
                    );
                    return Ok(Some(
                        serde_json::json!({
                            "reason": "input_inspection_failed", "resource": access.resource,
                            "failure": model::failure(&failure),
                            "nextStep": "Inspect the current resource and correct the task before retrying."
                        })
                        .to_string(),
                    ));
                }
            };
            if !matches!(result, AutomationCapabilityResult::ResourceInspection(ref value) if crate::agents::resource_version_matches(access.resource.kind, version, &value.version))
            {
                self.evidence
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .blocked_reason = Some("input_changed".into());
                return Ok(Some(
                    serde_json::json!({
                        "reason": "input_changed", "resource": access.resource,
                        "nextStep": "Inspect the changed resource, reassess the task, then resume the worker."
                    })
                    .to_string(),
                ));
            }
        }
        Ok(None)
    }

    pub(super) async fn task_resource_version(
        &self,
        resource: &ProjectResourceRef,
        capture_unobserved: bool,
        runner: &dyn ToolRunner,
    ) -> Result<Option<ResourceVersion>, CapabilityFailure> {
        let needs_database_binding = {
            let observations = self.observations.lock().unwrap_or_else(|e| e.into_inner());
            match observations.version(resource, !capture_unobserved) {
                Ok(Some(version)) => return Ok(Some(version)),
                Err(failure) => return Err(failure),
                Ok(None) => {}
            }
            observations.needs_database_binding(resource)
        };
        if !capture_unobserved && !needs_database_binding {
            return Ok(None);
        }
        let (_, result) = self
            .execute_internal(
                runner,
                AutomationCapabilityRequest::InspectResource(InspectResourceRequest {
                    resource: resource.clone(),
                }),
            )
            .await?;
        let AutomationCapabilityResult::ResourceInspection(value) = result else {
            return Err(persistence_failure());
        };
        if value.resource != *resource {
            return Err(internal_failure());
        }
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bind(&value)
            .map(Some)
    }
}
