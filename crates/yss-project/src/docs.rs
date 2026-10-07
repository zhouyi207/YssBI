use crate::{
    ProjectOperationError, ProjectState,
    file_resources::{self, ResourceFile},
};
use std::collections::HashMap;
use yss_project_history::{DocResourceKey, ResourceKey, ResourceLifecycleKind};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::{
    ProjectData, ProjectDataPatch,
    doc::{DocDocument, DocPath, DocState},
    file::FilePatch,
};
pub type DocSnapshot = file_resources::FileSnapshot<DocDocument>;
pub type DocIndexEntry = file_resources::FileIndexEntry<DocDocument>;
pub type DocCommand = file_resources::FileCommand<DocDocument>;
pub type DocCommandResult = file_resources::FileCommandResult<DocDocument>;
impl ResourceFile for DocDocument {
    const LIFECYCLE_KIND: ResourceLifecycleKind = ResourceLifecycleKind::Doc;
    fn files(data: &ProjectData) -> &HashMap<DocPath, DocState> {
        &data.docs
    }
    fn files_mut(data: &mut ProjectData) -> &mut HashMap<DocPath, DocState> {
        &mut data.docs
    }
    fn patch(patch: FilePatch<Self>) -> ProjectDataPatch {
        ProjectDataPatch::Doc(patch)
    }
    fn key(path: &DocPath) -> ResourceKey {
        ResourceKey::Doc(DocResourceKey(path.as_str().into()))
    }
}
impl ProjectState {
    /// Read current content only while its saved file remains a valid source.
    pub fn read_doc_source(
        &self,
        project: &ProjectInstanceId,
        path: &DocPath,
    ) -> Result<DocSnapshot, ProjectOperationError> {
        self.read_file_source(project, path)
    }

    pub fn read_doc(
        &self,
        project: &ProjectInstanceId,
        path: &DocPath,
    ) -> Result<DocSnapshot, ProjectOperationError> {
        self.read_file(project, path)
    }
    pub fn apply_doc_command(
        &self,
        project: &ProjectInstanceId,
        operation: OperationId,
        command: DocCommand,
    ) -> Result<DocCommandResult, ProjectOperationError> {
        self.apply_file_command(project, operation, command)
    }
}
