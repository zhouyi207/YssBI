use crate::events::{CommittedResourceMutation, committed_resource_mutation_from_project};
use crate::session::{ApplicationState, SessionCaptureError, SessionRevalidationError};
use yss_project::{
    ProjectOperationError,
    file_resources::{FileCommand, FileSnapshot, ResourceFile},
};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::file::{FileContent, FilePath};
#[derive(Debug, thiserror::Error)]
pub enum FileApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectOperationError),
    #[error("file application session changed")]
    SessionChanged(#[from] SessionRevalidationError),
}
pub struct FileMutation<T: FileContent> {
    pub snapshot: Option<FileSnapshot<T>>,
    pub mutation: CommittedResourceMutation,
}
impl ApplicationState {
    pub fn read_file<T: ResourceFile>(
        &self,
        project: ProjectInstanceId,
        path: FilePath<T>,
    ) -> Result<FileSnapshot<T>, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session.project().read_file(&project, &path)?;
        self.revalidate_captured_session(&session)?;
        Ok(result)
    }
    pub fn apply_file_command<T: ResourceFile>(
        &self,
        project: ProjectInstanceId,
        operation: OperationId,
        command: FileCommand<T>,
    ) -> Result<FileMutation<T>, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session
            .project()
            .apply_file_command(&project, operation, command)?;
        self.revalidate_captured_session(&session)?;
        Ok(FileMutation {
            snapshot: result.snapshot,
            mutation: committed_resource_mutation_from_project(result.mutation),
        })
    }
}
