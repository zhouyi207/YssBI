use crate::project::ProjectState;
use std::sync::Arc;
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_operation::{
    ProjectOperationAdmissionError, ProjectOperationLedger, ProjectOperationReservation,
};

impl ProjectState {
    pub(crate) fn reserve_resource_operation(
        &self,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectOperationReservation, ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before resource operation admission".into(),
            });
        }
        let reservation = ProjectOperationLedger::reserve(
            Arc::clone(&self.resource_operations),
            project_instance_id,
            operation_id,
        )
        .map_err(map_operation_admission_error);
        drop(publication);
        reservation
    }
}

fn map_operation_admission_error(error: ProjectOperationAdmissionError) -> ProjectFilesystemError {
    let message = error.to_string();
    match error {
        ProjectOperationAdmissionError::StaleProject { .. } => {
            ProjectFilesystemError::StaleProjectLifecycle { message }
        }
        ProjectOperationAdmissionError::DuplicateOperation { .. } => {
            ProjectFilesystemError::DuplicateOperation { message }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_errors_keep_existing_project_error_categories() {
        let requested = ProjectInstanceId::from_existing("requested".into());
        let current = ProjectInstanceId::from_existing("current".into());
        let stale = map_operation_admission_error(ProjectOperationAdmissionError::StaleProject {
            requested_project_instance_id: requested.clone(),
            current_project_instance_id: current,
        });
        let duplicate =
            map_operation_admission_error(ProjectOperationAdmissionError::DuplicateOperation {
                operation_id: OperationId::new(),
                project_instance_id: requested,
            });

        assert!(matches!(
            stale,
            ProjectFilesystemError::StaleProjectLifecycle { .. }
        ));
        assert!(matches!(
            duplicate,
            ProjectFilesystemError::DuplicateOperation { .. }
        ));
    }
}
