use super::{DatabaseDeclDTO, VariableInstanceDTO};
use crate::project::project_writers::ProjectSaveResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSaveResultDto {
    pub project_instance_id: String,
    pub operation_id: yss_project_identity::OperationId,
    pub publication_revision: u64,
    pub affected_resources: Vec<crate::project::ResourceKey>,
    pub index_invalidated: bool,
    pub history: crate::project::HistoryStatusDto,
}

impl From<ProjectSaveResult> for ProjectSaveResultDto {
    fn from(value: ProjectSaveResult) -> Self {
        let (
            project_instance_id,
            operation_id,
            publication_revision,
            affected_resources,
            index_invalidated,
            history,
        ) = value.into_parts();
        Self {
            project_instance_id: project_instance_id.to_string(),
            operation_id,
            publication_revision,
            affected_resources: affected_resources.into_vec(),
            index_invalidated,
            history: crate::project::HistoryStatusDto {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            },
        }
    }
}

/// 分阶段加载：databases + variables（第一步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabasesVariablesDTO {
    pub databases: HashMap<String, DatabaseDeclDTO>,
    pub variables: HashMap<String, VariableInstanceDTO>,
}
