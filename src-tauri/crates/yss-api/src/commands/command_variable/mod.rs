use crate::error::CommandError;
use crate::event::{Event, emit_project_event_result};
use crate::schema::VariableInstanceDTO;
use crate::schema::application_event::ResourceMutationResultDto;
use tauri::{AppHandle, State};
use yss_data_contract::{DataType, DataValue};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};
use yss_variable_contract::{VariableId, VariableScope};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableCommandResult {
    pub variable_id: String,
    pub variable: Option<VariableInstanceDTO>,
    pub result: Option<ResourceMutationResultDto>,
}

fn variable_dto(
    variable: &yss_variable_contract::VariableInstance,
) -> Result<VariableInstanceDTO, CommandError> {
    VariableInstanceDTO::try_from(variable)
        .map_err(|error| CommandError::diagnosed("variable_dto_mapping_failed", error))
}

/// 创建变量（统一接口，支持全局和局部变量）
#[tauri::command]
pub fn create_variable(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    name: &str,
    data_type: DataType,
    data_value: DataValue,
    description: &str,
    scope: VariableScope,
    tags: Vec<String>,
    project_instance_id: ProjectInstanceId,
    expected_collection_revision: u64,
    operation_id: OperationId,
) -> Result<VariableCommandResult, CommandError> {
    let request = yss_application::variable_mutation::VariableMutationRequest::Create {
        project_instance_id,
        name: name.to_owned(),
        data_type,
        data_value,
        description: description.to_owned(),
        scope,
        tags,
        expected_collection_revision,
        operation_id,
    };
    let committed = application
        .mutate_variable(request)
        .map_err(map_variable_mutation_error)?;
    let variable = variable_dto(committed.variable())?;
    emit_application_event(&app, committed.event())?;
    Ok(VariableCommandResult {
        variable_id: committed.variable().id.to_string(),
        variable: Some(variable),
        result: None,
    })
}

/// 获取变量（统一接口）
#[tauri::command]
pub fn get_variable(
    application: State<yss_application::execution::ApplicationState>,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
) -> Result<VariableInstanceDTO, CommandError> {
    let variable = application
        .query_variable(project_instance_id, variable_id)
        .map_err(map_variable_query_error)?;
    variable_dto(&variable)
}

/// 更新变量（统一接口，部分更新）
#[tauri::command]
pub fn update_variable(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    variable_id: VariableId,
    name: Option<String>,
    data_type: Option<DataType>,
    data_value: Option<DataValue>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<VariableCommandResult, CommandError> {
    let request = yss_application::variable_mutation::VariableMutationRequest::Update {
        project_instance_id,
        variable_id,
        name,
        data_type,
        data_value,
        description,
        tags,
        expected_revision,
        operation_id,
    };
    let committed = application
        .mutate_variable(request)
        .map_err(map_variable_mutation_error)?;
    let variable = variable_dto(committed.variable())?;
    emit_application_event(&app, committed.event())?;
    Ok(VariableCommandResult {
        variable_id: committed.variable().id.to_string(),
        variable: Some(variable),
        result: None,
    })
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<VariableCommandResult, CommandError> {
    let request = yss_application::variable_mutation::VariableMutationRequest::Delete {
        project_instance_id,
        variable_id,
        expected_revision,
        operation_id,
    };
    let committed = application
        .mutate_variable(request)
        .map_err(map_variable_mutation_error)?;
    emit_application_event(&app, committed.event())?;
    Ok(VariableCommandResult {
        variable_id: committed.variable().id.to_string(),
        variable: None,
        result: None,
    })
}

fn emit_application_event(
    app: &AppHandle,
    event: &yss_application::events::ApplicationEvent,
) -> Result<(), CommandError> {
    let event = crate::schema::application_event::application_event_to_transport(event)
        .map_err(|error| CommandError::diagnosed("variable_event_mapping_failed", error))?;
    emit_project_event_result(app, &Event::Project(event))
        .map_err(|error| CommandError::diagnosed("variable_event_emit_failed", error))
}

fn map_variable_mutation_error(
    error: yss_application::variable_mutation::VariableMutationApplicationError,
) -> CommandError {
    use yss_application::variable_mutation::VariableMutationApplicationError;
    match error {
        VariableMutationApplicationError::SessionCapture(error) => map_session_capture_error(error),
        VariableMutationApplicationError::ProjectIdentityMismatch { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        VariableMutationApplicationError::InvalidDataType => {
            CommandError::expected("invalid_variable_type")
        }
        VariableMutationApplicationError::VariableNotFound { .. } => {
            CommandError::expected("variable_not_found")
        }
        VariableMutationApplicationError::Project(error) => {
            crate::commands::project_failure::application_project_command_error(error)
        }
        VariableMutationApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("variable_session_changed", error)
        }
    }
}

fn map_variable_query_error(
    error: yss_application::variable_mutation::VariableQueryApplicationError,
) -> CommandError {
    use yss_application::variable_mutation::VariableQueryApplicationError;
    match error {
        VariableQueryApplicationError::SessionCapture(error) => map_session_capture_error(error),
        VariableQueryApplicationError::ProjectIdentityMismatch { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        VariableQueryApplicationError::VariableNotFound { .. } => {
            CommandError::expected("variable_not_found")
        }
        VariableQueryApplicationError::Project(error) => {
            crate::commands::project_failure::application_project_command_error(error)
        }
        VariableQueryApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("variable_session_changed", error)
        }
    }
}

fn map_session_capture_error(
    error: yss_application::execution::SessionCaptureError,
) -> CommandError {
    match error {
        yss_application::execution::SessionCaptureError::Inactive => {
            CommandError::expected("stale_project_lifecycle")
        }
        yss_application::execution::SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        yss_application::execution::SessionCaptureError::Recovering => {
            CommandError::expected("project_recovery_required")
                .with_details(serde_json::json!({ "recoveryRequired": true }))
        }
    }
}
