//! 全局和局部变量 CRUD 操作

use std::collections::HashMap;

use super::project_state::ProjectState;
use crate::project::SubGraphType;
use crate::schema::VariableDefinition;
use uuid::Uuid;

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

    // ==================== Unified Create Variable (Backend Generated ID) ====================

    pub fn create_variable(
        &self,
        subgraph_id: Option<String>,
        name_hint: Option<String>,
        data_type_str: Option<String>,
    ) -> Result<VariableDefinition, String> {
        let mut project = self.data.write().unwrap();
        let new_id = format!("var-{}", uuid::Uuid::new_v4());

        let data_type = match data_type_str.as_deref().unwrap_or("int") {
            "int" => crate::schema::VariableDataType::Int,
            "float" => crate::schema::VariableDataType::Float,
            "string" => crate::schema::VariableDataType::String,
            "bool" => crate::schema::VariableDataType::Bool,
            _ => crate::schema::VariableDataType::Any,
        };

        let default_value = match data_type {
            crate::schema::VariableDataType::Int => serde_json::json!(0),
            crate::schema::VariableDataType::Float => serde_json::json!(0.0),
            crate::schema::VariableDataType::String => serde_json::json!(""),
            crate::schema::VariableDataType::Bool => serde_json::json!(false),
            _ => serde_json::json!(null),
        };

        if let Some(sid) = subgraph_id {
            // Local Variable
            let subgraph = crate::get_subgraph_mut!(project, &sid)
                .ok_or_else(|| format!("Subgraph '{}' not found", sid))?;

            // Generate Name
            let base_name = name_hint.unwrap_or_else(|| "New Variable".to_string());
            let mut final_name = base_name.clone();
            let mut count = 1;
            let existing_names: Vec<String> = subgraph
                .variables
                .values()
                .map(|v| v.name.clone())
                .collect();
            while existing_names.contains(&final_name) {
                final_name = format!("{}_{}", base_name, count);
                count += 1;
            }

            // Determine Scope
            let scope = match subgraph.sub_type {
                SubGraphType::Function => crate::schema::VariableScope::Function {
                    function_id: sid.clone(),
                },
                SubGraphType::Macro => crate::schema::VariableScope::Macro {
                    macro_id: sid.clone(),
                },
                _ => crate::schema::VariableScope::Global,
            };

            let mut def = VariableDefinition::new(new_id.clone(), final_name, data_type);
            def.default_value = Some(default_value);
            def.scope = scope;

            subgraph.variables.insert(new_id.clone(), def.clone());
            Ok(def)
        } else {
            // Global Variable
            let base_name = name_hint.unwrap_or_else(|| "New Global".to_string());
            let mut final_name = base_name.clone();
            let mut count = 1;
            let existing_names: Vec<String> = project
                .global_variables
                .values()
                .map(|v| v.name.clone())
                .collect();
            while existing_names.contains(&final_name) {
                final_name = format!("{}_{}", base_name, count);
                count += 1;
            }

            let mut def = VariableDefinition::new(new_id.clone(), final_name, data_type);
            def.default_value = Some(default_value);
            def.scope = crate::schema::VariableScope::Global;

            project.global_variables.insert(new_id.clone(), def.clone());
            Ok(def)
        }
    }
}
