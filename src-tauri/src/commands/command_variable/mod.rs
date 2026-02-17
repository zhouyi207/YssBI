use crate::event::{emit_project_event, Event, EventVariable};
use crate::graph::value::{DataType, DataValue};
use crate::project::ProjectState;
use crate::schema::VariableInstanceDTO;
use crate::variable::{VariableId, VariableScope};
use tauri::{AppHandle, State};

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
) -> Result<String, String> {
    let variable = state.add_variable(name, data_type, data_value, description, scope, tags);
    let variable_id = variable.id.to_string();
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
) -> Result<VariableInstanceDTO, String> {
    let variable = state.get_variable(&variable_id).unwrap();
    Ok((&variable).into())
}

/// 更新变量（统一接口）
#[tauri::command]
pub fn update_variable(id: String, state: State<ProjectState>) -> Result<(), String> {
    Ok(())
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
) -> Result<(), String> {
    let variable = state.remove_variable(&variable_id).unwrap();
    emit_project_event(
        &app,
        Event::Variable(EventVariable::VariableDeleted {
            variable_id: variable.id,
            variable_scope: variable.scope,
        }),
    );
    Ok(())
}
