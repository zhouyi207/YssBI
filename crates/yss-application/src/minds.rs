use crate::{
    events::committed_resource_mutation_from_project,
    file_resources::{FileApplicationError, FileMutation},
    session::ApplicationState,
};
use yss_project::minds::{MindCommand, MindSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::mind::{MindDocument, MindPath};
impl ApplicationState {
    pub fn read_mind(
        &self,
        project: ProjectInstanceId,
        path: MindPath,
    ) -> Result<MindSnapshot, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session.project().read_mind(&project, &path)?;
        self.revalidate_captured_session(&session)?;
        Ok(result)
    }
    pub fn apply_mind_command(
        &self,
        project: ProjectInstanceId,
        operation: OperationId,
        command: MindCommand,
    ) -> Result<FileMutation<MindDocument>, FileApplicationError> {
        let session = self.capture_session()?;
        let result = session
            .project()
            .apply_mind_command(&project, operation, command)?;
        self.revalidate_captured_session(&session)?;
        Ok(FileMutation {
            snapshot: result.snapshot,
            mutation: committed_resource_mutation_from_project(result.mutation),
        })
    }
}
