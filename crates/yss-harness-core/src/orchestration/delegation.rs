//! Bind model task intentions to read observations and the existing internal authority.

use super::*;
use yss_harness_contract::model::AgentTaskInput;

impl RunExecutor {
    pub(super) async fn delegate_input(
        &self,
        input: AgentTaskInput,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| rejected("workers_cannot_delegate"))?;
        let digest = yss_canonical_hash::hash_canonical("yssbi.harness.task", &input)
            .map_err(|_| persistence_failure())?;
        let key = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let existing = manager
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .map(|entry| entry.task.clone());
        if let Some(existing) = existing {
            if AgentTaskInput::from(&existing) != input {
                return Err(rejected("task_key_conflict"));
            }
            return manager.delegate(existing).await;
        }
        let mut resources = Vec::with_capacity(input.scope.resources.len());
        for resource in input.scope.resources {
            let version = self
                .task_resource_version(&resource.resource, true)
                .await?
                .ok_or_else(persistence_failure)?;
            resources.push(AgentResourceAccess {
                resource: resource.resource,
                version: Some(version),
                operations: resource.operations,
            });
        }
        let outcome = manager
            .delegate(AgentTask {
                key,
                worker: input.worker,
                objective: input.objective,
                constraints: input.constraints,
                completion_criteria: input.completion_criteria,
                depends_on: input.depends_on,
                scope: AgentTaskScope {
                    resources,
                    results: input.scope.results,
                    creations: input.scope.creations,
                    export_paths: input.scope.export_paths,
                },
            })
            .await?;
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .invalidate(&outcome.artifacts);
        Ok(outcome)
    }

    pub(super) async fn task_resource_version(
        &self,
        resource: &ProjectResourceRef,
        capture_unobserved: bool,
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
        let outcome = self
            .execute_tool(AutomationCapabilityRequest::InspectResource(
                InspectResourceRequest {
                    resource: resource.clone(),
                },
            ))
            .await?;
        let AutomationCapabilityResult::ResourceInspection(value) = outcome.result else {
            return Err(persistence_failure());
        };
        if value.resource != *resource {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ));
        }
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bind(&value)
            .map(Some)
    }
}
