use super::*;
use crate::harness::knowledge::ProjectKnowledgeError;
use yss_harness_contract::{KnowledgeSourceId, ProjectKnowledgeSourceSummary};
use yss_project_identity::ProjectInstanceId;

fn project(id: String) -> Result<ProjectInstanceId, CommandError> {
    if id.trim().is_empty() {
        return Err(CommandError::expected("invalid_harness_request"));
    }
    Ok(ProjectInstanceId::from_existing(id))
}

#[tauri::command]
pub async fn list_harness_knowledge(
    runtime: State<'_, HarnessRuntimeState>,
    project_instance_id: String,
) -> Result<Vec<ProjectKnowledgeSourceSummary>, CommandError> {
    runtime
        .knowledge
        .list(&project(project_instance_id)?)
        .await
        .map_err(map_knowledge_error)
}

#[tauri::command]
pub async fn rebuild_harness_knowledge(
    runtime: State<'_, HarnessRuntimeState>,
    project_instance_id: String,
    path: String,
) -> Result<(), CommandError> {
    runtime
        .knowledge
        .rebuild(&project(project_instance_id)?, &path)
        .await
        .map_err(map_knowledge_error)
}

#[tauri::command]
pub async fn remove_harness_knowledge(
    runtime: State<'_, HarnessRuntimeState>,
    project_instance_id: String,
    source_id: String,
) -> Result<(), CommandError> {
    let id = KnowledgeSourceId::try_new(source_id)
        .map_err(|_| CommandError::expected("invalid_harness_request"))?;
    runtime
        .knowledge
        .remove(&project(project_instance_id)?, &id)
        .await
        .map_err(map_knowledge_error)
}

pub(super) fn map_knowledge_error(error: ProjectKnowledgeError) -> CommandError {
    match error {
        ProjectKnowledgeError::ProjectChanged => CommandError::expected("project_session_changed"),
        ProjectKnowledgeError::DocumentUnavailable => {
            CommandError::expected("harness_knowledge_document_unavailable")
        }
        ProjectKnowledgeError::Persistence(_) => {
            CommandError::internal("harness_knowledge_persistence")
        }
    }
}
