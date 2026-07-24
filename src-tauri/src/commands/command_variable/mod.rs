use crate::error::AppError;
use crate::event::{Event, EventVariable, emit_project_event};
use crate::graph::value::{DataType, DataValue};
use crate::project::ProjectState;
use crate::schema::VariableInstanceDTO;
use crate::variable::{VariableId, VariableScope};
use tauri::{AppHandle, State};

fn ensure_variable_data_type(data_type: &DataType) -> Result<(), AppError> {
    if matches!(data_type, DataType::Any) {
        return Err(AppError::new(
            "invalid_variable_type",
            "Variable data type cannot be Any",
        ));
    }
    Ok(())
}

/// 创建变量（统一接口，支持全局和局部变量）
#[tauri::command]
pub fn create_variable(
    state: State<ProjectState>,
    app: AppHandle,
    name: &str,
    data_type: DataType,
    data_value: DataValue,
    description: &str,
    scope: VariableScope,
    tags: Vec<String>,
) -> Result<String, AppError> {
    ensure_variable_data_type(&data_type)?;
    let variable = state.add_variable(name, data_type, data_value, description, scope, tags);
    let variable_id = variable.id.to_string();
    if matches!(variable.scope, VariableScope::Global) {
        state.persist_current_project()?;
    }
    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableCreated {
            variable_id: variable.id,
            variable_scope: variable.scope.clone(),
            data: (&variable).into(),
        }),
    );
    Ok(variable_id)
}

/// 获取变量（统一接口）
#[tauri::command]
pub fn get_variable(
    _app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
) -> Result<VariableInstanceDTO, AppError> {
    let variable = state.get_variable(&variable_id).ok_or_else(|| {
        AppError::new(
            "variable_not_found",
            format!("Variable '{}' not found", variable_id),
        )
    })?;
    Ok((&variable).into())
}

/// 更新变量（统一接口，部分更新）
#[tauri::command]
pub fn update_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
    name: Option<String>,
    data_type: Option<DataType>,
    data_value: Option<DataValue>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<VariableInstanceDTO, AppError> {
    if let Some(ref dt) = data_type {
        ensure_variable_data_type(dt)?;
    }

    let updated = state
        .update_variable(&variable_id, name, data_type, data_value, description, tags)
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{}' not found", variable_id),
            )
        })?;
    let persist_global = matches!(updated.scope, VariableScope::Global);

    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableUpdated {
            variable_id: updated.id,
            variable_scope: updated.scope.clone(),
            data: (&updated).into(),
        }),
    );

    if persist_global {
        state.persist_current_project()?;
    }
    Ok((&updated).into())
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
) -> Result<(), AppError> {
    let variable = state.remove_variable(&variable_id).ok_or_else(|| {
        AppError::new(
            "variable_not_found",
            format!("Variable '{}' not found", variable_id),
        )
    })?;
    if matches!(variable.scope, VariableScope::Global) {
        state.persist_current_project()?;
    }
    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableDeleted {
            variable_id: variable.id,
            variable_scope: variable.scope,
        }),
    );
    Ok(())
}
