//! SubGraph (Events/Functions/Macros) CRUD 操作

use std::collections::HashMap;

use super::project_state::ProjectState;
use crate::project::{SubGraphData, SubGraphType};

impl ProjectState {
    // ==================== Events CRUD ====================

    pub fn get_events(&self) -> HashMap<String, SubGraphData> {
        self.data.read().unwrap().events.clone()
    }

    pub fn get_event(&self, id: &str) -> Option<SubGraphData> {
        self.data.read().unwrap().events.get(id).cloned()
    }

    pub fn create_event(&self, id: String, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if project.events.contains_key(&id) {
            return Err(format!("Event with id '{}' already exists", id));
        }
        project.events.insert(id, data.clone());
        Ok(data)
    }

    pub fn update_event(&self, id: &str, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if !project.events.contains_key(id) {
            return Err(format!("Event with id '{}' not found", id));
        }
        project.events.insert(id.to_string(), data.clone());
        Ok(data)
    }

    pub fn delete_event(&self, id: &str) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        if project.events.remove(id).is_none() {
            return Err(format!("Event with id '{}' not found", id));
        }
        Ok(())
    }

    // ==================== Functions CRUD ====================

    pub fn get_functions(&self) -> HashMap<String, SubGraphData> {
        self.data.read().unwrap().functions.clone()
    }

    pub fn get_function(&self, id: &str) -> Option<SubGraphData> {
        self.data.read().unwrap().functions.get(id).cloned()
    }

    pub fn create_function(&self, id: String, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if project.functions.contains_key(&id) {
            return Err(format!("Function with id '{}' already exists", id));
        }
        project.functions.insert(id, data.clone());
        Ok(data)
    }

    pub fn update_function(&self, id: &str, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if !project.functions.contains_key(id) {
            return Err(format!("Function with id '{}' not found", id));
        }
        project.functions.insert(id.to_string(), data.clone());
        Ok(data)
    }

    pub fn delete_function(&self, id: &str) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        if project.functions.remove(id).is_none() {
            return Err(format!("Function with id '{}' not found", id));
        }
        Ok(())
    }

    // ==================== Macros CRUD ====================

    pub fn get_macros(&self) -> HashMap<String, SubGraphData> {
        self.data.read().unwrap().macros.clone()
    }

    pub fn get_macro(&self, id: &str) -> Option<SubGraphData> {
        self.data.read().unwrap().macros.get(id).cloned()
    }

    pub fn create_macro(&self, id: String, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if project.macros.contains_key(&id) {
            return Err(format!("Macro with id '{}' already exists", id));
        }
        project.macros.insert(id, data.clone());
        Ok(data)
    }

    pub fn update_macro(&self, id: &str, data: SubGraphData) -> Result<SubGraphData, String> {
        let mut project = self.data.write().unwrap();
        if !project.macros.contains_key(id) {
            return Err(format!("Macro with id '{}' not found", id));
        }
        project.macros.insert(id.to_string(), data.clone());
        Ok(data)
    }

    pub fn delete_macro(&self, id: &str) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        if project.macros.remove(id).is_none() {
            return Err(format!("Macro with id '{}' not found", id));
        }
        Ok(())
    }

    // ==================== SubGraph 辅助方法 ====================

    /// 查找子图所在的集合类型
    pub fn find_subgraph_type(&self, id: &str) -> Option<SubGraphType> {
        let project = self.data.read().unwrap();
        if project.events.contains_key(id) {
            Some(SubGraphType::Event)
        } else if project.functions.contains_key(id) {
            Some(SubGraphType::Function)
        } else if project.macros.contains_key(id) {
            Some(SubGraphType::Macro)
        } else {
            None
        }
    }
}
