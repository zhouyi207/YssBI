//! 项目事件定义和发送逻辑

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::project::{DataFrameData, ProjectData, SerializedNode, SubGraphData};
use crate::schema::VariableDefinition;

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

    // 连接事件
    ConnectionsUpdated {
        subgraph_id: String,
        connections: Vec<crate::project::ConnectionDto>,
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

    // DataFrame 事件
    DataFrameCreated {
        id: String,
        data: DataFrameData,
    },
    DataFrameDeleted {
        id: String,
    },
}

/// 发送项目事件到前端
pub fn emit_project_event(app_handle: &AppHandle, event: ProjectEvent) {
    use tauri_plugin_log::log::error;
    if let Err(e) = app_handle.emit("project-event", &event) {
        error!("Failed to emit project event: {}", e);
    }
}
