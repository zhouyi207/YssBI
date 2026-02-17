use super::ProjectState;
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
        let variable_instance = VariableInstance {
            id: VariableId::new(),
            name: name.to_string(),
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
        self.project_data.read().unwrap().variables.get(variable_id).cloned()
    }
}
