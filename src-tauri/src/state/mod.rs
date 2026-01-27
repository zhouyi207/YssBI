//! 项目状态管理模块
//!
//! 提供全局状态管理，作为数据的 Single Source of Truth。
//! 前端通过 Tauri 命令进行 CRUD 操作，通过 Tauri Events 接收数据变更通知。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter};

use crate::project::{
    CanvasState, PinDefinition, ProjectData, SerializedNode, SubGraphData, SubGraphType,
};
use crate::schema::VariableDefinition;

// ==================== 项目状态 ====================

/// 全局项目状态
pub struct ProjectState {
    /// 项目数据
    pub data: Arc<RwLock<ProjectData>>,
    /// 当前项目文件路径
    pub current_path: Arc<RwLock<Option<String>>>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(ProjectData::default())),
            current_path: Arc::new(RwLock::new(None)),
        }
    }

    /// 获取项目数据的克隆
    pub fn get_data(&self) -> ProjectData {
        self.data.read().unwrap().clone()
    }

    /// 设置项目数据
    pub fn set_data(&self, data: ProjectData) {
        use tauri_plugin_log::log::info;
        info!(
            "[ProjectState] Setting data: global_vars={}, events={}, functions={}, macros={}",
            data.global_variables.len(),
            data.events.len(),
            data.functions.len(),
            data.macros.len()
        );
        *self.data.write().unwrap() = data;
    }

    /// 获取当前路径
    pub fn get_current_path(&self) -> Option<String> {
        self.current_path.read().unwrap().clone()
    }

    /// 设置当前路径
    pub fn set_current_path(&self, path: Option<String>) {
        *self.current_path.write().unwrap() = path;
    }

    /// 清空项目
    pub fn clear(&self) {
        *self.data.write().unwrap() = ProjectData::default();
        *self.current_path.write().unwrap() = None;
    }
}

// ==================== 事件类型 ====================

/// 项目事件（用于通知前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ProjectEvent {
    // 项目级事件
    ProjectLoaded {
        data: ProjectData,
        path: Option<String>,
    },
    ProjectCleared,
    ProjectSaved {
        path: String,
    },

    // Event 子图事件
    EventCreated {
        id: String,
        data: SubGraphData,
    },
    EventUpdated {
        id: String,
        data: SubGraphData,
    },
    EventDeleted {
        id: String,
    },

    // Function 子图事件
    FunctionCreated {
        id: String,
        data: SubGraphData,
    },
    FunctionUpdated {
        id: String,
        data: SubGraphData,
    },
    FunctionDeleted {
        id: String,
    },

    // Macro 子图事件
    MacroCreated {
        id: String,
        data: SubGraphData,
    },
    MacroUpdated {
        id: String,
        data: SubGraphData,
    },
    MacroDeleted {
        id: String,
    },

    // 全局变量事件
    GlobalVariableCreated {
        id: String,
        data: VariableDefinition,
    },
    GlobalVariableUpdated {
        id: String,
        data: VariableDefinition,
    },
    GlobalVariableDeleted {
        id: String,
    },

    // 节点事件
    NodesUpdated {
        subgraph_id: String,
        nodes: Vec<SerializedNode>,
    },

    // 局部变量事件
    LocalVariableCreated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    LocalVariableUpdated {
        subgraph_id: String,
        variable_id: String,
        data: VariableDefinition,
    },
    LocalVariableDeleted {
        subgraph_id: String,
        variable_id: String,
    },
}

/// 发送项目事件到前端
pub fn emit_project_event(app_handle: &AppHandle, event: ProjectEvent) {
    if let Err(e) = app_handle.emit("project-event", &event) {
        eprintln!("Failed to emit project event: {}", e);
    }
}

// ==================== 辅助宏 ====================

/// 获取子图的可变引用（用于避免借用检查器问题）
macro_rules! get_subgraph_mut {
    ($project:expr, $id:expr) => {{
        if $project.events.contains_key($id) {
            $project.events.get_mut($id)
        } else if $project.functions.contains_key($id) {
            $project.functions.get_mut($id)
        } else if $project.macros.contains_key($id) {
            $project.macros.get_mut($id)
        } else {
            None
        }
    }};
}

/// 获取子图的不可变引用
macro_rules! get_subgraph {
    ($project:expr, $id:expr) => {{
        if $project.events.contains_key($id) {
            $project.events.get($id)
        } else if $project.functions.contains_key($id) {
            $project.functions.get($id)
        } else if $project.macros.contains_key($id) {
            $project.macros.get($id)
        } else {
            None
        }
    }};
}

// ==================== CRUD 操作实现 ====================

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

    pub fn get_local_variables(
        &self,
        subgraph_id: &str,
    ) -> Result<HashMap<String, VariableDefinition>, String> {
        let project = self.data.read().unwrap();
        let subgraph = get_subgraph!(project, subgraph_id)
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
        let subgraph = get_subgraph_mut!(project, subgraph_id)
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
        let subgraph = get_subgraph_mut!(project, subgraph_id)
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
        let subgraph = get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if subgraph.variables.remove(var_id).is_none() {
            return Err(format!(
                "Variable '{}' not found in subgraph '{}'",
                var_id, subgraph_id
            ));
        }
        Ok(())
    }

    // ==================== Nodes CRUD ====================

    pub fn get_nodes(&self, subgraph_id: &str) -> Result<Vec<SerializedNode>, String> {
        let project = self.data.read().unwrap();
        let subgraph = get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.nodes.clone())
    }

    pub fn set_nodes(&self, subgraph_id: &str, nodes: Vec<SerializedNode>) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.nodes = nodes;
        Ok(())
    }

    pub fn update_canvas(&self, subgraph_id: &str, canvas: CanvasState) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.canvas = canvas;
        Ok(())
    }

    // ==================== SubGraph 输入输出 ====================

    pub fn update_subgraph_io(
        &self,
        subgraph_id: &str,
        inputs: Option<Vec<PinDefinition>>,
        outputs: Option<Vec<PinDefinition>>,
    ) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if let Some(inputs) = inputs {
            subgraph.inputs = inputs;
        }
        if let Some(outputs) = outputs {
            subgraph.outputs = outputs;
        }
        Ok(())
    }

    pub fn rename_subgraph(&self, subgraph_id: &str, new_name: String) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.name = new_name;
        Ok(())
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_state_new() {
        let state = ProjectState::new();
        let data = state.get_data();
        assert!(data.events.is_empty());
        assert!(data.functions.is_empty());
        assert!(data.macros.is_empty());
        assert!(data.global_variables.is_empty());
    }

    #[test]
    fn test_event_crud() {
        let state = ProjectState::new();
        let event_data = SubGraphData {
            id: "test-event".to_string(),
            name: "Test Event".to_string(),
            sub_type: SubGraphType::Event,
            nodes: vec![],
            canvas: CanvasState::default(),
            variables: HashMap::new(),
            inputs: vec![],
            outputs: vec![],
        };

        // Create
        state
            .create_event("test-event".to_string(), event_data.clone())
            .unwrap();
        assert_eq!(state.get_events().len(), 1);

        // Read
        let retrieved = state.get_event("test-event").unwrap();
        assert_eq!(retrieved.name, "Test Event");

        // Update
        let mut updated = event_data.clone();
        updated.name = "Updated Event".to_string();
        state.update_event("test-event", updated).unwrap();
        assert_eq!(state.get_event("test-event").unwrap().name, "Updated Event");

        // Delete
        state.delete_event("test-event").unwrap();
        assert!(state.get_events().is_empty());
    }
}
