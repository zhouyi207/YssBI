use crate::project::{
    LifecycleMutationResultDto, ProjectActivationResultDto, ResourceMutationResultDto,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded { result: ProjectActivationResultDto },
    #[serde(rename_all = "camelCase")]
    ProjectCleared { project_instance_id: String },
    #[serde(rename_all = "camelCase")]
    ProjectLifecycleCommitted { result: LifecycleMutationResultDto },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted { result: ResourceMutationResultDto },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        result: crate::project::ProjectSaveResultDto,
    },
}
