use crate::{
    ProjectOperationError, ProjectState,
    file_resources::{self, ResourceFile},
};
use std::collections::HashMap;
use yss_project_history::{MindResourceKey, ResourceKey, ResourceLifecycleKind};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::{
    ProjectData, ProjectDataPatch,
    file::FilePatch,
    mind::{MindDocument, MindPath, MindState},
};
pub type MindSnapshot = file_resources::FileSnapshot<MindDocument>;
pub type MindIndexEntry = file_resources::FileIndexEntry<MindDocument>;
pub type MindCommand = file_resources::FileCommand<MindDocument>;
pub type MindCommandResult = file_resources::FileCommandResult<MindDocument>;
impl ResourceFile for MindDocument {
    const LIFECYCLE_KIND: ResourceLifecycleKind = ResourceLifecycleKind::Mind;
    fn files(data: &ProjectData) -> &HashMap<MindPath, MindState> {
        &data.minds
    }
    fn files_mut(data: &mut ProjectData) -> &mut HashMap<MindPath, MindState> {
        &mut data.minds
    }
    fn patch(patch: FilePatch<Self>) -> ProjectDataPatch {
        ProjectDataPatch::Mind(patch)
    }
    fn key(path: &MindPath) -> ResourceKey {
        ResourceKey::Mind(MindResourceKey(path.as_str().into()))
    }
}
impl ProjectState {
    pub fn read_mind(
        &self,
        project: &ProjectInstanceId,
        path: &MindPath,
    ) -> Result<MindSnapshot, ProjectOperationError> {
        self.read_file(project, path)
    }
    pub fn apply_mind_command(
        &self,
        project: &ProjectInstanceId,
        operation: OperationId,
        command: MindCommand,
    ) -> Result<MindCommandResult, ProjectOperationError> {
        self.apply_file_command(project, operation, command)
    }
}
