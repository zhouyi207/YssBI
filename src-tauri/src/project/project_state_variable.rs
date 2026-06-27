use super::ProjectState;
use super::unique_name;
use crate::graph::value::{DataType, DataValue};
use crate::variable::VariableId;
use crate::variable::{VariableInstance, VariableScope};

impl ProjectState {
    pub fn add_variable(
        &self,
        name: &str,
        data_type: DataType,
        data_value: DataValue,
        description: &str,
        scope: VariableScope,
        tags: Vec<String>,
    ) -> VariableInstance {
        let unique_var_name = {
            let project_data = self.project_data.read().unwrap();
            let existing: Vec<&str> = project_data
                .variables
                .values()
                .map(|v| v.name.as_str())
                .collect();
            unique_name::unique_name(name, existing)
        };

        let variable_instance = VariableInstance {
            id: VariableId::new(),
            name: unique_var_name,
            data_type: data_type,
            data_value: data_value,
            description: description.to_string(),
            scope: scope,
            tags: tags,
        };

        self.project_data
            .write()
            .unwrap()
            .variables
            .insert(variable_instance.id, variable_instance.clone());
        variable_instance
    }

    pub fn remove_variable(&self, variable_id: &VariableId) -> Option<VariableInstance> {
        self.project_data
            .write()
            .unwrap()
            .variables
            .remove(&variable_id)
    }

    pub fn get_variable(&self, variable_id: &VariableId) -> Option<VariableInstance> {
        self.project_data
            .read()
            .unwrap()
            .variables
            .get(variable_id)
            .cloned()
    }

    /// 更新变量（部分字段），返回更新后的实例
    pub fn update_variable(
        &self,
        variable_id: &VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Option<VariableInstance> {
        let mut data = self.project_data.write().unwrap();
        let var = data.variables.get_mut(variable_id)?;
        if let Some(n) = name {
            var.name = n;
        }
        if let Some(dt) = data_type {
            let changed = var.data_type != dt;
            var.data_type = dt;
            if changed && data_value.is_none() {
                var.data_value = var.data_type.default_value();
            }
        }
        if let Some(dv) = data_value {
            var.data_value = dv;
        }
        if let Some(d) = description {
            var.description = d;
        }
        if let Some(t) = tags {
            var.tags = t;
        }
        Some(var.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_int_variable(state: &ProjectState) -> VariableInstance {
        state.add_variable(
            "x",
            DataType::Int32,
            DataValue::Int32(42),
            "",
            VariableScope::Global,
            vec![],
        )
    }

    #[test]
    fn update_variable_resets_value_to_type_default_when_type_changes_without_value() {
        let state = ProjectState::new();
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                None,
                None,
                None,
            )
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(false));
    }

    #[test]
    fn update_variable_keeps_explicit_value_when_type_and_value_are_both_changed() {
        let state = ProjectState::new();
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                Some(DataValue::Boolean(true)),
                None,
                None,
            )
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(true));
    }
}
