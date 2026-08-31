use crate::schema::application_event::{
    LifecycleMutationResultDto, ProjectActivationResultDto, ResourceMutationResultDto,
};
use serde::{Deserialize, Serialize};
use yss_computation_settings::ComputationSettingsMutationReceipt;

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
    GraphDelta {
        project_instance_id: String,
        delta: crate::schema::application_event::GraphDeltaEventDto<
            yss_graph_document_edit::GraphDocumentPatch,
        >,
    },
    #[serde(rename_all = "camelCase")]
    ResourceMutationCommitted {
        result: ResourceMutationResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ProjectSaved {
        result: crate::schema::ProjectSaveResultDto,
    },
    #[serde(rename_all = "camelCase")]
    ComputationSettingsChanged {
        result: ComputationSettingsMutationReceipt,
    },
}
