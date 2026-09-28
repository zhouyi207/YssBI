use crate::{
    events::committed_resource_mutation_from_project,
    file_resources::{FileApplicationError, FileMutation},
    session::ApplicationState,
};
use yss_project::docs::{DocCommand, DocSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::doc::{DocDocument, DocPath};
impl ApplicationState {
    pub fn read_doc(
        &self,
        project: ProjectInstanceId,
        path: DocPath,
    ) -> Result<DocSnapshot, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session.project().read_doc(&project, &path)?;
        self.revalidate_captured_session(&session)?;
        Ok(result)
    }
    pub fn apply_doc_command(
        &self,
        project: ProjectInstanceId,
        operation: OperationId,
        command: DocCommand,
    ) -> Result<FileMutation<DocDocument>, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session
            .project()
            .apply_doc_command(&project, operation, command)?;
        self.revalidate_captured_session(&session)?;
        Ok(FileMutation {
            snapshot: result.snapshot,
            mutation: committed_resource_mutation_from_project(result.mutation),
        })
    }
}
