//! Bind model task intentions through the root run's hooks and deduplicate them within the turn.

use super::*;

impl RunExecutor {
    pub(super) async fn delegate_input(
        &self,
        manager: &Arc<TurnOrchestrator>,
        input: AgentTaskInput,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let digest = yss_canonical_hash::hash_canonical("yssbi.harness.task", &input)
            .map_err(|_| persistence_failure())?;
        let key = hex::encode(digest);
        let existing = manager
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .map(|entry| entry.task.clone());
        if let Some(existing) = existing {
            if task_input(self.extension.as_ref(), &existing) != input {
                return Err(rejected("task_key_conflict"));
            }
            return manager.delegate(existing).await;
        }
        let scope = self.hooks.bind_task(input.clone(), self).await?;
        manager
            .delegate(AgentTask {
                key,
                worker: input.worker,
                objective: input.objective,
                constraints: input.constraints,
                completion_criteria: input.completion_criteria,
                depends_on: input.depends_on,
                scope,
            })
            .await
    }
}
