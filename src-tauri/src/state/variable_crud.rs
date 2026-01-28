//! 全局和局部变量 CRUD 操作

use std::collections::HashMap;

use super::project_state::ProjectState;
use crate::schema::VariableDefinition;

impl ProjectState {
    // ==================== Global Variables CRUD ====================

    pub fn get_global_variables(&self) -> HashMap<String, VariableDefinition> {
        self.data.read().unwrap().global_variables.clone()
    }

    pub fn get_global_variable(&self, id: &str) -> Option<VariableDefinition> {
        self.data.read().unwrap().global_variables.get(id).cloned()
    }

    pub fn create_global_variable(
        &self,
        id: String,
        data: VariableDefinition,
    ) -> Result<VariableDefinition, String> {
        let mut project = self.data.write().unwrap();
        if project.global_variables.contains_key(&id) {
            return Err(format!("Global variable with id '{}' already exists", id));
        }
        project.global_variables.insert(id, data.clone());
        Ok(data)
    }

    pub fn update_global_variable(
        &self,
        id: &str,
        data: VariableDefinition,
    ) -> Result<VariableDefinition, String> {
        let mut project = self.data.write().unwrap();
        if !project.global_variables.contains_key(id) {
            return Err(format!("Global variable with id '{}' not found", id));
        }
        project
            .global_variables
            .insert(id.to_string(), data.clone());
        Ok(data)
    }

    pub fn delete_global_variable(&self, id: &str) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        if project.global_variables.remove(id).is_none() {
            return Err(format!("Global variable with id '{}' not found", id));
        }
        Ok(())
    }

    // ==================== Local Variables CRUD ====================

    pub fn get_local_variables(
        &self,
        subgraph_id: &str,
    ) -> Result<HashMap<String, VariableDefinition>, String> {
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.variables.clone())
    }

    pub fn create_local_variable(
        &self,
        subgraph_id: &str,
        var_id: String,
        data: VariableDefinition,
    ) -> Result<VariableDefinition, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if subgraph.variables.contains_key(&var_id) {
            return Err(format!(
                "Variable '{}' already exists in subgraph '{}'",
                var_id, subgraph_id
            ));
        }
        subgraph.variables.insert(var_id, data.clone());
        Ok(data)
    }

    pub fn update_local_variable(
        &self,
        subgraph_id: &str,
        var_id: &str,
        data: VariableDefinition,
    ) -> Result<VariableDefinition, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if !subgraph.variables.contains_key(var_id) {
            return Err(format!(
                "Variable '{}' not found in subgraph '{}'",
                var_id, subgraph_id
            ));
        }
        subgraph.variables.insert(var_id.to_string(), data.clone());
        Ok(data)
    }

    pub fn delete_local_variable(&self, subgraph_id: &str, var_id: &str) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if subgraph.variables.remove(var_id).is_none() {
            return Err(format!(
                "Variable '{}' not found in subgraph '{}'",
                var_id, subgraph_id
            ));
        }
        Ok(())
    }
}
