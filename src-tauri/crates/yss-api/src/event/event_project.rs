use crate::schema::application_event::{
    LifecycleMutationResultDto, ProjectActivationResultDto, ResourceMutationResultDto,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventProject {
    #[serde(rename_all = "camelCase")]
    ProjectLoaded {
        result: ProjectActivationResultDto,
    },
    ProjectCleared,
    #[serde(rename_all = "camelCase")]
    ProjectLifecycleCommitted {
        result: LifecycleMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted {
        result: ResourceMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        result: crate::schema::ProjectSaveResultDto,
    },
}
