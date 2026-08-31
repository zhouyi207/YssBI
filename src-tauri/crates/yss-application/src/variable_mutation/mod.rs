#![allow(
    dead_code,
    reason = "staged until the Application session cutover installs the variable command caller"
)]

use thiserror::Error;

use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::events::{ApplicationEvent, committed_resource_mutation_from_project};
use yss_data_contract::{DataType, DataValue};
use yss_project::project_writers::{
    CreateVariableRequest as ProjectCreateVariableRequest,
    DeleteVariableRequest as ProjectDeleteVariableRequest,
    UpdateVariableRequest as ProjectUpdateVariableRequest, VariableMutationResult,
};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_variable_contract::{VariableId, VariableInstance, VariableScope};

pub enum VariableMutationRequest {
    Create {
        project_instance_id: ProjectInstanceId,
        name: String,
        data_type: DataType,
        data_value: DataValue,
        description: String,
        scope: VariableScope,
        tags: Vec<String>,
        expected_collection_revision: u64,
        operation_id: OperationId,
    },
    Update {
        project_instance_id: ProjectInstanceId,
        variable_id: VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    },
    Delete {
        project_instance_id: ProjectInstanceId,
        variable_id: VariableId,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    },
}

impl VariableMutationRequest {
    fn project_instance_id(&self) -> &ProjectInstanceId {
        match self {
            Self::Create {
                project_instance_id,
                ..
            }
            | Self::Update {
                project_instance_id,
                ..
            }
            | Self::Delete {
                project_instance_id,
                ..
            } => project_instance_id,
        }
    }
}

pub struct CommittedVariableMutation {
    variable: VariableInstance,
    event: ApplicationEvent,
}

impl CommittedVariableMutation {
    pub fn variable(&self) -> &VariableInstance {
        &self.variable
    }

    pub fn event(&self) -> &ApplicationEvent {
        &self.event
    }
}

#[derive(Debug, Error)]
pub enum VariableMutationApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("variable mutation belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("variable type is not supported by the Project contract")]
    InvalidDataType,
    #[error("variable does not exist")]
    VariableNotFound { variable: VariableId },
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("captured application session changed after variable commit")]
    SessionChanged(#[source] SessionRevalidationError),
}

#[derive(Debug, Error)]
pub enum VariableQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("variable query belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("variable does not exist")]
    VariableNotFound { variable: VariableId },
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("captured application session changed during variable query")]
    SessionChanged(#[source] SessionRevalidationError),
}

impl ApplicationState {
    pub fn query_variable(
        &self,
        project_instance_id: ProjectInstanceId,
        variable_id: VariableId,
    ) -> Result<VariableInstance, VariableQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != &project_instance_id {
            return Err(VariableQueryApplicationError::ProjectIdentityMismatch {
                requested: project_instance_id,
            });
        }
        let variable = captured.project().get_variable(&variable_id)?.ok_or(
            VariableQueryApplicationError::VariableNotFound {
                variable: variable_id,
            },
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(VariableQueryApplicationError::SessionChanged)?;
        Ok(variable)
    }

    pub fn mutate_variable(
        &self,
        request: VariableMutationRequest,
    ) -> Result<CommittedVariableMutation, VariableMutationApplicationError> {
        let captured = self.capture_session()?;
        let result = mutate_variable_in_session(&captured, request)?;
        self.revalidate_captured_session(&captured)
            .map_err(VariableMutationApplicationError::SessionChanged)?;
        Ok(result)
    }
}

pub(crate) fn mutate_variable_in_session(
    session: &ApplicationSession,
    request: VariableMutationRequest,
) -> Result<CommittedVariableMutation, VariableMutationApplicationError> {
    if request.project_instance_id() != session.project_instance_id() {
        return Err(VariableMutationApplicationError::ProjectIdentityMismatch {
            requested: request.project_instance_id().clone(),
        });
    }

    let committed = match request {
        VariableMutationRequest::Create {
            project_instance_id,
            name,
            data_type,
            data_value,
            description,
            scope,
            tags,
            expected_collection_revision,
            operation_id,
        } => {
            if matches!(data_type, DataType::Any) {
                return Err(VariableMutationApplicationError::InvalidDataType);
            }
            session
                .project()
                .create_variable_transaction(ProjectCreateVariableRequest {
                    project_instance_id,
                    name,
                    data_type,
                    data_value,
                    description,
                    scope,
                    tags,
                    expected_collection_revision,
                    operation_id,
                })?
        }
        VariableMutationRequest::Update {
            project_instance_id,
            variable_id,
            name,
            data_type,
            data_value,
            description,
            tags,
            expected_revision,
            operation_id,
        } => {
            if data_type
                .as_ref()
                .is_some_and(|value| matches!(value, DataType::Any))
            {
                return Err(VariableMutationApplicationError::InvalidDataType);
            }
            session.project().get_variable(&variable_id)?.ok_or(
                VariableMutationApplicationError::VariableNotFound {
                    variable: variable_id,
                },
            )?;
            session
                .project()
                .update_variable_transaction(ProjectUpdateVariableRequest {
                    project_instance_id,
                    id: variable_id,
                    name,
                    data_type,
                    data_value,
                    description,
                    tags,
                    expected_revision,
                    operation_id,
                })?
        }
        VariableMutationRequest::Delete {
            project_instance_id,
            variable_id,
            expected_revision,
            operation_id,
        } => {
            session.project().get_variable(&variable_id)?.ok_or(
                VariableMutationApplicationError::VariableNotFound {
                    variable: variable_id,
                },
            )?;
            session
                .project()
                .delete_variable_transaction(ProjectDeleteVariableRequest {
                    project_instance_id,
                    id: variable_id,
                    expected_revision,
                    operation_id,
                })?
        }
    };

    Ok(committed_variable_mutation(committed))
}

fn committed_variable_mutation(committed: VariableMutationResult) -> CommittedVariableMutation {
    let (variable, facts) = committed.into_parts();
    let event =
        ApplicationEvent::ResourceCommitted(committed_resource_mutation_from_project(facts));
    CommittedVariableMutation { variable, event }
}
