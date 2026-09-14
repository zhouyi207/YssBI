use super::DatabaseDeclDTO;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use yss_ipc_contract::project::ProjectSaveResultDto;
use yss_project::project_writers::ProjectSaveResult;

pub(crate) fn project_save_to_transport(value: ProjectSaveResult) -> ProjectSaveResultDto {
    let (
        project_instance_id,
        operation_id,
        publication_revision,
        affected_resources,
        index_invalidated,
    ) = value.into_parts();
    ProjectSaveResultDto {
        project_instance_id: project_instance_id.to_string(),
        operation_id,
        publication_revision,
        affected_resources: affected_resources.into_vec(),
        index_invalidated,
    }
}

/// 分阶段加载：databases（第一步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDatabasesDTO {
    pub databases: HashMap<String, DatabaseDeclDTO>,
}
