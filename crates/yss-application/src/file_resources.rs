use crate::events::CommittedResourceMutation;
use crate::session::{SessionCaptureError, SessionRevalidationError};
use yss_project::{ProjectOperationError, file_resources::FileSnapshot};
use yss_project_model::file::FileContent;
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
